# Remove Tauri — Migrate to tray-icon + muda

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Tauri with the same underlying crates it uses (`tray-icon` 0.21 + `muda` 0.17), flatten `caw-app` into a normal Rust crate, and delete the `ui/` directory.

**Architecture:** Plain Rust binary using `tray-icon` for the system tray, `muda` for menus, and a minimal event loop. On macOS, use `objc2-app-kit` to set activation policy (no dock icon). The WebSocket server (`ws.rs`) is preserved for future use. Everything else (caw-core, plugins) is unchanged.

**Tech Stack:** `tray-icon` 0.21, `muda` 0.17, `image` (PNG decoding), `objc2-app-kit` (macOS dock hiding)

---

### Task 1: Flatten caw-app directory structure

Move files out of `src-tauri/` nesting into a standard Rust crate layout. Delete Tauri-specific files, dead code, and the `ui/` directory.

**Files:**
- Delete: `crates/caw-app/src-tauri/build.rs`
- Delete: `crates/caw-app/src-tauri/tauri.conf.json`
- Delete: `crates/caw-app/src-tauri/capabilities/` (entire dir)
- Delete: `crates/caw-app/src-tauri/gen/` (entire dir)
- Delete: `crates/caw-app/src-tauri/src/commands.rs` (dead code)
- Move: `crates/caw-app/src-tauri/Cargo.toml` → `crates/caw-app/Cargo.toml`
- Move: `crates/caw-app/src-tauri/src/` → `crates/caw-app/src/`
- Move: `crates/caw-app/src-tauri/icons/` → `crates/caw-app/icons/`
- Delete: `crates/caw-app/src-tauri/` (now empty)
- Delete: `ui/` (entire directory)
- Modify: `Cargo.toml` (workspace root) — update member path

- [ ] **Step 1: Move files to flattened layout**

```bash
cd /Users/pablo/Projects/caw

# Move the crate contents up one level
cp crates/caw-app/src-tauri/Cargo.toml crates/caw-app/Cargo.toml
cp -r crates/caw-app/src-tauri/src crates/caw-app/src
cp -r crates/caw-app/src-tauri/icons crates/caw-app/icons

# Delete Tauri-specific files and dead code
rm -rf crates/caw-app/src-tauri
rm crates/caw-app/src/commands.rs
rm -rf ui
```

- [ ] **Step 2: Update workspace root Cargo.toml**

Change the member path from `crates/caw-app/src-tauri` to `crates/caw-app`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/caw-core",
    "crates/caw-plugin-claude",
    "crates/caw-plugin-codex",
    "crates/caw-plugin-opencode",
    "crates/caw-cli",
    "crates/caw-app",
]
```

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "refactor(app): flatten caw-app, remove Tauri scaffolding and ui/"
```

---

### Task 2: Replace Tauri dependencies with tray-icon + muda

Rewrite `crates/caw-app/Cargo.toml` to use direct crates instead of Tauri.

**Files:**
- Modify: `crates/caw-app/Cargo.toml`

- [ ] **Step 1: Rewrite Cargo.toml**

```toml
[package]
name = "caw-app"
version = "0.1.0"
edition = "2021"
license = "MIT"

[[bin]]
name = "caw"
path = "src/main.rs"

[dependencies]
caw-core = { path = "../caw-core" }
caw-plugin-claude = { path = "../caw-plugin-claude" }
caw-plugin-codex = { path = "../caw-plugin-codex" }
caw-plugin-opencode = { path = "../caw-plugin-opencode" }
tray-icon = "0.21"
muda = "0.17"
image = { version = "0.25", default-features = false, features = ["png"] }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tokio-tungstenite = "0.26"
futures-util = "0.3"

[target.'cfg(target_os = "macos")'.dependencies]
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSRunningApplication"] }
objc2-foundation = "0.3"
```

- [ ] **Step 2: Verify it parses**

```bash
cargo metadata --no-deps -q > /dev/null
```

- [ ] **Step 3: Commit**

```bash
git add crates/caw-app/Cargo.toml
git commit -m "build(app): replace Tauri deps with tray-icon + muda"
```

---

### Task 3: Rewrite main.rs — event loop without Tauri

Replace `tauri::Builder` with a plain event loop that pumps `muda::MenuEvent` and `tray_icon::TrayIconEvent`.

