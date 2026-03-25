#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tray;
mod ws;

use caw_core::{Monitor, NormalizedSession, PluginRegistry, ProcessScanner};
use caw_plugin_claude::ClaudePlugin;
use caw_plugin_codex::CodexPlugin;
use caw_plugin_opencode::OpenCodePlugin;
use std::sync::{mpsc, Arc, Mutex};

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

    // Hide dock icon on macOS
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSApplication;
        use objc2_app_kit::NSApplicationActivationPolicy;
        use objc2_foundation::MainThreadMarker;
        let mtm = MainThreadMarker::new().expect("must be called on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    }

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("failed to create tokio runtime"));

    let monitor = rt.block_on(async {
        let registry = build_registry();
        Arc::new(Monitor::new(registry))
    });

    // Keep tokio alive in a background thread
    let rt_bg = rt.clone();
    std::thread::spawn(move || {
        rt_bg.block_on(std::future::pending::<()>());
    });

    // Channel for background task to send session snapshots to main thread
    let (snapshot_tx, snapshot_rx) = mpsc::channel::<Vec<NormalizedSession>>();

    // Background task: poll monitor every 5s (or on rebuild signal)
    let bg_monitor = monitor.clone();
    let rebuild_signal = tray::rebuild_signal();
    rt.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        loop {
            let sessions = bg_monitor.snapshot().await;
            let _ = snapshot_tx.send(sessions);

            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
                _ = rebuild_signal.notified() => {},
            }
        }
    });

    // Create tray icon and menu on main thread
    let mut tray_state = tray::TrayState::new();

    // Main event loop
    #[cfg(target_os = "macos")]
    run_macos_event_loop(&mut tray_state, &snapshot_rx);

    #[cfg(not(target_os = "macos"))]
    run_generic_event_loop(&mut tray_state, &snapshot_rx);
}

#[cfg(target_os = "macos")]
fn run_macos_event_loop(
    tray_state: &mut tray::TrayState,
    snapshot_rx: &mpsc::Receiver<Vec<NormalizedSession>>,
) {
    use objc2::rc::Retained;
    use objc2_app_kit::{NSApplication, NSEvent, NSEventMask};
    use objc2_foundation::{MainThreadMarker, NSDate, NSDefaultRunLoopMode};

    let mtm = MainThreadMarker::new().expect("must be on main thread");
    let app = NSApplication::sharedApplication(mtm);

    loop {
        // Drain all pending Cocoa events
        loop {
            let event: Option<Retained<NSEvent>> =
                app.nextEventMatchingMask_untilDate_inMode_dequeue(
                    NSEventMask::Any,
                    Some(&NSDate::distantPast()),
                    unsafe { NSDefaultRunLoopMode },
                    true,
                );
            match event {
                Some(e) => app.sendEvent(&e),
                None => break,
            }
        }

        // Process muda menu events
        if let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            tray::handle_menu_event(tray_state, &event);
        }

        // Process tray icon events
        if let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            tray::handle_tray_event(tray_state, &event);
        }

        // Process session snapshots from background task
        // Take the latest one (drain queue)
        let mut latest_snapshot = None;
        while let Ok(sessions) = snapshot_rx.try_recv() {
            latest_snapshot = Some(sessions);
        }
        if let Some(sessions) = latest_snapshot {
            tray::update_from_snapshot(tray_state, &sessions);
        }

        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(not(target_os = "macos"))]
fn run_generic_event_loop(
    tray_state: &mut tray::TrayState,
    snapshot_rx: &mpsc::Receiver<Vec<NormalizedSession>>,
) {
    loop {
        if let Ok(event) = muda::MenuEvent::receiver().try_recv() {
            tray::handle_menu_event(tray_state, &event);
        }
        if let Ok(event) = tray_icon::TrayIconEvent::receiver().try_recv() {
            tray::handle_tray_event(tray_state, &event);
        }
        let mut latest_snapshot = None;
        while let Ok(sessions) = snapshot_rx.try_recv() {
            latest_snapshot = Some(sessions);
        }
        if let Some(sessions) = latest_snapshot {
            tray::update_from_snapshot(tray_state, &sessions);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
