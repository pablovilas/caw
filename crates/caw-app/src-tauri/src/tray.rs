use caw_core::{Monitor, NormalizedSession};
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

    // Spawn a task to update the tray menu when sessions change
    let handle = app.handle().clone();
    let tray_id = tray.id().clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            let mut rx = monitor.subscribe();

            // Initial snapshot
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let sessions = monitor.snapshot().await;
            let _ = update_tray_menu(&handle, &tray_id, &sessions);

            loop {
                match rx.recv().await {
                    Ok(_event) => {
                        let sessions = monitor.snapshot().await;
                        let _ = update_tray_menu(&handle, &tray_id, &sessions);
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
        .tooltip("caw — coding assistant watcher")
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "quit" => app.exit(0),
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(tray)
}

fn build_menu(
    handle: &impl Manager<tauri::Wry>,
    sessions: &[NormalizedSession],
) -> Result<tauri::menu::Menu<tauri::Wry>, Box<dyn std::error::Error>> {
    let mut builder = MenuBuilder::new(handle);

    if sessions.is_empty() {
        let empty = MenuItemBuilder::with_id("empty", "No sessions")
            .enabled(false)
            .build(handle)?;
        builder = builder.item(&empty);
    } else {
        for session in sessions {
            let label = format!(
                "{} {} — {}",
                session.status.symbol(),
                session.project_name,
                session.display_name,
            );
            let item = MenuItemBuilder::with_id(
                format!("session-{}", session.id),
                label,
            )
            .build(handle)?;
            builder = builder.item(&item);
        }
    }

    builder = builder.separator();

    let show = MenuItemBuilder::with_id("show", "Show Window").build(handle)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit caw").build(handle)?;
    builder = builder.item(&show).separator().item(&quit);

    Ok(builder.build()?)
}

fn update_tray_menu(
    handle: &AppHandle,
    tray_id: &tauri::tray::TrayIconId,
    sessions: &[NormalizedSession],
) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_menu(handle, sessions)?;
    if let Some(tray) = handle.tray_by_id(tray_id) {
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}