**Files:**
- Rewrite: `crates/caw-app/src/main.rs`
- Delete: `crates/caw-app/src/lib.rs` (merge into main.rs — no need for separate lib)

- [ ] **Step 1: Write main.rs**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod ws;

use caw_core::{Monitor, PluginRegistry, ProcessScanner};
use caw_plugin_claude::ClaudePlugin;
use caw_plugin_codex::CodexPlugin;
use caw_plugin_opencode::OpenCodePlugin;
use std::sync::{Arc, Mutex};

fn build_registry() -> PluginRegistry {
    let scanner = Arc::new(Mutex::new(ProcessScanner::new()));
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(ClaudePlugin::new(scanner.clone())));
    registry.register(Arc::new(CodexPlugin::new(scanner.clone())));
    registry.register(Arc::new(OpenCodePlugin::new(scanner)));
    registry
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("caw=info")
        .init();

    #[cfg(target_os = "macos")]
    macos_hide_dock_icon();

    let rt = Arc::new(
        tokio::runtime::Runtime::new().expect("failed to create tokio runtime"),
    );

    let monitor = rt.block_on(async {
        let registry = build_registry();
        Arc::new(Monitor::new(registry))
    });

    // Keep tokio runtime alive in a background thread
    let rt_bg = rt.clone();
    std::thread::spawn(move || {
        rt_bg.block_on(std::future::pending::<()>());
    });

    let _tray = tray::setup_tray(monitor, rt);

    // Run the native event loop — pumps menu and tray events
    // On macOS this drives NSApplication; on other platforms it's a
    // simple polling loop.
    #[cfg(target_os = "macos")]
    macos_run_event_loop();

    #[cfg(not(target_os = "macos"))]
    non_macos_event_loop();
}

#[cfg(target_os = "macos")]
fn macos_hide_dock_icon() {
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    let app = unsafe { NSApplication::sharedApplication() };
    unsafe {
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }
}

#[cfg(target_os = "macos")]
fn macos_run_event_loop() {
    use objc2_app_kit::NSApplication;
    let app = unsafe { NSApplication::sharedApplication() };
    unsafe { app.run() };
}

#[cfg(not(target_os = "macos"))]
fn non_macos_event_loop() {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
```

- [ ] **Step 2: Delete lib.rs**

```bash
rm crates/caw-app/src/lib.rs
```

- [ ] **Step 3: Verify it compiles (will fail on tray.rs — that's expected)**

```bash
cargo check -p caw-app 2>&1 | head -5
```

Expected: errors in `tray.rs` because it still uses Tauri types.

- [ ] **Step 4: Commit**

```bash
git add crates/caw-app/src/main.rs
git rm crates/caw-app/src/lib.rs
git commit -m "refactor(app): plain Rust event loop, no Tauri"
```

---

### Task 4: Rewrite tray.rs using tray-icon + muda

Port all tray/menu logic from Tauri wrappers to the underlying crates directly. Preserves all existing behavior: in-place text updates via `MenuItem::set_text()`, `LiveMenu` struct, fingerprint-based structural rebuild, `Notify` for grouping changes, truncated last-message display.

**Files:**
- Rewrite: `crates/caw-app/src/tray.rs`

Key API mapping:
- `tauri::menu::MenuBuilder` → `muda::Menu::new()` + `.append()`
- `tauri::menu::MenuItemBuilder::with_id(id, text)` → `muda::MenuItem::with_id(MenuId::new(id), text)`
- `tauri::menu::SubmenuBuilder` → `muda::Submenu::with_id()`
- `tauri::tray::TrayIconBuilder` → `tray_icon::TrayIconBuilder::new()`
- `item.set_text(text)` → `item.set_text(text)` (same API)
- `tray.set_menu(Some(menu))` → `tray.set_menu(Some(Box::new(menu)))`
- `tray.set_tooltip(Some(text))` → `tray.set_tooltip(Some(text))`
- Menu events: `muda::MenuEvent::receiver()` channel (polled in background)
- Tray events: `tray_icon::TrayIconEvent::receiver()` channel (polled in background)
- Icon: `tray_icon::Icon::from_rgba(rgba_data, width, height)` — use `image` crate to decode PNG

- [ ] **Step 1: Write the new tray.rs**

```rust
use caw_core::{Monitor, NormalizedSession, SessionStatus};
use muda::{MenuEvent, MenuId, MenuItem, Menu, Submenu};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

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
    fingerprint: String,
    summary: Option<MenuItem>,
    sessions: Vec<(String, MenuItem)>,
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
            *guard = None;
            false
        }
        None => false,
    }
}

