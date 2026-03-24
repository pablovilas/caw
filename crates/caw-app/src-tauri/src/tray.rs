use caw_core::{Monitor, NormalizedSession, SessionStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    App, AppHandle, Manager,
};

type SessionPidMap = Arc<Mutex<HashMap<String, u32>>>;

#[derive(Clone, Copy, PartialEq)]
enum GroupBy {
    Project,
    App,
    Assistant,
    None,
}

impl GroupBy {
    fn from_id(id: &str) -> Option<Self> {
        match id {
            "group-project" => Some(Self::Project),
            "group-app" => Some(Self::App),
            "group-assistant" => Some(Self::Assistant),
            "group-none" => Some(Self::None),
            _ => None,
        }
    }

    fn group_key(&self, s: &NormalizedSession) -> String {
        match self {
            Self::Project => s.project_name.clone(),
            Self::App => s.app_name.clone().unwrap_or_else(|| "-".to_string()),
            Self::Assistant => s.display_name.clone(),
            Self::None => String::new(),
        }
    }

    fn group_header(&self, s: &NormalizedSession) -> String {
        match self {
            Self::Project => {
                let branch = s.git_branch.as_deref()
                    .map(|b| format!(" @{}", b))
                    .unwrap_or_default();
                format!("{}{}", s.project_name, branch)
            }
            Self::App => s.app_name.clone().unwrap_or_else(|| "-".to_string()),
            Self::Assistant => s.display_name.clone(),
            Self::None => String::new(),
        }
    }
}

type GroupByState = Arc<Mutex<GroupBy>>;

pub fn setup_tray(
    app: &App,
    monitor: Arc<Monitor>,
    rt: Arc<tokio::runtime::Runtime>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pid_map: SessionPidMap = Arc::new(Mutex::new(HashMap::new()));
    let group_by: GroupByState = Arc::new(Mutex::new(GroupBy::Project));
    let force_update = Arc::new(tokio::sync::Notify::new());

    let tray = build_tray(app, &[], &pid_map, &group_by, &force_update)?;

    let handle = app.handle().clone();
    let tray_id = tray.id().clone();
    let update_group_by = group_by.clone();
    let update_pid_map = pid_map.clone();
    let update_notify = force_update.clone();
    rt.spawn(async move {
        let mut rx = monitor.subscribe();

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let sessions = monitor.snapshot().await;
        let _ = update_tray(&handle, &tray_id, &sessions, &update_pid_map, &update_group_by);

        loop {
            tokio::select! {
                // Normal monitor events — debounce 10s
                result = rx.recv() => {
                    match result {
                        Ok(_) => {
                            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                            while rx.try_recv().is_ok() {}
                            let sessions = monitor.snapshot().await;
                            let _ = update_tray(&handle, &tray_id, &sessions, &update_pid_map, &update_group_by);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                        Err(_) => break,
                    }
                }
                // Forced update (grouping changed) — immediate
                _ = update_notify.notified() => {
                    let sessions = monitor.snapshot().await;
                    let _ = update_tray(&handle, &tray_id, &sessions, &update_pid_map, &update_group_by);
                }
            }
        }
    });

    Ok(())
}

fn build_tray(
    app: &App,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: &GroupByState,
    force_update: &Arc<tokio::sync::Notify>,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let current_group = *group_by.lock().unwrap();
    let menu = build_menu(app, sessions, pid_map, current_group)?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?.to_owned();

    let pid_map = pid_map.clone();
    let group_by = group_by.clone();
    let force_update = force_update.clone();
    let tray = TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip(&build_tooltip(sessions))
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "quit" => app.exit(0),
                _ if GroupBy::from_id(id).is_some() => {
                    let new_group = GroupBy::from_id(id).unwrap();
                    *group_by.lock().unwrap() = new_group;
                    force_update.notify_one();
                }
                _ => {
                    if let Some(&pid) = pid_map.lock().unwrap().get(id) {
                        std::thread::spawn(move || {
                            caw_core::focus::focus_terminal_for_pid(pid);
                        });
                    }
                }
            }
        })
        .build(app)?;

    Ok(tray)
}

fn build_tooltip(sessions: &[NormalizedSession]) -> String {
    if sessions.is_empty() {
        return "caw — no sessions".to_string();
    }
    let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
    let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
    let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();
    format!("caw — {} working, {} waiting, {} idle", working, waiting, idle)
}

