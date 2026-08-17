pub mod ollama;
pub mod openai_compatible;

use crate::models::Message;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::{Config, ProviderKind};
use std::sync::Arc;

pub fn from_config(config: &Config) -> Arc<dyn LlmProvider> {
    match config.provider.kind {
        ProviderKind::Ollama => Arc::new(ollama::Ollama::new(config.ollama.url.clone())),
        ProviderKind::OpenAiCompatible => Arc::new(openai_compatible::OpenAiCompatible::new(
            config.openai_compatible.url.clone(),
        )),
    }
}

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
