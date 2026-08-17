use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: String,
    #[serde(default)]
    pub provider: ProviderConfig,
    pub ollama: OllamaConfig,
    #[serde(default)]
    pub openai_compatible: OpenAiCompatibleConfig,
    pub ui: UiConfig,
    pub assistant: AssistantConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Ollama,
    OpenAiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    #[serde(default)]
    pub kind: ProviderKind,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            kind: ProviderKind::Ollama,
        }
    }
}

impl Default for ProviderKind {
    fn default() -> Self {
        Self::Ollama
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub url: String,
}

impl Default for OpenAiCompatibleConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:1234/v1".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub sidebar_width: u16,
    #[serde(default = "default_show_timestamps")]
    pub show_timestamps: bool,
    #[serde(default = "default_show_sidebar")]
    pub show_sidebar: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_animations")]
    pub animations: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantConfig {
    pub system_prompt: String,
}

fn default_show_timestamps() -> bool {
    false
}

fn default_show_sidebar() -> bool {
    true
}

fn default_theme() -> String {
    "catppuccin-mocha".into()
}

fn default_animations() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            model: "qwen3:8b".into(),
            provider: ProviderConfig::default(),
            ollama: OllamaConfig {
                url: "http://localhost:11434".into(),
            },
            openai_compatible: OpenAiCompatibleConfig::default(),
            ui: UiConfig {
                sidebar_width: 26,
                show_timestamps: false,
                show_sidebar: true,
                theme: default_theme(),
                animations: true,
            },
            assistant: AssistantConfig {
                system_prompt: "You are Morrow, a helpful, precise, and concise local AI assistant. Format answers using markdown. Provide practical, high-quality responses.".into(),
            },
        }
    }
}

pub fn paths() -> Result<(PathBuf, PathBuf)> {
    let dirs =
        ProjectDirs::from("com", "morrow", "Morrow").context("could not locate home directory")?;
    Ok((
        dirs.config_dir().join("config.toml"),
        dirs.data_dir().join("history.db"),
    ))
}

pub fn load_or_create() -> Result<Config> {
    let (path, _) = paths()?;
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("failed to create config directory")?;
        }
        let value = Config::default();
        fs::write(&path, toml::to_string_pretty(&value)?)?;
        return Ok(value);
    }
    let text = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(toml::from_str(&text).unwrap_or_default())
}

pub fn save(config: &Config) -> Result<()> {
    let (path, _) = paths()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }
    fs::write(path, toml::to_string_pretty(config)?)?;
    Ok(())
}