fn load_icon() -> tray_icon::Icon {
    let png_bytes = include_bytes!("../icons/tray.png");
    let img = image::load_from_memory(png_bytes)
        .expect("failed to load tray icon")
        .into_rgba8();
    let (w, h) = img.dimensions();
    tray_icon::Icon::from_rgba(img.into_raw(), w, h)
        .expect("failed to create tray icon")
}

/// Sets up the tray icon and spawns background tasks. Returns the TrayIcon
/// handle (must be kept alive for the tray to remain visible).
pub fn setup_tray(
    monitor: Arc<Monitor>,
    rt: Arc<tokio::runtime::Runtime>,
) -> TrayIcon {
    let pid_map: SessionPidMap = Arc::new(Mutex::new(HashMap::new()));
    let group_by: GroupByState = Arc::new(Mutex::new(GroupBy::Project));
    let live_menu: LiveMenuState = Arc::new(Mutex::new(None));
    let menu_open_time: MenuOpenTime = Arc::new(Mutex::new(None));
    let rebuild_signal = Arc::new(tokio::sync::Notify::new());

    let current_group = *group_by.lock().unwrap();
    let (menu, initial_live) = build_menu(&[], &pid_map, current_group);

    *live_menu.lock().unwrap() = Some(initial_live);

    let icon = load_icon();

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_icon_as_template(true)
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(true)
        .with_tooltip("caw — coding assistant watcher")
        .build()
        .expect("failed to build tray icon");

    // Spawn menu-event handler thread
    let ev_pid_map = pid_map.clone();
    let ev_group_by = group_by.clone();
    let ev_menu_open = menu_open_time.clone();
    let ev_rebuild = rebuild_signal.clone();
    std::thread::spawn(move || {
        let menu_rx = MenuEvent::receiver();
        let tray_rx = TrayIconEvent::receiver();

        loop {
            // Check for menu events (non-blocking)
            if let Ok(event) = menu_rx.try_recv() {
                *ev_menu_open.lock().unwrap() = None;
                let id = event.id().0.as_str();

                match id {
                    "quit" => std::process::exit(0),
                    _ if GroupBy::from_id(id).is_some() => {
                        *ev_group_by.lock().unwrap() = GroupBy::from_id(id).unwrap();
                        ev_rebuild.notify_one();
                    }
                    _ => {
                        if let Some(&pid) = ev_pid_map.lock().unwrap().get(id) {
                            std::thread::spawn(move || {
                                caw_core::focus::focus_terminal_for_pid(pid);
                            });
                        }
                    }
                }
            }

            // Check for tray icon events (click = menu opening)
            if let Ok(TrayIconEvent::Click { .. }) = tray_rx.try_recv() {
                *ev_menu_open.lock().unwrap() = Some(Instant::now());
            }

            std::thread::sleep(Duration::from_millis(50));
        }
    });

    // Background task: refresh data periodically
    let bg_tray_id = tray.id().clone();
    let bg_pid_map = pid_map;
    let bg_group_by = group_by;
    let bg_menu_open = menu_open_time;
    let bg_live_menu = live_menu;
    let bg_signal = rebuild_signal;

    rt.spawn(async move {
        // Initial load
        tokio::time::sleep(Duration::from_secs(1)).await;
        let sessions = monitor.snapshot().await;
        let current_group = *bg_group_by.lock().unwrap();
        rebuild_tray(&bg_tray_id, &sessions, &bg_pid_map, current_group, &bg_live_menu);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = bg_signal.notified() => {},
            }

            let sessions = monitor.snapshot().await;
            let current_group = *bg_group_by.lock().unwrap();
            let new_fp = compute_fingerprint(&sessions, current_group);
            let menu_open = is_menu_open(&bg_menu_open);

            let needs_structural_rebuild = {
                let guard = bg_live_menu.lock().unwrap();
                guard.as_ref().map_or(true, |m| m.fingerprint != new_fp)
            };

            if needs_structural_rebuild && !menu_open {
                rebuild_tray(&bg_tray_id, &sessions, &bg_pid_map, current_group, &bg_live_menu);
            } else {
                update_texts_in_place(&bg_live_menu, &sessions, &bg_pid_map, current_group);
                update_tooltip(&bg_tray_id, &sessions);
            }
        }
    });

    tray
}