fn build_menu(
    handle: &impl Manager<tauri::Wry>,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(handle);
    let mut new_pid_map = HashMap::new();

    if sessions.is_empty() {
        let empty = MenuItemBuilder::with_id("empty", "No active sessions")
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&empty);
    } else {
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();

        let summary = format!("{} working  {} waiting  {} idle", working, waiting, idle);
        let summary_item = MenuItemBuilder::with_id("summary", summary)
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&summary_item).separator();

        if group_by == GroupBy::None {
            let mut sorted: Vec<_> = sessions.iter().collect();
            sorted.sort_by_key(|s| status_ord(&s.status));

            for session in sorted {
                let menu_id = format!("session-{}", session.id);
                let label = format!(
                    "{} {}  {}  {}",
                    session.status.symbol(),
                    session.project_name,
                    session.display_name,
                    session.app_name.as_deref().unwrap_or(""),
                );
                if let Some(pid) = session.pid {
                    new_pid_map.insert(menu_id.clone(), pid);
                }
                let item = MenuItemBuilder::with_id(&menu_id, label).build(handle)?;
                builder = builder.item(&item);
            }
        } else {
            let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
            for session in sessions {
                groups.entry(group_by.group_key(session)).or_default().push(session);
            }

            let mut sorted_groups: Vec<_> = groups.into_iter().collect();
            sorted_groups.sort_by_key(|(_, sessions)| {
                sessions.iter().map(|s| status_ord(&s.status)).min().unwrap_or(3)
            });

            for (i, (_, group_sessions)) in sorted_groups.iter().enumerate() {
                if i > 0 {
                    builder = builder.separator();
                }

                let header_text = group_by.group_header(group_sessions[0]);
                let header = MenuItemBuilder::with_id(
                    format!("group-header-{}", i),
                    header_text,
                )
                .enabled(false)
                .build(handle)?;
                builder = builder.item(&header);

                for session in group_sessions {
                    let menu_id = format!("session-{}", session.id);
                    let label = match group_by {
                        GroupBy::Project => format!(
                            "  {} {}  {}",
                            session.status.symbol(),
                            session.display_name,
                            session.app_name.as_deref().unwrap_or(""),
                        ),
                        GroupBy::App => format!(
                            "  {} {}  {}",
                            session.status.symbol(),
                            session.project_name,
                            session.display_name,
                        ),
                        GroupBy::Assistant => format!(
                            "  {} {}  {}",
                            session.status.symbol(),
                            session.project_name,
                            session.app_name.as_deref().unwrap_or(""),
                        ),
                        GroupBy::None => unreachable!(),
                    };

                    if let Some(pid) = session.pid {
                        new_pid_map.insert(menu_id.clone(), pid);
                    }
                    let item = MenuItemBuilder::with_id(&menu_id, label).build(handle)?;
                    builder = builder.item(&item);
                }
            }
        }
    }

    builder = builder.separator();

    let check = |g: GroupBy| if g == group_by { "  ✓" } else { "" };
    let group_submenu = SubmenuBuilder::with_id(handle, "grouping", "Group by")
        .item(&MenuItemBuilder::with_id("group-project", format!("Project{}", check(GroupBy::Project))).build(handle)?)
        .item(&MenuItemBuilder::with_id("group-app", format!("App{}", check(GroupBy::App))).build(handle)?)
        .item(&MenuItemBuilder::with_id("group-assistant", format!("Assistant{}", check(GroupBy::Assistant))).build(handle)?)
        .separator()
        .item(&MenuItemBuilder::with_id("group-none", format!("None{}", check(GroupBy::None))).build(handle)?)
        .build()?;
    builder = builder.item(&group_submenu);

    builder = builder.separator();
    let quit = MenuItemBuilder::with_id("quit", "Quit caw").build(handle)?;
    builder = builder.item(&quit);

    *pid_map.lock().unwrap() = new_pid_map;

    Ok(builder.build()?)
}

fn status_ord(status: &SessionStatus) -> u8 {
    match status {
        SessionStatus::Working => 0,
        SessionStatus::WaitingInput => 1,
        SessionStatus::Idle => 2,
        SessionStatus::Dead => 3,
    }
}

fn update_tray(
    handle: &AppHandle,
    tray_id: &tauri::tray::TrayIconId,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: &GroupByState,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_group = *group_by.lock().unwrap();
    let menu = build_menu(handle, sessions, pid_map, current_group)?;
    if let Some(tray) = handle.tray_by_id(tray_id) {
        tray.set_menu(Some(menu))?;
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
    Ok(())
}
