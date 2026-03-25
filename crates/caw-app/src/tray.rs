use caw_core::{Monitor, NormalizedSession, SessionStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
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
                let branch = s
                    .git_branch
                    .as_deref()
                    .map(|b| format!(" @{}", b))
                    .unwrap_or_default();
                format!("{}{}", s.project_name, branch)
            }
            Self::App => s.app_name.clone().unwrap_or_else(|| "-".to_string()),
            Self::Assistant => s.display_name.clone(),
            Self::None => String::new(),
        }
    }

    fn session_label(&self, s: &NormalizedSession) -> String {
        let msg = truncate_message(s.last_message.as_deref(), 50);
        let msg_part = if msg.is_empty() {
            String::new()
        } else {
            format!(" — {}", msg)
        };

        match self {
            Self::None => format!(
                "{} {}  {}  {}{}",
                s.status.symbol(),
                s.project_name,
                s.display_name,
                s.app_name.as_deref().unwrap_or(""),
                msg_part,
            ),
            Self::Project => format!(
                "  {} {}  {}{}",
                s.status.symbol(),
                s.display_name,
                s.app_name.as_deref().unwrap_or(""),
                msg_part,
            ),
            Self::App => format!(
                "  {} {}  {}{}",
                s.status.symbol(),
                s.project_name,
                s.display_name,
                msg_part,
            ),
            Self::Assistant => format!(
                "  {} {}  {}{}",
                s.status.symbol(),
                s.project_name,
                s.app_name.as_deref().unwrap_or(""),
                msg_part,
            ),
        }
    }

    fn id_char(&self) -> char {
        match self {
            Self::Project => 'P',
            Self::App => 'A',
            Self::Assistant => 'S',
            Self::None => 'N',
        }
    }
}

type GroupByState = Arc<Mutex<GroupBy>>;
type MenuOpenTime = Arc<Mutex<Option<Instant>>>;

/// Holds references to live menu items for in-place text updates
/// without replacing the entire menu (which would close it on macOS).
struct LiveMenu {
    /// Structural fingerprint: session IDs + grouping mode
    fingerprint: String,
    /// Summary line at the top (None when no sessions)
    summary: Option<tauri::menu::MenuItem<tauri::Wry>>,
    /// (session_id, MenuItem) in display order
    sessions: Vec<(String, tauri::menu::MenuItem<tauri::Wry>)>,
}

type LiveMenuState = Arc<Mutex<Option<LiveMenu>>>;

fn compute_fingerprint(sessions: &[NormalizedSession], group_by: GroupBy) -> String {
    let mut ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    format!("{}:{}", group_by.id_char(), ids.join(","))
}

fn is_menu_open(state: &MenuOpenTime) -> bool {
    let mut guard = state.lock().unwrap();
    match *guard {
        Some(t) if t.elapsed() < Duration::from_secs(30) => true,
        Some(_) => {
            // Stale — assume menu was dismissed without firing a menu event
            *guard = None;
            false
        }
        None => false,
    }
}