fn rebuild_tray(
    tray_id: &tray_icon::TrayIconId,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
    live_menu: &LiveMenuState,
) {
    let (menu, live) = build_menu(sessions, pid_map, group_by);
    // tray-icon requires accessing the tray from the main thread on macOS,
    // but set_menu/set_tooltip are thread-safe in tray-icon 0.21+
    if let Some(tray) = tray_icon::TrayIcon::with_id(tray_id, |t| {
        let _ = t.set_menu(Some(Box::new(menu)));
        let _ = t.set_tooltip(Some(build_tooltip(sessions)));
    }) {
        // tray_icon doesn't have with_id — we store the TrayIcon handle instead
        // This is handled differently, see note below
    }
    *live_menu.lock().unwrap() = Some(live);
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

    if let Some(ref summary) = live.summary {
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();
        let _ = summary.set_text(format!(
            "{} working  {} waiting  {} idle",
            working, waiting, idle
        ));
    }

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

fn update_tooltip(tray_id: &tray_icon::TrayIconId, sessions: &[NormalizedSession]) {
    // Tooltip update is deferred to rebuild_tray since we need the TrayIcon handle
    // This is a no-op for in-place updates; tooltip updates on next full rebuild
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
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
    group_by: GroupBy,
) -> (Menu, LiveMenu) {
    let fingerprint = compute_fingerprint(sessions, group_by);
    let menu = Menu::new();
    let mut new_pid_map = HashMap::new();
    let mut summary_item = None;
    let mut session_items = Vec::new();

    if sessions.is_empty() {
        let empty = MenuItem::with_id(MenuId::new("empty"), "No active sessions");
        empty.set_enabled(false);
        let _ = menu.append(&empty);
    } else {
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();

        let summary_text = format!("{} working  {} waiting  {} idle", working, waiting, idle);
        let item = MenuItem::with_id(MenuId::new("summary"), &summary_text);
        item.set_enabled(false);
        let _ = menu.append(&item);
        let _ = menu.append(&muda::PredefinedMenuItem::separator());
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
                let item = MenuItem::with_id(MenuId::new(&menu_id), &label);
                let _ = menu.append(&item);
                session_items.push((session.id.clone(), item));
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
                    let _ = menu.append(&muda::PredefinedMenuItem::separator());
                }

                let header_text = group_by.group_header(group_sessions[0]);
                let header = MenuItem::with_id(
                    MenuId::new(&format!("group-header-{}", i)),
                    &header_text,
                );
                header.set_enabled(false);
                let _ = menu.append(&header);

                for session in group_sessions {
                    let menu_id = format!("session-{}", session.id);
                    let label = group_by.session_label(session);
                    if let Some(pid) = session.pid {
                        new_pid_map.insert(menu_id.clone(), pid);
                    }
                    let item = MenuItem::with_id(MenuId::new(&menu_id), &label);
                    let _ = menu.append(&item);
                    session_items.push((session.id.clone(), item));
                }
            }
        }
    }

    let _ = menu.append(&muda::PredefinedMenuItem::separator());

    let check = |g: GroupBy| if g == group_by { "  ✓" } else { "" };
    let group_submenu = Submenu::with_id(MenuId::new("grouping"), "Group by");
    let _ = group_submenu.append(&MenuItem::with_id(
        MenuId::new("group-project"),
        &format!("Project{}", check(GroupBy::Project)),
    ));
    let _ = group_submenu.append(&MenuItem::with_id(
        MenuId::new("group-app"),
        &format!("App{}", check(GroupBy::App)),
    ));
    let _ = group_submenu.append(&MenuItem::with_id(
        MenuId::new("group-assistant"),
        &format!("Assistant{}", check(GroupBy::Assistant)),
    ));
    let _ = group_submenu.append(&muda::PredefinedMenuItem::separator());
    let _ = group_submenu.append(&MenuItem::with_id(
        MenuId::new("group-none"),
        &format!("None{}", check(GroupBy::None)),
    ));
    let _ = menu.append(&group_submenu);

    let _ = menu.append(&muda::PredefinedMenuItem::separator());
    let _ = menu.append(&MenuItem::with_id(MenuId::new("quit"), "Quit caw"));

    *pid_map.lock().unwrap() = new_pid_map;

    let live = LiveMenu {
        fingerprint,
        summary: summary_item,
        sessions: session_items,
    };

    (menu, live)
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
```

**Important:** The `tray-icon` crate doesn't have a `with_id` lookup. The `TrayIcon` handle must be shared directly. See Task 5 for the fix.

- [ ] **Step 2: Commit**

```bash
git add crates/caw-app/src/tray.rs
git commit -m "refactor(app): rewrite tray using tray-icon + muda directly"
```

---

### Task 5: Fix TrayIcon handle sharing

`tray-icon` doesn't have `TrayIcon::with_id()` like Tauri. The `TrayIcon` handle needs to be stored in an `Arc` and shared with the background task for `set_menu`/`set_tooltip` calls. On macOS, `TrayIcon` is `!Send` — tray mutations must happen on the main thread. Use a channel to send rebuild requests to the main thread.

**Files:**
- Modify: `crates/caw-app/src/tray.rs` — store `TrayIcon` in main thread, use channel for mutations
- Modify: `crates/caw-app/src/main.rs` — pump tray update channel in event loop

- [ ] **Step 1: Add TrayUpdate channel**

In `tray.rs`, define a channel for tray mutations:

```rust
pub enum TrayUpdate {
    SetMenu(Box<Menu>),
    SetTooltip(String),
    SetMenuAndTooltip(Box<Menu>, String),
}
```

Modify `setup_tray` to return `(TrayIcon, std::sync::mpsc::Receiver<TrayUpdate>)`.

The background task sends `TrayUpdate` messages instead of calling `tray.set_menu()` directly. The main thread event loop receives and applies them.

- [ ] **Step 2: Update main.rs event loop**

```rust
#[cfg(target_os = "macos")]
fn macos_run_event_loop(tray: TrayIcon, tray_rx: std::sync::mpsc::Receiver<tray::TrayUpdate>) {
    use objc2_app_kit::NSApplication;
    use objc2_foundation::MainThreadMarker;

    let app = NSApplication::sharedApplication(MainThreadMarker::new().unwrap());

    // Pump tray updates periodically using a timer
    std::thread::spawn(move || {
        loop {
            match tray_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(update) => match update {
                    tray::TrayUpdate::SetMenu(menu) => { let _ = tray.set_menu(Some(menu)); }
                    tray::TrayUpdate::SetTooltip(text) => { let _ = tray.set_tooltip(Some(&text)); }
                    tray::TrayUpdate::SetMenuAndTooltip(menu, text) => {
                        let _ = tray.set_menu(Some(menu));
                        let _ = tray.set_tooltip(Some(&text));
                    }
                },
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });

    unsafe { app.run() };
}
```

Note: On macOS, `TrayIcon` operations dispatch to the main thread internally in `tray-icon` 0.21, so sending from a background thread is safe.

- [ ] **Step 3: Build and verify**

```bash
cargo build -p caw-app
```

Expected: compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add crates/caw-app/src/
git commit -m "fix(app): share TrayIcon handle via channel for thread-safe updates"
```

---

### Task 6: Clean up and verify

Remove leftover files, update `.gitignore` if needed, and verify the app runs.

**Files:**
- Verify: no Tauri references remain in the codebase
- Clean: remove any leftover Tauri artifacts from Cargo.lock

- [ ] **Step 1: Verify no Tauri imports remain**

```bash
grep -r "tauri" crates/caw-app/src/ --include="*.rs"
```

Expected: no matches.

- [ ] **Step 2: Clean build**

```bash
cargo build -p caw-app
```

Expected: compiles successfully.

- [ ] **Step 3: Run the app briefly**

```bash
cargo run -p caw-app &
sleep 3
kill %1
```

Expected: tray icon appears in menu bar, no panics.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "chore(app): remove Tauri from Cargo.lock, final cleanup"
```
