use super::{LlmProvider, StreamEvent};
use crate::models::Message;
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

/// A local implementation of the OpenAI-compatible API, supported by LM Studio,
/// llama.cpp, LocalAI, and many self-hosted inference servers. No API key is sent.
pub struct OpenAiCompatible {
    client: reqwest::Client,
    base_url: String,
}

impl OpenAiCompatible {
    pub fn new(base_url: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(120))
                .connect_timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            base_url: base_url.trim_end_matches('/').into(),
        }
    }
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    id: String,
}

#[derive(Deserialize)]
struct StreamResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[async_trait]
impl LlmProvider for OpenAiCompatible {
    fn url(&self) -> String {
        self.base_url.clone()
    }

    async fn reachable(&self) -> bool {
        self.client
            .get(format!("{}/models", self.base_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn models(&self) -> Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/models", self.base_url))
            .send()
            .await
            .context("Local OpenAI-compatible server is not reachable")?
            .error_for_status()?;
        let mut models: Vec<_> = response
            .json::<ModelsResponse>()
            .await?
            .data
            .into_iter()
            .map(|model| model.id)
            .collect();
        models.sort();
        Ok(models)
    }

    async fn chat(
        &self,
        model: &str,
        messages: Vec<Message>,
        events: UnboundedSender<StreamEvent>,
    ) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .json(&ChatRequest {
                model,
                messages: messages
                    .iter()
                    .map(|message| WireMessage {
                        role: message.role.as_str(),
                        content: &message.content,
                    })
                    .collect(),
                stream: true,
            })
            .send()
            .await
            .context("Cannot connect to local OpenAI-compatible server")?
            .error_for_status()?;

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = stream.next().await {
            buffer.push_str(&String::from_utf8_lossy(&chunk?));
            while let Some(end) = buffer.find('\n') {
                let line = buffer[..end].trim().to_string();
                buffer.drain(..=end);
                let Some(data) = line.strip_prefix("data:") else {
                    continue;
                };
                let data = data.trim();
                if data == "[DONE]" {
                    let _ = events.send(StreamEvent::Done);
                    return Ok(());
                }
                if let Ok(item) = serde_json::from_str::<StreamResponse>(data) {
                    for choice in item.choices {
                        if let Some(content) = choice.delta.content {
                            let _ = events.send(StreamEvent::Chunk(content));
                        }
                    }
                }
            }
        }
        let _ = events.send(StreamEvent::Done);
        Ok(())
    }
}
