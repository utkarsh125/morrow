pub mod ollama;

use crate::models::Message;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Chunk(String),
    Done,
    Error(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        model: &str,
        messages: Vec<Message>,
        events: UnboundedSender<StreamEvent>,
    ) -> Result<()>;
    async fn reachable(&self) -> bool;
    async fn models(&self) -> Result<Vec<String>>;
    #[allow(dead_code)]
    fn url(&self) -> String;
}
