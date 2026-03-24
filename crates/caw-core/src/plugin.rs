use crate::types::{RawInstance, RawSession};
use async_trait::async_trait;
use std::time::Duration;

#[async_trait]
pub trait IPlugin: Send + Sync {
    fn name(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    async fn discover(&self) -> anyhow::Result<Vec<RawInstance>>;
    async fn read_session(&self, id: &str) -> anyhow::Result<Option<RawSession>>;
    fn poll_interval(&self) -> Duration {
        Duration::from_secs(2)
    }
}
