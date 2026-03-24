use caw_core::{Monitor, NormalizedSession, SessionStatus};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    App, AppHandle, Manager,
};

/// Map of session menu IDs to PIDs for focus-on-click.
type SessionPidMap = Arc<Mutex<HashMap<String, u32>>>;

pub fn setup_tray(
    app: &App,
    monitor: Arc<Monitor>,
) -> Result<(), Box<dyn std::error::Error>> {
    let pid_map: SessionPidMap = Arc::new(Mutex::new(HashMap::new()));
    let tray = build_tray(app, &[], &pid_map)?;

    let handle = app.handle().clone();
    let tray_id = tray.id().clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut rx = monitor.subscribe();

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let sessions = monitor.snapshot().await;
            let _ = update_tray(&handle, &tray_id, &sessions, &pid_map);

            loop {
                match rx.recv().await {
                    Ok(_) => {
                        let sessions = monitor.snapshot().await;
                        let _ = update_tray(&handle, &tray_id, &sessions, &pid_map);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(_) => break,
                }
            }
        });
    });

    Ok(())
}

fn build_tray(
    app: &App,
    sessions: &[NormalizedSession],
    pid_map: &SessionPidMap,
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = build_menu(app, sessions, pid_map)?;

    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?.to_owned();

    let pid_map = pid_map.clone();
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
                _ => {
                    // Session click → focus the terminal
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
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(handle);
    let mut new_pid_map = HashMap::new();

    if sessions.is_empty() {
        let empty = MenuItemBuilder::with_id("empty", "No active sessions")
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&empty);
    } else {
        // Summary
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();

        let summary = format!("{} working  {} waiting  {} idle", working, waiting, idle);
        let summary_item = MenuItemBuilder::with_id("summary", summary)
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&summary_item).separator();

        // Group by project
        let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
        for session in sessions {
            groups.entry(session.project_name.clone()).or_default().push(session);
        }

        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by_key(|(_, sessions)| {
            sessions.iter().map(|s| status_ord(&s.status)).min().unwrap_or(3)
        });

        for (i, (project_name, group_sessions)) in sorted_groups.iter().enumerate() {
            if i > 0 {
                builder = builder.separator();
            }

            let branch = group_sessions
                .first()
                .and_then(|s| s.git_branch.as_deref())
                .map(|b| format!(" @{}", b))
                .unwrap_or_default();

            let header = MenuItemBuilder::with_id(
                format!("project-{}", project_name),
                format!("{}{}", project_name, branch),
            )
            .enabled(false)
            .build(handle)?;
            builder = builder.item(&header);

            for session in group_sessions {
                let app = session.app_name.as_deref().unwrap_or("");
                let menu_id = format!("session-{}", session.id);

                let label = format!(
                    "  {} {}  {}",
                    session.status.symbol(),
                    session.display_name,
                    app,
                );

                if let Some(pid) = session.pid {
                    new_pid_map.insert(menu_id.clone(), pid);
                }

                let item = MenuItemBuilder::with_id(&menu_id, label)
                    .build(handle)?;
                builder = builder.item(&item);
            }
        }
    }

    builder = builder.separator();
    let quit = MenuItemBuilder::with_id("quit", "Quit caw").build(handle)?;
    builder = builder.item(&quit);

    // Update the shared PID map
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
) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(handle, sessions, pid_map)?;
    if let Some(tray) = handle.tray_by_id(tray_id) {
        tray.set_menu(Some(menu))?;
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
    Ok(())
}
