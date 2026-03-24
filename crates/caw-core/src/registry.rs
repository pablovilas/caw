use crate::plugin::IPlugin;
use std::sync::Arc;

pub struct PluginRegistry {
    plugins: Vec<Arc<dyn IPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register(&mut self, plugin: Arc<dyn IPlugin>) {
        tracing::info!("Registered plugin: {}", plugin.display_name());
        self.plugins.push(plugin);
    }

    pub fn plugins(&self) -> &[Arc<dyn IPlugin>] {
        &self.plugins
    }

    pub fn into_plugins(self) -> Vec<Arc<dyn IPlugin>> {
        self.plugins
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
