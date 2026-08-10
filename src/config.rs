use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub model: String,
    pub ollama: OllamaConfig,
    pub ui: UiConfig,
    pub assistant: AssistantConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub url: String,
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

impl Default for Config {
    fn default() -> Self {
        Self {
            model: "qwen3:8b".into(),
            ollama: OllamaConfig {
                url: "http://localhost:11434".into(),
            },
            ui: UiConfig {
                sidebar_width: 26,
                show_timestamps: false,
                show_sidebar: true,
                theme: default_theme(),
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
