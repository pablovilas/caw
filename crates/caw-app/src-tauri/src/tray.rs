use caw_core::{Monitor, NormalizedSession, SessionStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::TrayIconBuilder,
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
type MenuOpenState = Arc<std::sync::atomic::AtomicBool>;

pub fn setup_tray(
    app: &App,
    monitor: Arc<Monitor>,
    rt: Arc<tokio::runtime::Runtime>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pid_map: SessionPidMap = Arc::new(Mutex::new(HashMap::new()));
    let group_by: GroupByState = Arc::new(Mutex::new(GroupBy::Project));

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?.to_owned();
    let menu_open: MenuOpenState = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let current_group = *group_by.lock().unwrap();
    let menu = build_menu(app, &[], &pid_map, current_group)?;

    let event_pid_map = pid_map.clone();
    let event_group_by = group_by.clone();
    let event_menu_open = menu_open.clone();
    let click_menu_open = menu_open.clone();

    let _tray = TrayIconBuilder::with_id("caw-tray")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("caw — coding assistant watcher")
        .on_tray_icon_event(move |_tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                click_menu_open.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        })
        .on_menu_event(move |app, event| {
            // Any menu event means the menu just closed
            event_menu_open.store(false, std::sync::atomic::Ordering::Relaxed);

            let id = event.id().as_ref();
            match id {
                "quit" => app.exit(0),
                _ if GroupBy::from_id(id).is_some() => {
                    *event_group_by.lock().unwrap() = GroupBy::from_id(id).unwrap();
                }
                _ => {
                    if let Some(&pid) = event_pid_map.lock().unwrap().get(id) {
                        std::thread::spawn(move || {
                            caw_core::focus::focus_terminal_for_pid(pid);
                        });
                    }
                }
            }
        })
        .build(app)?;

    // Background task: update menu every 5s, but SKIP if menu is open
    let handle = app.handle().clone();
    let bg_monitor = monitor;
    let bg_pid_map = pid_map;
    let bg_group_by = group_by;
    let bg_menu_open = menu_open;
    rt.spawn(async move {
        // Initial load
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let sessions = bg_monitor.snapshot().await;
        let current_group = *bg_group_by.lock().unwrap();
        let _ = rebuild_tray(&handle, &sessions, &bg_pid_map, current_group);

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            // Skip rebuild if menu is currently open
            if bg_menu_open.load(std::sync::atomic::Ordering::Relaxed) {
                continue;
            }

            let sessions = bg_monitor.snapshot().await;
            let current_group = *bg_group_by.lock().unwrap();
            let _ = rebuild_tray(&handle, &sessions, &bg_pid_map, current_group);
        }
    });

    Ok(())
}

fn rebuild_tray(
    handle: &AppHandle,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(handle, sessions, pid_map, group_by)?;
    if let Some(tray) = handle.tray_by_id("caw-tray") {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
    Ok(())
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
