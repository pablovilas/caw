use crate::plugin::IPlugin;
use crate::registry::PluginRegistry;
use crate::types::{MonitorEvent, NormalizedSession};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::warn;

pub struct Monitor {
    sessions: Arc<RwLock<HashMap<String, NormalizedSession>>>,
    tx: broadcast::Sender<MonitorEvent>,
    _rx: broadcast::Receiver<MonitorEvent>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}

impl Monitor {
    pub fn new(registry: PluginRegistry) -> Self {
        let (tx, rx) = broadcast::channel(256);
        let sessions: Arc<RwLock<HashMap<String, NormalizedSession>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let mut tasks = Vec::new();

        for plugin in registry.into_plugins() {
            let tx = tx.clone();
            let sessions = sessions.clone();

            let handle = tokio::spawn(async move {
                Self::poll_plugin(plugin, sessions, tx).await;
            });
            tasks.push(handle);
        }

        Self {
            sessions,
            tx,
            _rx: rx,
            tasks,
        }
    }

    async fn poll_plugin(
        plugin: Arc<dyn IPlugin>,
        sessions: Arc<RwLock<HashMap<String, NormalizedSession>>>,
        tx: broadcast::Sender<MonitorEvent>,
    ) {
        let interval = plugin.poll_interval();
        let plugin_name = plugin.name();
        let display_name = plugin.display_name();

        loop {
            match plugin.discover().await {
                Ok(instances) => {
                    let mut seen_ids = Vec::new();
                    for instance in &instances {
                        seen_ids.push(instance.id.clone());

                        let session = match plugin.read_session(&instance.id).await {
                            Ok(Some(s)) => s,
                            Ok(None) => continue,
                            Err(e) => {
                                warn!("Error reading session {}: {}", instance.id, e);
                                continue;
                            }
                        };

                        let normalized =
                            NormalizedSession::from_raw(instance, &session, plugin_name, display_name);

                        let mut map = sessions.write().await;
                        let key = format!("{}:{}", plugin_name, instance.id);

                        if let Some(existing) = map.get(&key) {
                            if existing.status != normalized.status
                                || existing.last_message != normalized.last_message
                                || existing.token_usage.total() != normalized.token_usage.total()
                            {
                                let _ = tx.send(MonitorEvent::Updated(normalized.clone()));
                            }
                        } else {
                            let _ = tx.send(MonitorEvent::Added(normalized.clone()));
                        }

                        map.insert(key, normalized);
                    }

                    // Remove sessions that are no longer discovered
                    let mut map = sessions.write().await;
                    let prefix = format!("{}:", plugin_name);
                    let to_remove: Vec<String> = map
                        .keys()
                        .filter(|k| k.starts_with(&prefix))
                        .filter(|k| {
                            let id = &k[prefix.len()..];
                            !seen_ids.contains(&id.to_string())
                        })
                        .cloned()
                        .collect();

                    for key in to_remove {
                        let id = key[prefix.len()..].to_string();
                        map.remove(&key);
                        let _ = tx.send(MonitorEvent::Removed {
                            id,
                            plugin: plugin_name.to_string(),
                        });
                    }
                }
                Err(e) => {
                    warn!("Plugin {} discover failed: {}", plugin_name, e);
                }
            }

            tokio::time::sleep(interval).await;
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<MonitorEvent> {
        self.tx.subscribe()
    }

    pub async fn snapshot(&self) -> Vec<NormalizedSession> {
        let map = self.sessions.read().await;
        map.values().cloned().collect()
    }

    pub async fn shutdown(mut self) {
        for task in self.tasks.drain(..) {
            task.abort();
        }
    }
}

impl Drop for Monitor {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}
