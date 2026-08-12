use super::{LlmProvider, StreamEvent};
use crate::models::Message;
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub struct Ollama {
    client: reqwest::Client,
    base_url: String,
}

impl Ollama {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url: base_url.trim_end_matches('/').into(),
        }
    }
}

#[derive(Serialize)]
struct Request<'a> {
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
struct Response {
    message: Option<ResponseMessage>,
    done: Option<bool>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct TagsResponse {
    models: Option<Vec<ModelInfo>>,
}

#[derive(Deserialize)]
struct ModelInfo {
    name: String,
}

#[async_trait]
impl LlmProvider for Ollama {
    fn url(&self) -> String {
        self.base_url.clone()
    }

    async fn reachable(&self) -> bool {
        self.client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    async fn chat(
        &self,
        model: &str,
        messages: Vec<Message>,
        events: UnboundedSender<StreamEvent>,
    ) -> Result<()> {
        let body = Request {
            model,
            messages: messages
                .iter()
                .map(|m| WireMessage {
                    role: m.role.as_str(),
                    content: &m.content,
                })
                .collect(),
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&body)
            .send()
            .await
            .context("Cannot connect to Ollama. Make sure 'ollama serve' is running.")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Ollama error (HTTP {status}): {text}"));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(next) = stream.next().await {
            let bytes = match next {
                Ok(b) => b,
                Err(err) => {
                    let _ = events.send(StreamEvent::Error(format!("Network stream error: {err}")));
                    return Ok(());
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&bytes));
            while let Some(pos) = buffer.find('\n') {
                let line = buffer[..pos].trim().to_string();
                buffer.drain(..=pos);
                if line.is_empty() {
                    continue;
                }

                if let Ok(item) = serde_json::from_str::<Response>(&line) {
                    if let Some(err) = item.error {
                        let _ = events.send(StreamEvent::Error(err));
                        return Ok(());
                    }
                    if let Some(message) = item.message {
                        if !message.content.is_empty() {
                            let _ = events.send(StreamEvent::Chunk(message.content));
                        }
                    }
                    if item.done.unwrap_or(false) {
                        let _ = events.send(StreamEvent::Done);
                        return Ok(());
                    }
                }
            }
        }

        let _ = events.send(StreamEvent::Done);
        Ok(())
    }

    async fn models(&self) -> Result<Vec<String>> {
        let response = self
            .client
            .get(format!("{}/api/tags", self.base_url))
            .timeout(Duration::from_secs(4))
            .send()
            .await
            .context("Ollama is not reachable")?
            .error_for_status()?;

        let tags: TagsResponse = response.json().await?;
        let mut names: Vec<String> = tags
            .models
            .unwrap_or_default()
            .into_iter()
            .map(|model| model.name)
            .collect();
        names.sort();
        Ok(names)
    }
}