pub fn setup_tray(
    app: &App,
    monitor: Arc<Monitor>,
    rt: Arc<tokio::runtime::Runtime>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pid_map: SessionPidMap = Arc::new(Mutex::new(HashMap::new()));
    let group_by: GroupByState = Arc::new(Mutex::new(GroupBy::Project));
    let live_menu: LiveMenuState = Arc::new(Mutex::new(None));
    let menu_open_time: MenuOpenTime = Arc::new(Mutex::new(None));
    let rebuild_signal = Arc::new(tokio::sync::Notify::new());

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?.to_owned();

    let current_group = *group_by.lock().unwrap();
    let (menu, initial_live) = build_menu(app, &[], &pid_map, current_group)?;
    *live_menu.lock().unwrap() = Some(initial_live);

    let event_pid_map = pid_map.clone();
    let event_group_by = group_by.clone();
    let event_menu_open = menu_open_time.clone();
    let click_menu_open = menu_open_time.clone();
    let event_rebuild_signal = rebuild_signal.clone();

    let _tray = TrayIconBuilder::with_id("caw-tray")
        .icon(icon)
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .tooltip("caw — coding assistant watcher")
        .on_tray_icon_event(move |_tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                *click_menu_open.lock().unwrap() = Some(Instant::now());
            }
        })
        .on_menu_event(move |app, event| {
            // Any menu-item click means the menu is closing
            *event_menu_open.lock().unwrap() = None;

            let id = event.id().as_ref();
            match id {
                "quit" => app.exit(0),
                _ if GroupBy::from_id(id).is_some() => {
                    *event_group_by.lock().unwrap() = GroupBy::from_id(id).unwrap();
                    // Wake background loop for immediate rebuild with new grouping
                    event_rebuild_signal.notify_one();
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

    // Background task: refresh data every 5s, use in-place text updates
    // when menu structure hasn't changed, full rebuild only when needed
    // and menu is closed.
    let handle = app.handle().clone();
    let bg_monitor = monitor;
    let bg_pid_map = pid_map;
    let bg_group_by = group_by;
    let bg_menu_open = menu_open_time;
    let bg_live_menu = live_menu;
    let bg_signal = rebuild_signal;

    rt.spawn(async move {
        // Initial load
        tokio::time::sleep(Duration::from_secs(1)).await;
        let sessions = bg_monitor.snapshot().await;
        let current_group = *bg_group_by.lock().unwrap();
        if let Ok(live) = rebuild_tray(&handle, &sessions, &bg_pid_map, current_group) {
            *bg_live_menu.lock().unwrap() = Some(live);
        }

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = bg_signal.notified() => {},
            }

            let sessions = bg_monitor.snapshot().await;
            let current_group = *bg_group_by.lock().unwrap();
            let new_fp = compute_fingerprint(&sessions, current_group);
            let menu_open = is_menu_open(&bg_menu_open);

            let needs_structural_rebuild = {
                let guard = bg_live_menu.lock().unwrap();
                guard.as_ref().map_or(true, |m| m.fingerprint != new_fp)
            };

            if needs_structural_rebuild && !menu_open {
                // Structure changed and menu is closed — full rebuild
                if let Ok(live) = rebuild_tray(&handle, &sessions, &bg_pid_map, current_group) {
                    *bg_live_menu.lock().unwrap() = Some(live);
                }
            } else {
                // Either no structural change, or menu is open — update texts in-place
                update_texts_in_place(&bg_live_menu, &sessions, &bg_pid_map, current_group);
                update_tooltip(&handle, &sessions);
            }
        }
    });

    Ok(())
}

fn rebuild_tray(
    handle: &AppHandle,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) -> Result<LiveMenu, Box<dyn std::error::Error>> {
    let (menu, live) = build_menu(handle, sessions, pid_map, group_by)?;
    if let Some(tray) = handle.tray_by_id("caw-tray") {
        let _ = tray.set_menu(Some(menu));
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
    Ok(live)
}

fn update_texts_in_place(
    live_menu: &LiveMenuState,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) {
    let guard = live_menu.lock().unwrap();
    let live = match guard.as_ref() {
        Some(l) => l,
        None => return,
    };

    // Update summary counts
    if let Some(ref summary) = live.summary {
        let working = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Working)
            .count();
        let waiting = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::WaitingInput)
            .count();
        let idle = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Idle)
            .count();
        let _ = summary.set_text(format!(
            "{} working  {} waiting  {} idle",
            working, waiting, idle
        ));
    }

    // Update session item labels (status symbols, etc.)
    let session_map: HashMap<&str, &NormalizedSession> =
        sessions.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut new_pid_map = HashMap::new();

    for (session_id, item) in &live.sessions {
        if let Some(session) = session_map.get(session_id.as_str()) {
            let _ = item.set_text(group_by.session_label(session));
            let menu_id = format!("session-{}", session_id);
            if let Some(pid) = session.pid {
                new_pid_map.insert(menu_id, pid);
            }
        }
    }

    *pid_map.lock().unwrap() = new_pid_map;
}

fn update_tooltip(handle: &AppHandle, sessions: &[NormalizedSession]) {
    if let Some(tray) = handle.tray_by_id("caw-tray") {
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
}

fn build_tooltip(sessions: &[NormalizedSession]) -> String {
    if sessions.is_empty() {
        return "caw — no sessions".to_string();
    }
    let working = sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Working)
        .count();
    let waiting = sessions
        .iter()
        .filter(|s| s.status == SessionStatus::WaitingInput)
        .count();
    let idle = sessions
        .iter()
        .filter(|s| s.status == SessionStatus::Idle)
        .count();
    format!(
        "caw — {} working, {} waiting, {} idle",
        working, waiting, idle
    )
}

