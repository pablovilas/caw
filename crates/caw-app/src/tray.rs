use caw_core::{NormalizedSession, SessionStatus};
use muda::{Menu, MenuId, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
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

/// Items that live in the dynamic section of the menu (between separators).
/// These are removed and re-added when grouping changes.
struct DynamicItems {
    /// All items in the dynamic section (headers + sessions + separators between groups)
    items: Vec<MenuItemKind>,
    /// Which items are session items: (session_id, MenuItem)
    sessions: Vec<(String, MenuItem)>,
}

/// All tray state, kept on the main thread (MenuItem is !Send).
pub struct TrayState {
    tray_icon: TrayIcon,
    menu: Menu,
    pid_map: HashMap<String, u32>,
    group_by: GroupBy,
    /// Summary item at top
    summary: Option<MenuItem>,
    /// Dynamic items (sessions + headers) that get replaced on grouping change
    dynamic: DynamicItems,
    /// Position in menu where dynamic items start (after summary + separator)
    dynamic_start: usize,
    /// Group-by submenu items for updating checkmarks
    group_items: GroupMenuItems,
    /// Fingerprint of current session IDs
    session_fingerprint: String,
    /// Cached sessions for immediate rebuild on grouping change
    last_sessions: Vec<NormalizedSession>,
}

struct GroupMenuItems {
    project: MenuItem,
    app: MenuItem,
    assistant: MenuItem,
    none: MenuItem,
}

fn compute_session_fingerprint(sessions: &[NormalizedSession]) -> String {
    let mut ids: Vec<&str> = sessions.iter().map(|s| s.id.as_str()).collect();
    ids.sort();
    ids.join(",")
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
        let menu = Menu::new();
        let group_by = GroupBy::Project;

        // Empty item
        let empty = MenuItem::with_id(MenuId::new("empty"), "No active sessions", false, None);
        let _ = menu.append(&empty);

        // --- separator before bottom items ---
        let _ = menu.append(&PredefinedMenuItem::separator());

        // Group-by submenu with individual items we can update
        let gi_project = MenuItem::with_id(
            MenuId::new("group-project"),
            "Project  \u{2713}",
            true,
            None,
        );
        let gi_app = MenuItem::with_id(MenuId::new("group-app"), "App", true, None);
        let gi_assistant =
            MenuItem::with_id(MenuId::new("group-assistant"), "Assistant", true, None);
        let gi_none = MenuItem::with_id(MenuId::new("group-none"), "None", true, None);

        let group_submenu = Submenu::with_id(MenuId::new("grouping"), "Group by", true);
        let _ = group_submenu.append(&gi_project);
        let _ = group_submenu.append(&gi_app);
        let _ = group_submenu.append(&gi_assistant);
        let _ = group_submenu.append(&PredefinedMenuItem::separator());
        let _ = group_submenu.append(&gi_none);
        let _ = menu.append(&group_submenu);

        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&MenuItem::with_id(MenuId::new("quit"), "Quit caw", true, None));

        let tray_icon = tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_icon_as_template(true)
            .with_menu(Box::new(menu.clone()))
            .with_menu_on_left_click(true)
            .with_tooltip("caw — coding assistant watcher")
            .build()
            .expect("failed to build tray icon");

        TrayState {
            tray_icon,
            menu,
            pid_map: HashMap::new(),
            group_by,
            summary: None,
            dynamic: DynamicItems {
                items: vec![MenuItemKind::MenuItem(empty)],
                sessions: Vec::new(),
            },
            dynamic_start: 0,
            group_items: GroupMenuItems {
                project: gi_project,
                app: gi_app,
                assistant: gi_assistant,
                none: gi_none,
            },
            session_fingerprint: String::new(),
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
            update_group_checkmarks(state);
            // Rebuild dynamic section in-place (menu stays open)
            let sessions = state.last_sessions.clone();
            rebuild_dynamic_section(state, &sessions);
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

    let new_fp = compute_session_fingerprint(sessions);
    let structure_changed = new_fp != state.session_fingerprint;

    if structure_changed {
        // Sessions added/removed — rebuild dynamic section
        rebuild_dynamic_section(state, sessions);
        state.session_fingerprint = new_fp;
    } else {
        // Same sessions — update texts in-place (menu stays open)
        update_texts_in_place(state, sessions);
    }

    let tooltip = build_tooltip(sessions);
    let _ = state.tray_icon.set_tooltip(Some(&tooltip));
}

fn update_group_checkmarks(state: &TrayState) {
    let check = |g: GroupBy| {
        if g == state.group_by {
            format!("{}  \u{2713}", g.label())
        } else {
            g.label().to_string()
        }
    };
    let _ = state.group_items.project.set_text(check(GroupBy::Project));
    let _ = state.group_items.app.set_text(check(GroupBy::App));
    let _ = state
        .group_items
        .assistant
        .set_text(check(GroupBy::Assistant));
    let _ = state.group_items.none.set_text(check(GroupBy::None));
}

impl GroupBy {
    fn label(&self) -> &str {
        match self {
            Self::Project => "Project",
            Self::App => "App",
            Self::Assistant => "Assistant",
            Self::None => "None",
        }
    }
}

/// Remove all dynamic items and re-add them with current grouping.
/// Because we mutate the shared Menu (Rc-based), changes appear
/// on the live NSMenu without closing it.
fn rebuild_dynamic_section(state: &mut TrayState, sessions: &[NormalizedSession]) {
    // Remove old dynamic items + summary from top of menu
    let remove_count = state.dynamic.items.len() + if state.summary.is_some() { 1 } else { 0 };
    for _ in 0..remove_count {
        state.menu.remove_at(0);
    }

    let mut new_dynamic_items: Vec<MenuItemKind> = Vec::new();
    let mut new_session_items: Vec<(String, MenuItem)> = Vec::new();
    let mut new_pid_map = HashMap::new();
    let mut pos = 0; // insert position (top of menu)

    if sessions.is_empty() {
        let empty = MenuItem::with_id(MenuId::new("empty"), "No active sessions", false, None);
        let _ = state.menu.insert(&empty, pos);
        new_dynamic_items.push(MenuItemKind::MenuItem(empty));
        state.summary = None;
    } else {
        // Summary
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
        let summary_text = format!("{} working  {} waiting  {} idle", working, waiting, idle);
        let summary = MenuItem::with_id(MenuId::new("summary"), &summary_text, false, None);
        let _ = state.menu.insert(&summary, pos);
        pos += 1;

        let sep = PredefinedMenuItem::separator();
        let _ = state.menu.insert(&sep, pos);
        pos += 1;
        new_dynamic_items.push(MenuItemKind::Predefined(sep));

        state.summary = Some(summary);

        if state.group_by == GroupBy::None {
            let mut sorted: Vec<_> = sessions.iter().collect();
            sorted.sort_by_key(|s| status_ord(&s.status));

            for session in sorted {
                let menu_id = format!("session-{}", session.id);
                let label = state.group_by.session_label(session);
                if let Some(pid) = session.pid {
                    new_pid_map.insert(menu_id.clone(), pid);
                }
                let item = MenuItem::with_id(MenuId::new(&menu_id), &label, true, None);
                let _ = state.menu.insert(&item, pos);
                pos += 1;
                new_session_items.push((session.id.clone(), item.clone()));
                new_dynamic_items.push(MenuItemKind::MenuItem(item));
            }
        } else {
            let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
            for session in sessions {
                groups
                    .entry(state.group_by.group_key(session))
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
                    let sep = PredefinedMenuItem::separator();
                    let _ = state.menu.insert(&sep, pos);
                    pos += 1;
                    new_dynamic_items.push(MenuItemKind::Predefined(sep));
                }

                let header_text = state.group_by.group_header(group_sessions[0]);
                let header_id = format!("group-header-{}", i);
                let header =
                    MenuItem::with_id(MenuId::new(&header_id), &header_text, false, None);
                let _ = state.menu.insert(&header, pos);
                pos += 1;
                new_dynamic_items.push(MenuItemKind::MenuItem(header));

                for session in group_sessions {
                    let menu_id = format!("session-{}", session.id);
                    let label = state.group_by.session_label(session);
                    if let Some(pid) = session.pid {
                        new_pid_map.insert(menu_id.clone(), pid);
                    }
                    let item = MenuItem::with_id(MenuId::new(&menu_id), &label, true, None);
                    let _ = state.menu.insert(&item, pos);
                    pos += 1;
                    new_session_items.push((session.id.clone(), item.clone()));
                    new_dynamic_items.push(MenuItemKind::MenuItem(item));
                }
            }
        }
    }

    state.dynamic_start = 0;
    state.dynamic = DynamicItems {
        items: new_dynamic_items,
        sessions: new_session_items,
    };
    state.pid_map = new_pid_map;
    state.session_fingerprint = compute_session_fingerprint(sessions);
}

fn update_texts_in_place(state: &mut TrayState, sessions: &[NormalizedSession]) {
    // Update summary counts
    if let Some(ref summary) = state.summary {
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

    // Update session item labels
    let session_map: HashMap<&str, &NormalizedSession> =
        sessions.iter().map(|s| (s.id.as_str(), s)).collect();
    let mut new_pid_map = HashMap::new();

    for (session_id, item) in &state.dynamic.sessions {
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
