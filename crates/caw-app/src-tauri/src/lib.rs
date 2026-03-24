mod tray;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("caw=info")
        .init();

    let rt = Arc::new(tokio::runtime::Runtime::new().expect("failed to create tokio runtime"));

    let monitor = rt.block_on(async {
        let registry = build_registry();
        Arc::new(Monitor::new(registry))
    });

    // Keep the runtime alive in a background thread
    let rt_bg = rt.clone();
    std::thread::spawn(move || {
        rt_bg.block_on(std::future::pending::<()>());
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup({
            let monitor = monitor.clone();
            move |app| {
                // macOS: set as accessory app (menu bar only, no dock icon)
                #[cfg(target_os = "macos")]
                {
                    use tauri::ActivationPolicy;
                    app.set_activation_policy(ActivationPolicy::Accessory);
                }

                tray::setup_tray(app, monitor, rt.clone())?;
                Ok(())
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