fn build_menu(
    handle: &impl Manager<tauri::Wry>,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) -> Result<(tauri::menu::Menu<tauri::Wry>, LiveMenu), Box<dyn std::error::Error>> {
    let fingerprint = compute_fingerprint(sessions, group_by);
    let mut builder = MenuBuilder::new(handle);
    let mut new_pid_map = HashMap::new();
    let mut summary_item = None;
    let mut session_items = Vec::new();

    if sessions.is_empty() {
        let empty = MenuItemBuilder::with_id("empty", "No active sessions")
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&empty);
    } else {
        let working = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Working)
            .count();
        let waiting = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::WaitingInput)
            .count();
        let idle = sessions
            .iter()
            .filter(|s| s.status == SessionStatus::Idle)
            .count();

        let summary = format!("{} working  {} waiting  {} idle", working, waiting, idle);
        let item = MenuItemBuilder::with_id("summary", summary)
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&item).separator();
        summary_item = Some(item);

        if group_by == GroupBy::None {
            let mut sorted: Vec<_> = sessions.iter().collect();
            sorted.sort_by_key(|s| status_ord(&s.status));

            for session in sorted {
                let menu_id = format!("session-{}", session.id);
                let label = group_by.session_label(session);
                if let Some(pid) = session.pid {
                    new_pid_map.insert(menu_id.clone(), pid);
                }
                let item = MenuItemBuilder::with_id(&menu_id, label).build(handle)?;
                builder = builder.item(&item);
                session_items.push((session.id.clone(), item));
            }
        } else {
            let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
            for session in sessions {
                groups
                    .entry(group_by.group_key(session))
                    .or_default()
                    .push(session);
            }

            let mut sorted_groups: Vec<_> = groups.into_iter().collect();
            sorted_groups.sort_by_key(|(_, sessions)| {
                sessions
                    .iter()
                    .map(|s| status_ord(&s.status))
                    .min()
                    .unwrap_or(3)
            });

            for (i, (_, group_sessions)) in sorted_groups.iter().enumerate() {
                if i > 0 {
                    builder = builder.separator();
                }

                let header_text = group_by.group_header(group_sessions[0]);
                let header = MenuItemBuilder::with_id(format!("group-header-{}", i), header_text)
                    .enabled(false)
                    .build(handle)?;
                builder = builder.item(&header);

                for session in group_sessions {
                    let menu_id = format!("session-{}", session.id);
                    let label = group_by.session_label(session);
                    if let Some(pid) = session.pid {
                        new_pid_map.insert(menu_id.clone(), pid);
                    }
                    let item = MenuItemBuilder::with_id(&menu_id, label).build(handle)?;
                    builder = builder.item(&item);
                    session_items.push((session.id.clone(), item));
                }
            }
        }
    }

    builder = builder.separator();

    let check = |g: GroupBy| if g == group_by { "  ✓" } else { "" };
    let group_submenu = SubmenuBuilder::with_id(handle, "grouping", "Group by")
        .item(
            &MenuItemBuilder::with_id(
                "group-project",
                format!("Project{}", check(GroupBy::Project)),
            )
            .build(handle)?,
        )
        .item(
            &MenuItemBuilder::with_id("group-app", format!("App{}", check(GroupBy::App)))
                .build(handle)?,
        )
        .item(
            &MenuItemBuilder::with_id(
                "group-assistant",
                format!("Assistant{}", check(GroupBy::Assistant)),
            )
            .build(handle)?,
        )
        .separator()
        .item(
            &MenuItemBuilder::with_id("group-none", format!("None{}", check(GroupBy::None)))
                .build(handle)?,
        )
        .build()?;
    builder = builder.item(&group_submenu);

    builder = builder.separator();
    let quit = MenuItemBuilder::with_id("quit", "Quit caw").build(handle)?;
    builder = builder.item(&quit);

    *pid_map.lock().unwrap() = new_pid_map;

    let live = LiveMenu {
        fingerprint,
        summary: summary_item,
        sessions: session_items,
    };

    Ok((builder.build()?, live))
}

fn status_ord(status: &SessionStatus) -> u8 {
    match status {
        SessionStatus::Working => 0,
        SessionStatus::WaitingInput => 1,
        SessionStatus::Idle => 2,
        SessionStatus::Dead => 3,
    }
}

fn truncate_message(msg: Option<&str>, max_len: usize) -> String {
    match msg {
        None => String::new(),
        Some(s) => {
            let trimmed = s.trim().replace('\n', " ");
            if trimmed.is_empty() {
                return String::new();
            }
            if trimmed.len() <= max_len {
                trimmed
            } else {
                format!("{}…", &trimmed[..trimmed.floor_char_boundary(max_len)])
            }
        }
    }
}
