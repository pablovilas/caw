use caw_core::{NormalizedSession, SessionStatus};
use muda::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use std::collections::HashMap;
use tray_icon::TrayIcon;

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

    fn label(&self) -> &str {
        match self {
            Self::Project => "Project",
            Self::App => "App",
            Self::Assistant => "Assistant",
            Self::None => "None",
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
}

/// Holds references to live menu items for in-place text updates.
struct LiveMenu {
    fingerprint: String,
    summary: Option<MenuItem>,
    sessions: Vec<(String, MenuItem)>,
}

/// All tray state, kept on the main thread.
pub struct TrayState {
    tray_icon: TrayIcon,
    pid_map: HashMap<String, u32>,
    group_by: GroupBy,
    live_menu: Option<LiveMenu>,
    last_sessions: Vec<NormalizedSession>,
}

fn compute_fingerprint(sessions: &[NormalizedSession], group_by: GroupBy) -> String {
    let mut ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    format!("{}:{}", group_by.label(), ids.join(","))
}

fn load_icon() -> tray_icon::Icon {
    let png_data = include_bytes!("../icons/tray.png");
    let img = image::load_from_memory(png_data).expect("failed to decode tray icon PNG");
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    tray_icon::Icon::from_rgba(rgba.into_raw(), w, h).expect("failed to create tray icon")
}

impl TrayState {
    pub fn new() -> Self {
        let icon = load_icon();
        let (menu, live) = build_menu(&[], GroupBy::Project);

        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_icon_as_template(true)
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(true)
            .with_tooltip("caw — coding assistant watcher")
            .build()
            .expect("failed to build tray icon");

        TrayState {
            tray_icon,
            pid_map: HashMap::new(),
            group_by: GroupBy::Project,
            live_menu: Some(live),
            last_sessions: Vec::new(),
        }
    }
}

pub fn handle_menu_event(state: &mut TrayState, event: &muda::MenuEvent) {
    let id = event.id.0.as_str();

    match id {
        "quit" => std::process::exit(0),
        _ if GroupBy::from_id(id).is_some() => {
            state.group_by = GroupBy::from_id(id).unwrap();
            let sessions = state.last_sessions.clone();
            full_rebuild(state, &sessions);
        }
        _ => {
            if let Some(&pid) = state.pid_map.get(id) {
                std::thread::spawn(move || {
                    caw_core::focus::focus_terminal_for_pid(pid);
                });
            }
        }
    }
}

pub fn update_from_snapshot(state: &mut TrayState, sessions: &[NormalizedSession]) {
    state.last_sessions = sessions.to_vec();

    let new_fp = compute_fingerprint(sessions, state.group_by);
    let needs_rebuild = state
        .live_menu
        .as_ref()
        .map_or(true, |m| m.fingerprint != new_fp);

    if needs_rebuild {
        full_rebuild(state, sessions);
    } else {
        update_texts_in_place(state, sessions);
        let tooltip = build_tooltip(sessions);
        let _ = state.tray_icon.set_tooltip(Some(&tooltip));
    }
}

fn full_rebuild(state: &mut TrayState, sessions: &[NormalizedSession]) {
    let (menu, live) = build_menu(sessions, state.group_by);
    state.pid_map = extract_pid_map(sessions);
    state.tray_icon.set_menu(Some(Box::new(menu)));
    let _ = state.tray_icon.set_tooltip(Some(&build_tooltip(sessions)));
    state.live_menu = Some(live);
}

fn extract_pid_map(sessions: &[NormalizedSession]) -> HashMap<String, u32> {
    let mut map = HashMap::new();
    for s in sessions {
        if let Some(pid) = s.pid {
            map.insert(format!("session-{}", s.id), pid);
        }
    }
    map
}

fn update_texts_in_place(state: &mut TrayState, sessions: &[NormalizedSession]) {
    let live = match state.live_menu.as_ref() {
        Some(l) => l,
        None => return,
    };

    if let Some(ref summary) = live.summary {
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();
        let _ = summary.set_text(format!("{} working  {} waiting  {} idle", working, waiting, idle));
    }

    let session_map: HashMap<&str, &NormalizedSession> =
        sessions.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut new_pid_map = HashMap::new();

    for (session_id, item) in &live.sessions {
        if let Some(session) = session_map.get(session_id.as_str()) {
            let _ = item.set_text(state.group_by.session_label(session));
            let menu_id = format!("session-{}", session_id);
            if let Some(pid) = session.pid {
                new_pid_map.insert(menu_id, pid);
            }
        }
    }

    state.pid_map = new_pid_map;
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

fn build_menu(sessions: &[NormalizedSession], group_by: GroupBy) -> (Menu, LiveMenu) {
    let fingerprint = compute_fingerprint(sessions, group_by);
    let menu = Menu::new();
    let mut summary_item = None;
    let mut session_items = Vec::new();

    if sessions.is_empty() {
        let _ = menu.append(&MenuItem::with_id(MenuId::new("empty"), "No active sessions", false, None));
    } else {
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();

        let summary = MenuItem::with_id(
            MenuId::new("summary"),
            format!("{} working  {} waiting  {} idle", working, waiting, idle),
            false,
            None,
        );
        let _ = menu.append(&summary);
        let _ = menu.append(&PredefinedMenuItem::separator());
        summary_item = Some(summary);

        if group_by == GroupBy::None {
            let mut sorted: Vec<_> = sessions.iter().collect();
            sorted.sort_by_key(|s| status_ord(&s.status));

            for session in sorted {
                let menu_id = format!("session-{}", session.id);
                let item = MenuItem::with_id(MenuId::new(&menu_id), group_by.session_label(session), true, None);
                let _ = menu.append(&item);
                session_items.push((session.id.clone(), item));
            }
        } else {
            let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
            for session in sessions {
                groups.entry(group_by.group_key(session)).or_default().push(session);
            }

            let mut sorted_groups: Vec<_> = groups.into_iter().collect();
            sorted_groups.sort_by_key(|(_, g)| g.iter().map(|s| status_ord(&s.status)).min().unwrap_or(3));

            for (i, (_, group_sessions)) in sorted_groups.iter().enumerate() {
                if i > 0 {
                    let _ = menu.append(&PredefinedMenuItem::separator());
                }

                let _ = menu.append(&MenuItem::with_id(
                    MenuId::new(format!("group-header-{}", i)),
                    group_by.group_header(group_sessions[0]),
                    false,
                    None,
                ));

                for session in group_sessions {
                    let menu_id = format!("session-{}", session.id);
                    let item = MenuItem::with_id(MenuId::new(&menu_id), group_by.session_label(session), true, None);
                    let _ = menu.append(&item);
                    session_items.push((session.id.clone(), item));
                }
            }
        }
    }

    let _ = menu.append(&PredefinedMenuItem::separator());

    let check = |g: GroupBy| if g == group_by { format!("{}  \u{2713}", g.label()) } else { g.label().to_string() };
    let group_submenu = Submenu::with_id(MenuId::new("grouping"), "Group by", true);
    let _ = group_submenu.append(&MenuItem::with_id(MenuId::new("group-project"), check(GroupBy::Project), true, None));
    let _ = group_submenu.append(&MenuItem::with_id(MenuId::new("group-app"), check(GroupBy::App), true, None));
    let _ = group_submenu.append(&MenuItem::with_id(MenuId::new("group-assistant"), check(GroupBy::Assistant), true, None));
    let _ = group_submenu.append(&PredefinedMenuItem::separator());
    let _ = group_submenu.append(&MenuItem::with_id(MenuId::new("group-none"), check(GroupBy::None), true, None));
    let _ = menu.append(&group_submenu);

    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(MenuId::new("quit"), "Quit caw", true, None));

    (menu, LiveMenu { fingerprint, summary: summary_item, sessions: session_items })
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
