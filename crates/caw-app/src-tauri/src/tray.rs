use caw_core::{Monitor, NormalizedSession, SessionStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    App, AppHandle, Manager,
};

pub fn setup_tray(
    app: &App,
    monitor: Arc<Monitor>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tray = build_tray(app, &[])?;

    let handle = app.handle().clone();
    let tray_id = tray.id().clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut rx = monitor.subscribe();

            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let sessions = monitor.snapshot().await;
            let _ = update_tray(&handle, &tray_id, &sessions);

            loop {
                match rx.recv().await {
                    Ok(_) => {
                        let sessions = monitor.snapshot().await;
                        let _ = update_tray(&handle, &tray_id, &sessions);
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
) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let menu = build_menu(app, sessions)?;

    let tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip(&build_tooltip(sessions))
        .on_menu_event(move |app, event| {
            if event.id().as_ref() == "quit" {
                app.exit(0);
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
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(handle);

    if sessions.is_empty() {
        let empty = MenuItemBuilder::with_id("empty", "No active sessions")
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&empty);
    } else {
        // Summary line
        let working = sessions.iter().filter(|s| s.status == SessionStatus::Working).count();
        let waiting = sessions.iter().filter(|s| s.status == SessionStatus::WaitingInput).count();
        let idle = sessions.iter().filter(|s| s.status == SessionStatus::Idle).count();

        let summary = format!(
            "{} working  {}  waiting  {} idle",
            working, waiting, idle
        );
        let summary_item = MenuItemBuilder::with_id("summary", summary)
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&summary_item).separator();

        // Group by project
        let mut groups: HashMap<String, Vec<&NormalizedSession>> = HashMap::new();
        for session in sessions {
            groups
                .entry(session.project_name.clone())
                .or_default()
                .push(session);
        }

        // Sort groups by best status
        let mut sorted_groups: Vec<_> = groups.into_iter().collect();
        sorted_groups.sort_by_key(|(_, sessions)| {
            sessions.iter().map(|s| status_ord(&s.status)).min().unwrap_or(3)
        });

        for (i, (project_name, group_sessions)) in sorted_groups.iter().enumerate() {
            if i > 0 {
                builder = builder.separator();
            }

            // Project header (disabled, acts as label)
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

            // Session items
            for session in group_sessions {
                let app = session.app_name.as_deref().unwrap_or("");
                let label = format!(
                    "  {} {}  {}",
                    session.status.symbol(),
                    session.display_name,
                    app,
                );
                let item = MenuItemBuilder::with_id(
                    format!("session-{}", session.id),
                    label,
                )
                .build(handle)?;
                builder = builder.item(&item);
            }
        }
    }

    builder = builder.separator();

    let quit = MenuItemBuilder::with_id("quit", "Quit caw").build(handle)?;
    builder = builder.item(&quit);

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
) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(handle, sessions)?;
    if let Some(tray) = handle.tray_by_id(tray_id) {
        tray.set_menu(Some(menu))?;
        let _ = tray.set_tooltip(Some(&build_tooltip(sessions)));
    }
    Ok(())
}
