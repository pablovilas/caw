mod commands;
mod tray;
mod ws;

use caw_core::{Monitor, PluginRegistry};
use caw_plugin_claude::ClaudePlugin;
use caw_plugin_codex::CodexPlugin;
use caw_plugin_opencode::OpenCodePlugin;
use std::sync::Arc;

pub struct AppState {
    pub monitor: Arc<Monitor>,
}

fn build_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(ClaudePlugin::new()));
    registry.register(Arc::new(CodexPlugin::new()));
    registry.register(Arc::new(OpenCodePlugin::new()));
    registry
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("caw=debug")
        .init();

    let registry = build_registry();
    let monitor = Arc::new(Monitor::new(registry));

    // Start WebSocket server
    let ws_monitor = monitor.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(ws::run_ws_server(ws_monitor));
    });

    let state = AppState {
        monitor: monitor.clone(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(|app| {
            tray::setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
