use caw_core::{Monitor, MonitorEvent};
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

pub async fn run_ws_server(monitor: Arc<Monitor>) {
    let addr: SocketAddr = "127.0.0.1:7272".parse().unwrap();
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind WS server on {}: {}", addr, e);
            return;
        }
    };
    tracing::info!("WebSocket server listening on ws://{}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::debug!("New WS connection from {}", peer);
                let rx = monitor.subscribe();
                let snapshot = monitor.snapshot().await;
                tokio::spawn(handle_connection(stream, rx, snapshot));
            }
            Err(e) => {
                tracing::error!("WS accept error: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    mut rx: broadcast::Receiver<MonitorEvent>,
    initial_snapshot: Vec<caw_core::NormalizedSession>,
) {
    let ws_stream = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            tracing::error!("WS handshake error: {}", e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();

    // Send initial snapshot
    let snapshot_event = MonitorEvent::Snapshot(initial_snapshot);
    if let Ok(json) = serde_json::to_string(&snapshot_event) {
        let _ = write.send(Message::Text(json.into())).await;
    }

    loop {
        tokio::select! {
            // Forward monitor events to client
            Ok(event) = rx.recv() => {
                if let Ok(json) = serde_json::to_string(&event) {
                    if write.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
            // Handle incoming messages (ping/pong, close)
            msg = read.next() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = write.send(Message::Pong(data)).await;
                    }
                    Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}
