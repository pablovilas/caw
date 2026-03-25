#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod tray;
mod ui;

use caw_core::{Monitor, NormalizedSession, PluginRegistry, ProcessScanner};
use caw_plugin_claude::ClaudePlugin;
use caw_plugin_codex::CodexPlugin;
use caw_plugin_opencode::OpenCodePlugin;
use clap::{Parser, Subcommand};
use std::io::IsTerminal;
use std::sync::{mpsc, Arc, Mutex};

#[derive(Parser)]
#[command(name = "caw", about = "coding assistant watcher")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Live interactive dashboard
    Watch,
    /// System tray app
    Tray,
    /// One-line status for shell prompts
    Status,
    /// Headless daemon mode
    Serve,
    /// Debug process discovery
    Debug,
}

fn build_registry() -> PluginRegistry {
    let scanner = Arc::new(Mutex::new(ProcessScanner::new()));
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(ClaudePlugin::new(scanner.clone())));
    registry.register(Arc::new(CodexPlugin::new(scanner.clone())));
    registry.register(Arc::new(OpenCodePlugin::new(scanner)));
    registry
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None => {
            if std::io::stdout().is_terminal() {
                run_tui();
            } else {
                run_tray();
            }
        }
        Some(Command::Watch) => run_tui(),
        Some(Command::Tray) => run_tray(),
        Some(Command::Status) => run_status(),
        Some(Command::Serve) => run_serve(),
        Some(Command::Debug) => {
            caw_plugin_claude::debug::debug_processes();
        }
    }
}

fn run_tui() {
    tracing_subscriber::fmt()
        .with_env_filter("caw=debug")
        .with_writer(std::io::stderr)
        .init();

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let registry = build_registry();
        app::run_tui(registry).await.expect("TUI failed");
    });
}

fn run_status() {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let registry = build_registry();
        let monitor = Monitor::new(registry);
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let sessions = monitor.snapshot().await;

        let working = sessions
            .iter()
            .filter(|s| s.status == caw_core::SessionStatus::Working)
            .count();
        let waiting = sessions
            .iter()
            .filter(|s| s.status == caw_core::SessionStatus::WaitingInput)
            .count();
        let idle = sessions
            .iter()
            .filter(|s| s.status == caw_core::SessionStatus::Idle)
            .count();

        if sessions.is_empty() {
            println!("no sessions");
        } else {
            println!("{}w {}a {}i", working, waiting, idle);
        }
    });
}

fn run_serve() {
    tracing_subscriber::fmt()
        .with_env_filter("caw=info")
        .init();
    tracing::info!("caw daemon starting...");

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async {
        let registry = build_registry();
        let monitor = Monitor::new(registry);
        let mut rx = monitor.subscribe();

        loop {
            match rx.recv().await {
                Ok(event) => {
                    tracing::info!(?event, "monitor event");
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Lagged behind by {} events", n);
                }
                Err(_) => break,
            }
        }
    });
}

fn run_tray() {
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

    // Background task: poll monitor every 5s
    let bg_monitor = monitor.clone();
    rt.spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        loop {
            let sessions = bg_monitor.snapshot().await;
            let _ = snapshot_tx.send(sessions);
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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

        // Process session snapshots from background task (take latest)
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
