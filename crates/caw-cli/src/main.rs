mod app;
mod ui;

use caw_core::PluginRegistry;
use caw_plugin_claude::ClaudePlugin;
use caw_plugin_codex::CodexPlugin;
use caw_plugin_opencode::OpenCodePlugin;
use clap::{Parser, Subcommand};
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "caw", about = "coding assistant watcher")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Interactive TUI dashboard
    Tui,
    /// One-line status for shell prompts
    Status,
    /// Headless daemon mode
    Serve,
    /// Debug process discovery
    Debug,
}

fn build_registry() -> PluginRegistry {
    let mut registry = PluginRegistry::new();
    registry.register(Arc::new(ClaudePlugin::new()));
    registry.register(Arc::new(CodexPlugin::new()));
    registry.register(Arc::new(OpenCodePlugin::new()));
    registry
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Command::Tui) => {
            tracing_subscriber::fmt()
                .with_env_filter("caw=debug")
                .with_writer(std::io::stderr)
                .init();
            let registry = build_registry();
            app::run_tui(registry).await?;
        }
        Some(Command::Status) => {
            let registry = build_registry();
            let monitor = caw_core::Monitor::new(registry);
            // Wait for first discovery cycle
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
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
        }
        Some(Command::Debug) => {
            caw_plugin_claude::debug::debug_processes();
        }
        Some(Command::Serve) => {
            tracing_subscriber::fmt()
                .with_env_filter("caw=info")
                .init();
            tracing::info!("caw daemon starting...");
            let registry = build_registry();
            let monitor = caw_core::Monitor::new(registry);
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
        }
    }

    Ok(())
}
