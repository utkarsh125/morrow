use crate::{
    commands::{self, Command, CommandSpec},
    config::{self, Config},
    db::Database,
    models::{Conversation, Message, Role, estimate_tokens, title_from},
    providers::{LlmProvider, StreamEvent},
    theme::{THEMES, Theme, search_themes},
};
use anyhow::Result;
use chrono::Utc;
use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::Arc,
    time::Instant,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Connection {
    Connected,
    Generating,
    Disconnected,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Modal {
    None,
    Help,
    History,
    Models,
    Themes,
    Rename,
    SystemPrompt,
    Stats,
    ConfirmDeleteAll,
}

pub struct App {
    pub config: Config,
    pub db: Database,
    pub conversations: Vec<Conversation>,
    pub selected: usize,
    pub current: Option<Uuid>,
    pub messages: Vec<Message>,

    // Input state
    pub input: String,
    pub cursor: usize,
    pub input_history: Vec<String>,
    pub input_history_idx: Option<usize>,

    // View state
    pub scroll: u16,
    pub connection: Connection,
    pub modal: Modal,
    pub modal_input: String,
    pub modal_cursor: usize,
    pub modal_selected: usize,
    pub modal_scroll: u16,
    pub rename_target: Option<Uuid>,

    // Search queries in modals
    pub search_query: String,

    // Autocomplete popup
    pub autocomplete_active: bool,
    pub autocomplete_idx: usize,
    pub autocomplete_items: Vec<&'static CommandSpec>,

    // Status and notices
    pub notice: Option<(String, Instant)>,
    pub should_quit: bool,
    pub dirty: bool,
    pub animation_frame: u8,
    pub generation_start: Option<Instant>,
    pub generated_tokens: usize,

    // Model selection
    pub model_options: Vec<String>,
    pub model_selected: usize,

    // Theme selection
    pub theme_selected: usize,

    // Temporary chat mode
    pub temporary_mode: bool,
    pub temporary_conversations: HashSet<Uuid>,
    pub temporary_messages: HashMap<Uuid, Vec<Message>>,

    // Provider & streaming
    pub provider: Arc<dyn LlmProvider>,
    pub events: UnboundedReceiver<StreamEvent>,
    pub partial: String,
    pub abort_handle: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub async fn new(config: Config, db: Database, provider: Arc<dyn LlmProvider>) -> Result<Self> {
        let conversations = db.conversations()?;
        let current = conversations.first().map(|c| c.id);
        let messages = match current {
            Some(id) => db.messages(id)?,
            None => vec![],
        };

        let reachable = provider.reachable().await;
        let connection = if reachable {
            Connection::Connected
        } else {
            Connection::Disconnected
        };

        let mut config = config;
        let model_options = if reachable {
            provider.models().await.unwrap_or_default()
        } else {
            Vec::new()
        };

        if !model_options.is_empty() {
            if !model_options.contains(&config.model) {
                config.model = model_options[0].clone();
            }
        }

        let model_selected = model_options
            .iter()
            .position(|name| name == &config.model)
            .unwrap_or(0);

        let theme_selected = THEMES
            .iter()
            .position(|t| t.id == config.ui.theme)
            .unwrap_or(0);

        let notice = if model_options.is_empty() {
            Some((
                "No models detected, make sure Ollama is running (`ollama serve`).".into(),
                Instant::now(),
            ))
        } else {
            None
        };

        Ok(Self {
            config,
            db,
            conversations,
            selected: 0,
            current,
            messages,
            input: String::new(),
            cursor: 0,
            input_history: Vec::new(),
            input_history_idx: None,
            scroll: 0,
            connection,
            modal: Modal::None,
            modal_input: String::new(),
            modal_cursor: 0,
            modal_selected: 0,
            modal_scroll: 0,
            rename_target: None,
            search_query: String::new(),
            autocomplete_active: false,
            autocomplete_idx: 0,
            autocomplete_items: Vec::new(),
            notice,
            should_quit: false,
            dirty: true,
            animation_frame: 0,
            generation_start: None,
            generated_tokens: 0,
            model_options,
            model_selected,
            theme_selected,
            temporary_mode: false,
            temporary_conversations: HashSet::new(),
            temporary_messages: HashMap::new(),
            provider,
            events: mpsc::unbounded_channel().1,
            partial: String::new(),
            abort_handle: None,
        })
    }

    pub fn current_theme(&self) -> &'static Theme {
        Theme::from_name(&self.config.ui.theme)
    }

    pub fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), Instant::now()));
        self.dirty = true;
    }

    pub fn active_notice(&self) -> Option<&str> {
        if let Some((msg, created)) = &self.notice {
            if created.elapsed().as_secs() < 8 {
                return Some(msg.as_str());
            }
        }
        None
    }

    pub fn total_tokens_in_current_chat(&self) -> usize {
        let mut count = estimate_tokens(&self.config.assistant.system_prompt);
        for m in &self.messages {
            count += estimate_tokens(&m.content);
        }
        count
    }

    pub fn new_conversation(&mut self) -> Result<()> {
        let now = Utc::now().timestamp();
        let c = Conversation {
            id: Uuid::new_v4(),
            title: "New conversation".into(),
            created_at: now,
            updated_at: now,
        };

        if !self.temporary_mode {
            self.db.create_conversation(&c)?;
        } else {
            self.temporary_conversations.insert(c.id);
            self.temporary_messages.insert(c.id, Vec::new());
        }

        self.conversations.insert(0, c.clone());
        self.selected = 0;
        self.current = Some(c.id);
        self.messages.clear();
        self.scroll = 0;
        self.dirty = true;
        self.set_notice(if self.temporary_mode {
            "Started new temporary session (ephemeral)"
        } else {
            "Started new conversation"
        });
        Ok(())
    }

    pub fn select(&mut self, index: usize) -> Result<()> {
        if let Some(c) = self.conversations.get(index) {
            self.selected = index;
            self.current = Some(c.id);
            let is_temp = self.temporary_conversations.contains(&c.id);
            self.temporary_mode = is_temp;
            self.messages = if is_temp {
                self.temporary_messages
                    .get(&c.id)
                    .cloned()
                    .unwrap_or_default()
            } else {
                self.db.messages(c.id)?
            };
            self.scroll = 0;
            self.dirty = true;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn move_selection(&mut self, delta: isize) {
        if self.conversations.is_empty() {
            return;
        }
        let max = self.conversations.len() as isize - 1;
        self.selected = (self.selected as isize + delta).clamp(0, max) as usize;
        self.dirty = true;
    }

    #[allow(dead_code)]
    pub fn open_selected(&mut self) -> Result<()> {
        self.select(self.selected)
    }

    // Input handling
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.update_autocomplete();
        self.dirty = true;
    }

    pub fn insert_newline(&mut self) {
        self.input.insert(self.cursor, '\n');
        self.cursor += 1;
        self.update_autocomplete();
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.input.drain(prev..self.cursor);
            self.cursor = prev;
            self.update_autocomplete();
            self.dirty = true;
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor < self.input.len() {
            let next_len = self.input[self.cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(0);
            self.input.drain(self.cursor..self.cursor + next_len);
            self.update_autocomplete();
            self.dirty = true;
        }
    }

    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.input[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.dirty = true;
        }
    }

    pub fn cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += self.input[self.cursor..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(0);
            self.dirty = true;
        }
    }

    pub fn cursor_home(&mut self) {
        self.cursor = 0;
        self.dirty = true;
    }

    pub fn cursor_end(&mut self) {
        self.cursor = self.input.len();
        self.dirty = true;
    }

    pub fn history_prev(&mut self) {
        if self.input_history.is_empty() {
            return;
        }
        let next_idx = match self.input_history_idx {
            None => self.input_history.len().saturating_sub(1),
            Some(i) if i > 0 => i - 1,
            Some(i) => i,
        };
        self.input_history_idx = Some(next_idx);
        if let Some(hist) = self.input_history.get(next_idx) {
            self.input = hist.clone();
            self.cursor = self.input.len();
            self.update_autocomplete();
            self.dirty = true;
        }
    }

    pub fn history_next(&mut self) {
        if let Some(i) = self.input_history_idx {
            if i + 1 < self.input_history.len() {
                let next_idx = i + 1;
                self.input_history_idx = Some(next_idx);
                self.input = self.input_history[next_idx].clone();
                self.cursor = self.input.len();
            } else {
                self.input_history_idx = None;
                self.input.clear();
                self.cursor = 0;
            }
            self.update_autocomplete();
            self.dirty = true;
        }
    }

    pub fn update_autocomplete(&mut self) {
        let trimmed = self.input.trim_start();
        if trimmed.starts_with('/') && !trimmed.contains(' ') {
            self.autocomplete_items = commands::autocomplete_suggestions(trimmed);
            self.autocomplete_active = !self.autocomplete_items.is_empty();
            if self.autocomplete_idx >= self.autocomplete_items.len() {
                self.autocomplete_idx = 0;
            }
        } else {
            self.autocomplete_active = false;
        }
    }

    pub fn apply_autocomplete(&mut self) {
        if self.autocomplete_active && !self.autocomplete_items.is_empty() {
            let spec = self.autocomplete_items[self.autocomplete_idx];
            self.input = format!("{} ", spec.name);
            self.cursor = self.input.len();
            self.autocomplete_active = false;
            self.dirty = true;
        }
    }

    pub fn send(&mut self) -> Result<()> {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.connection == Connection::Generating {
            return Ok(());
        }

        // Save to input history
        if self.input_history.last() != Some(&text) {
            self.input_history.push(text.clone());
        }
        self.input_history_idx = None;

        if text.starts_with('/') {
            self.input.clear();
            self.cursor = 0;
            self.autocomplete_active = false;
            return self.run_command(&text);
        }

        if self.model_options.is_empty() {
            self.set_notice("No models detected, make sure Ollama is running.");
            return Ok(());
        }

        if self.current.is_none() {
            self.new_conversation()?;
        }

        let id = self.current.unwrap();
        let now = Utc::now().timestamp();
        let user = Message {
            id: None,
            conversation_id: id,
            role: Role::User,
            content: text.clone(),
            created_at: now,
        };

        let temporary = self.temporary_conversations.contains(&id);
        if !temporary {
            self.db.add_message(&user)?;
        }
        self.messages.push(user);

        if self.messages.len() == 1 {
            let title = title_from(&text);
            if !temporary {
                let _ = self.db.rename(id, &title);
            }
            if let Some(c) = self.conversations.iter_mut().find(|c| c.id == id) {
                c.title = title;
                c.updated_at = now;
            }
        }

        self.input.clear();
        self.cursor = 0;
        self.partial.clear();
        self.autocomplete_active = false;

        let placeholder = Message {
            id: None,
            conversation_id: id,
            role: Role::Assistant,
            content: String::new(),
            created_at: now,
        };
        self.messages.push(placeholder);

        if temporary {
            self.temporary_messages.insert(id, self.messages.clone());
        }

        let context: Vec<Message> = std::iter::once(Message {
            id: None,
            conversation_id: id,
            role: Role::System,
            content: self.config.assistant.system_prompt.clone(),
            created_at: now,
        })
        .chain(self.messages[..self.messages.len() - 1].iter().cloned())
        .collect();

        let (tx, rx) = mpsc::unbounded_channel();
        self.events = rx;
        let provider = self.provider.clone();
        let model = self.config.model.clone();

        let handle = tokio::spawn(async move {
            if let Err(err) = provider.chat(&model, context, tx.clone()).await {
                let _ = tx.send(StreamEvent::Error(err.to_string()));
            }
        });

        self.abort_handle = Some(handle);
        self.connection = Connection::Generating;
        self.generation_start = Some(Instant::now());
        self.generated_tokens = 0;
        self.scroll = 0;
        self.dirty = true;
        Ok(())
    }

    pub fn retry(&mut self) -> Result<()> {
        if self.connection == Connection::Generating {
            self.stop_generation()?;
        }

        // Find last user message
        if let Some(pos) = self.messages.iter().rposition(|m| m.role == Role::User) {
            let last_user_content = self.messages[pos].content.clone();
            // Truncate messages up to that user message
            self.messages.truncate(pos);
            self.input = last_user_content;
            self.cursor = self.input.len();
            self.send()?;
        } else {
            self.set_notice("No previous user message to retry.");
        }
        Ok(())
    }

    pub fn stop_generation(&mut self) -> Result<()> {
        if let Some(handle) = self.abort_handle.take() {
            handle.abort();
        }
        self.finish_generation()?;
        self.set_notice("Generation stopped.");
        Ok(())
    }

    pub fn poll_stream(&mut self) -> Result<()> {
        while let Ok(event) = self.events.try_recv() {
            match event {
                StreamEvent::Chunk(chunk) => {
                    self.generated_tokens += estimate_tokens(&chunk);
                    self.partial.push_str(&chunk);
                    if let Some(m) = self.messages.last_mut() {
                        m.content.push_str(&chunk);
                    }
                    if let Some(id) = self
                        .current
                        .filter(|id| self.temporary_conversations.contains(id))
                    {
                        self.temporary_messages.insert(id, self.messages.clone());
                    }
                    self.dirty = true;
                }
                StreamEvent::Done => {
                    self.finish_generation()?;
                }
                StreamEvent::Error(msg) => {
                    self.set_notice(format!("Error: {msg}"));
                    self.finish_generation()?;
                }
            }
        }
        Ok(())
    }

    pub fn advance_animation(&mut self) {
        if self.connection == Connection::Generating {
            self.animation_frame = self.animation_frame.wrapping_add(1);
            self.dirty = true;
        }
    }

    pub fn set_theme(&mut self, theme: &str) -> Result<()> {
        let theme_obj = Theme::from_name(theme);
        self.config.ui.theme = theme_obj.id.to_string();
        self.theme_selected = THEMES
            .iter()
            .position(|t| t.id == theme_obj.id)
            .unwrap_or(0);
        let _ = config::save(&self.config);
        self.set_notice(format!("Theme applied: {}", theme_obj.name));
        self.dirty = true;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn cycle_theme(&mut self) -> Result<()> {
        let next_idx = (self.theme_selected + 1) % THEMES.len();
        self.theme_selected = next_idx;
        let theme_obj = &THEMES[next_idx];
        self.config.ui.theme = theme_obj.id.to_string();
        let _ = config::save(&self.config);
        self.set_notice(format!(
            "Theme: {} ({}/{})",
            theme_obj.name,
            next_idx + 1,
            THEMES.len()
        ));
        self.dirty = true;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn move_model_selection(&mut self, delta: isize) {
        if self.model_options.is_empty() {
            return;
        }
        let max = self.model_options.len() as isize - 1;
        self.model_selected = (self.model_selected as isize + delta).clamp(0, max) as usize;
        self.dirty = true;
    }

    pub fn select_model(&mut self) -> Result<()> {
        if let Some(model) = self.model_options.get(self.model_selected).cloned() {
            self.config.model = model;
            let _ = config::save(&self.config);
            self.set_notice(format!("Model switched to: {}", self.config.model));
        }
        self.modal = Modal::None;
        self.dirty = true;
        Ok(())
    }

    pub async fn refresh_models(&mut self) {
        if self.provider.reachable().await {
            if let Ok(models) = self.provider.models().await {
                if !models.is_empty() {
                    if !models.contains(&self.config.model) {
                        self.config.model = models[0].clone();
                    }
                    self.model_options = models;
                    self.model_selected = self
                        .model_options
                        .iter()
                        .position(|m| m == &self.config.model)
                        .unwrap_or(0);
                    self.connection = Connection::Connected;
                } else {
                    self.model_options.clear();
                    self.connection = Connection::Connected;
                }
            } else {
                self.model_options.clear();
                self.connection = Connection::Disconnected;
            }
        } else {
            self.model_options.clear();
            self.connection = Connection::Disconnected;
        }
        self.dirty = true;
    }

    pub fn refresh_conversations(&mut self) -> Result<()> {
        let temporary: Vec<_> = self
            .conversations
            .iter()
            .filter(|c| self.temporary_conversations.contains(&c.id))
            .cloned()
            .collect();
        let mut db_convs = self.db.conversations()?;
        let mut all = temporary;
        all.append(&mut db_convs);
        self.conversations = all;
        if let Some(id) = self.current {
            if let Some(pos) = self.conversations.iter().position(|c| c.id == id) {
                self.selected = pos;
            }
        }
        Ok(())
    }

    pub fn finish_generation(&mut self) -> Result<()> {
        if self.connection != Connection::Generating {
            return Ok(());
        }

        if let Some(message) = self
            .messages
            .last()
            .filter(|m| m.role == Role::Assistant)
            .cloned()
        {
            if !message.content.is_empty()
                && !self
                    .temporary_conversations
                    .contains(&message.conversation_id)
            {
                let _ = self.db.add_message(&message);
            } else if message.content.is_empty() {
                self.messages.pop();
                if let Some(id) = self
                    .current
                    .filter(|id| self.temporary_conversations.contains(id))
                {
                    self.temporary_messages.insert(id, self.messages.clone());
                }
            }
        }

        self.connection = Connection::Connected;
        self.refresh_conversations()?;
        self.generation_start = None;
        self.dirty = true;
        Ok(())
    }

    pub fn delete_current(&mut self) -> Result<()> {
        if let Some(id) = self.current {
            if self.temporary_conversations.remove(&id) {
                self.temporary_messages.remove(&id);
                self.conversations.retain(|c| c.id != id);
            } else {
                self.db.delete(id)?;
                self.refresh_conversations()?;
            }
            self.current = None;
            self.messages.clear();
            if !self.conversations.is_empty() {
                self.select(0)?;
            }
            self.set_notice("Conversation deleted.");
            self.dirty = true;
        }
        Ok(())
    }

    pub fn delete_all(&mut self) -> Result<()> {
        self.db.delete_all()?;
        self.conversations.clear();
        self.temporary_conversations.clear();
        self.temporary_messages.clear();
        self.current = None;
        self.messages.clear();
        self.selected = 0;
        self.set_notice("All conversations permanently cleared.");
        self.modal = Modal::None;
        self.dirty = true;
        Ok(())
    }

    pub fn clear_messages(&mut self) -> Result<()> {
        self.messages.clear();
        if let Some(id) = self.current {
            if self.temporary_conversations.contains(&id) {
                self.temporary_messages.insert(id, Vec::new());
            } else {
                let _ = self.db.clear_messages(id);
            }
        }
        self.set_notice("Cleared message view.");
        self.dirty = true;
        Ok(())
    }

    pub fn set_temporary(&mut self, enabled: bool) -> Result<()> {
        self.temporary_mode = enabled;
        self.new_conversation()?;
        self.set_notice(if enabled {
            "Temporary mode ON: messages will not be saved."
        } else {
            "Persistent mode ON: conversations saved locally to SQLite."
        });
        Ok(())
    }

    pub fn toggle_sidebar(&mut self) -> Result<()> {
        self.config.ui.show_sidebar = !self.config.ui.show_sidebar;
        let _ = config::save(&self.config);
        self.set_notice(if self.config.ui.show_sidebar {
            "Sidebar shown"
        } else {
            "Sidebar hidden (Press Ctrl-B to show)"
        });
        self.dirty = true;
        Ok(())
    }

    pub fn toggle_timestamps(&mut self) -> Result<()> {
        self.config.ui.show_timestamps = !self.config.ui.show_timestamps;
        let _ = config::save(&self.config);
        self.set_notice(if self.config.ui.show_timestamps {
            "Timestamps enabled"
        } else {
            "Timestamps hidden"
        });
        self.dirty = true;
        Ok(())
    }

    pub fn copy_last_response(&mut self) -> Result<()> {
        if let Some(m) = self
            .messages
            .iter()
            .rev()
            .find(|m| m.role == Role::Assistant)
        {
            let content = &m.content;
            if content.is_empty() {
                self.set_notice("Last response is empty.");
                return Ok(());
            }
            // OSC 52 copy escape sequence
            use std::io::Write;
            let b64 = base64_encode(content.as_bytes());
            let osc52 = format!("\x1b]52;c;{}\x07", b64);
            let mut out = std::io::stdout();
            let _ = out.write_all(osc52.as_bytes());
            let _ = out.flush();
            self.set_notice("Copied last assistant response to clipboard (OSC 52).");
        } else {
            self.set_notice("No assistant response to copy.");
        }
        Ok(())
    }

    pub fn export_conversation(&mut self, format: Option<String>) -> Result<()> {
        if self.messages.is_empty() {
            self.set_notice("Nothing to export (conversation is empty).");
            return Ok(());
        }

        let conv_title = self
            .current
            .and_then(|id| self.conversations.iter().find(|c| c.id == id))
            .map(|c| c.title.clone())
            .unwrap_or_else(|| "conversation".into());

        let clean_filename = conv_title
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .to_lowercase();

        let fmt = format.as_deref().unwrap_or("md").to_lowercase();
        let target_path = match fmt.as_str() {
            "json" => {
                let filename = format!("morrow_{}_{}.json", clean_filename, Utc::now().timestamp());
                let json_data = serde_json::to_string_pretty(&self.messages)?;
                fs::write(&filename, json_data)?;
                filename
            }
            _ => {
                let filename = format!("morrow_{}_{}.md", clean_filename, Utc::now().timestamp());
                let mut md = format!(
                    "# {}\n\n*Exported from Morrow AI Workspace on {}*\n\n---\n\n",
                    conv_title,
                    Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
                );
                for m in &self.messages {
                    let role_name = match m.role {
                        Role::User => "## User",
                        Role::Assistant => "## Morrow",
                        Role::System => "## System",
                    };
                    md.push_str(&format!("{}\n\n{}\n\n---\n\n", role_name, m.content));
                }
                fs::write(&filename, md)?;
                filename
            }
        };

        self.set_notice(format!("Exported chat to: {}", target_path));
        Ok(())
    }

    pub fn set_url(&mut self, url: Option<String>) -> Result<()> {
        if let Some(new_url) = url {
            self.config.ollama.url = new_url.clone();
            self.provider = Arc::new(crate::providers::ollama::Ollama::new(new_url.clone()));
            let _ = config::save(&self.config);
            self.set_notice(format!("Ollama URL set to: {}", new_url));
        } else {
            self.set_notice(format!("Ollama endpoint: {}", self.config.ollama.url));
        }
        self.dirty = true;
        Ok(())
    }

    pub fn run_command(&mut self, text: &str) -> Result<()> {
        match commands::parse(text) {
            Ok(Command::Help) => {
                self.modal = Modal::Help;
                self.modal_scroll = 0;
            }
            Ok(Command::New) => self.new_conversation()?,
            Ok(Command::History) => {
                self.modal = Modal::History;
                self.search_query.clear();
                self.modal_selected = self.selected;
            }
            Ok(Command::Model(value)) => {
                if let Some(value) = value {
                    if let Some(index) = self.model_options.iter().position(|m| m == &value) {
                        self.model_selected = index;
                        self.select_model()?;
                    } else {
                        self.set_notice(format!(
                            "Model not in list: '{value}'. Type /model to browse installed models."
                        ));
                    }
                } else {
                    self.modal = Modal::Models;
                    self.search_query.clear();
                    self.modal_selected = self.model_selected;
                }
            }
            Ok(Command::Theme(value)) => {
                if let Some(theme) = value {
                    self.set_theme(&theme)?;
                } else {
                    self.modal = Modal::Themes;
                    self.search_query.clear();
                    self.modal_selected = self.theme_selected;
                }
            }
            Ok(Command::Temporary(value)) => {
                let enabled = match value.as_deref().map(str::to_lowercase).as_deref() {
                    Some("on") | Some("true") | Some("1") | Some("yes") => true,
                    Some("off") | Some("false") | Some("0") | Some("no") => false,
                    Some(_) => {
                        self.set_notice("Usage: /temp [on|off]");
                        return Ok(());
                    }
                    None => !self.temporary_mode,
                };
                self.set_temporary(enabled)?;
            }
            Ok(Command::Rename(value)) => {
                if let Some(title) = value {
                    if let Some(id) = self.current {
                        if self.temporary_conversations.contains(&id) {
                            if let Some(c) = self.conversations.iter_mut().find(|c| c.id == id) {
                                c.title = title.clone();
                                c.updated_at = Utc::now().timestamp();
                            }
                        } else {
                            let _ = self.db.rename(id, &title);
                            self.refresh_conversations()?;
                        }
                        self.set_notice(format!("Renamed to: {title}"));
                    }
                } else {
                    self.modal = Modal::Rename;
                    self.rename_target = self.current;
                    self.modal_input = self
                        .current
                        .and_then(|id| self.conversations.iter().find(|c| c.id == id))
                        .map(|c| c.title.clone())
                        .unwrap_or_default();
                    self.modal_cursor = self.modal_input.len();
                }
            }
            Ok(Command::Delete) => self.delete_current()?,
            Ok(Command::DeleteAll) => {
                self.modal = Modal::ConfirmDeleteAll;
            }
            Ok(Command::Clear) => self.clear_messages()?,
            Ok(Command::System(value)) => {
                if let Some(prompt) = value {
                    self.config.assistant.system_prompt = prompt;
                    let _ = config::save(&self.config);
                    self.set_notice("System instructions updated.");
                } else {
                    self.modal = Modal::SystemPrompt;
                    self.modal_input = self.config.assistant.system_prompt.clone();
                    self.modal_cursor = self.modal_input.len();
                }
            }
            Ok(Command::Sidebar) => self.toggle_sidebar()?,
            Ok(Command::Timestamps) => self.toggle_timestamps()?,
            Ok(Command::Copy) => self.copy_last_response()?,
            Ok(Command::Export(fmt)) => self.export_conversation(fmt)?,
            Ok(Command::Retry) => self.retry()?,
            Ok(Command::Stop) => self.stop_generation()?,
            Ok(Command::Stats) => {
                self.modal = Modal::Stats;
            }
            Ok(Command::Url(val)) => self.set_url(val)?,
            Ok(Command::Bye) => {
                self.set_notice("See you soon. Your conversations are safely local.");
                self.should_quit = true;
            }
            Ok(Command::Quit) => self.should_quit = true,
            Err(err) => self.set_notice(err),
        }
        self.dirty = true;
        Ok(())
    }

    pub fn confirm_modal(&mut self) -> Result<()> {
        match self.modal {
            Modal::History => {
                let filtered = self.filtered_conversations();
                if let Some(c) = filtered.get(self.modal_selected) {
                    if let Some(orig_idx) = self.conversations.iter().position(|x| x.id == c.id) {
                        self.select(orig_idx)?;
                    }
                }
                self.modal = Modal::None;
            }
            Modal::Models => {
                let filtered = self.filtered_models();
                if let Some(m) = filtered.get(self.modal_selected) {
                    self.config.model = m.to_string();
                    let _ = config::save(&self.config);
                    self.set_notice(format!("Model switched to: {}", self.config.model));
                }
                self.modal = Modal::None;
            }
            Modal::Themes => {
                let filtered = search_themes(&self.search_query);
                if let Some(t) = filtered.get(self.modal_selected) {
                    self.set_theme(t.id)?;
                }
                self.modal = Modal::None;
            }
            Modal::Rename => {
                let value = self.modal_input.trim().to_string();
                if !value.is_empty() {
                    let target = self.rename_target.or(self.current);
                    if let Some(id) = target {
                        if self.temporary_conversations.contains(&id) {
                            if let Some(c) = self.conversations.iter_mut().find(|c| c.id == id) {
                                c.title = value.clone();
                                c.updated_at = Utc::now().timestamp();
                            }
                        } else {
                            let _ = self.db.rename(id, &value);
                            self.refresh_conversations()?;
                        }
                        self.set_notice(format!("Renamed to: {value}"));
                    }
                }
                self.rename_target = None;
                self.modal = Modal::None;
            }
            Modal::SystemPrompt => {
                let value = self.modal_input.trim().to_string();
                if !value.is_empty() {
                    self.config.assistant.system_prompt = value;
                    let _ = config::save(&self.config);
                    self.set_notice("System instructions saved.");
                }
                self.modal = Modal::None;
            }
            Modal::ConfirmDeleteAll => {
                self.delete_all()?;
            }
            _ => {
                self.modal = Modal::None;
            }
        }
        self.dirty = true;
        Ok(())
    }

    pub fn filtered_conversations(&self) -> Vec<&Conversation> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            self.conversations.iter().collect()
        } else {
            self.conversations
                .iter()
                .filter(|c| c.title.to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn filtered_models(&self) -> Vec<&String> {
        let q = self.search_query.trim().to_lowercase();
        if q.is_empty() {
            self.model_options.iter().collect()
        } else {
            self.model_options
                .iter()
                .filter(|m| m.to_lowercase().contains(&q))
                .collect()
        }
    }
}

// Simple base64 encode helper for OSC 52
fn base64_encode(input: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    let chunks = input.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARSET[(n >> 18) & 63] as char);
        out.push(CHARSET[(n >> 12) & 63] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(n >> 6) & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[n & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::path::Path;

    struct DummyProvider;

    #[async_trait]
    impl LlmProvider for DummyProvider {
        async fn chat(
            &self,
            _model: &str,
            _messages: Vec<Message>,
            _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
        ) -> Result<()> {
            Ok(())
        }
        async fn reachable(&self) -> bool {
            true
        }
        async fn models(&self) -> Result<Vec<String>> {
            Ok(vec!["llama3.2".into()])
        }
        fn url(&self) -> String {
            "http://localhost:11434".into()
        }
    }

    #[tokio::test]
    async fn test_temporary_mode_toggle_and_isolation() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let config = Config::default();
        let provider = Arc::new(DummyProvider);
        let mut app = App::new(config, db, provider).await.unwrap();

        assert!(!app.temporary_mode);
        assert_eq!(app.conversations.len(), 0);

        // Run /temp to toggle ON
        app.run_command("/temp").unwrap();
        assert!(app.temporary_mode);
        assert_eq!(app.conversations.len(), 1);
        let temp_id = app.current.unwrap();
        assert!(app.temporary_conversations.contains(&temp_id));
        // Ephemeral conversation must NOT be in SQLite
        assert_eq!(app.db.count_conversations().unwrap(), 0);

        // Run /temp again to toggle OFF
        app.run_command("/temp").unwrap();
        assert!(!app.temporary_mode);
        assert_eq!(app.conversations.len(), 2);
        let pers_id = app.current.unwrap();
        assert!(!app.temporary_conversations.contains(&pers_id));
        // Persistent conversation IS in SQLite
        assert_eq!(app.db.count_conversations().unwrap(), 1);

        // Selecting temporary conversation updates temporary_mode to true
        let temp_idx = app.conversations.iter().position(|c| c.id == temp_id).unwrap();
        app.select(temp_idx).unwrap();
        assert!(app.temporary_mode);
        assert_eq!(app.current, Some(temp_id));

        // Selecting persistent conversation updates temporary_mode to false
        let pers_idx = app.conversations.iter().position(|c| c.id == pers_id).unwrap();
        app.select(pers_idx).unwrap();
        assert!(!app.temporary_mode);
        assert_eq!(app.current, Some(pers_id));

        // Explicit /temp on and /temp off
        app.run_command("/temp on").unwrap();
        assert!(app.temporary_mode);
        app.run_command("/temp off").unwrap();
        assert!(!app.temporary_mode);
    }

    #[tokio::test]
    async fn test_rename_and_clear_in_temporary_mode() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let config = Config::default();
        let provider = Arc::new(DummyProvider);
        let mut app = App::new(config, db, provider).await.unwrap();

        app.run_command("/temp on").unwrap();
        let temp_id = app.current.unwrap();

        // Rename temporary conversation
        app.run_command("/rename Secret Session").unwrap();
        assert_eq!(
            app.conversations.iter().find(|c| c.id == temp_id).unwrap().title,
            "Secret Session"
        );
        // Ensure it did not persist to db
        assert_eq!(app.db.count_conversations().unwrap(), 0);

        // Test clear
        app.messages.push(Message::new(temp_id, Role::User, "Hello".into()));
        app.temporary_messages.insert(temp_id, app.messages.clone());
        assert_eq!(app.messages.len(), 1);
        app.clear_messages().unwrap();
        assert_eq!(app.messages.len(), 0);
        assert_eq!(app.temporary_messages.get(&temp_id).unwrap().len(), 0);
    }

    struct OfflineProvider;

    #[async_trait]
    impl LlmProvider for OfflineProvider {
        async fn chat(
            &self,
            _model: &str,
            _messages: Vec<Message>,
            _events: tokio::sync::mpsc::UnboundedSender<StreamEvent>,
        ) -> Result<()> {
            Err(anyhow::anyhow!("Ollama unreachable"))
        }
        async fn reachable(&self) -> bool {
            false
        }
        async fn models(&self) -> Result<Vec<String>> {
            Err(anyhow::anyhow!("Ollama unreachable"))
        }
        fn url(&self) -> String {
            "http://localhost:11434".into()
        }
    }

    #[tokio::test]
    async fn test_no_model_detected_failsafe() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let config = Config::default();
        let provider = Arc::new(OfflineProvider);
        let mut app = App::new(config, db, provider).await.unwrap();

        assert_eq!(app.model_options.len(), 0);
        assert_eq!(app.connection, Connection::Disconnected);
        assert!(app.notice.is_some());

        // User attempts to start chat by typing a message and sending
        app.input = "Explain quantum physics".into();
        app.send().unwrap();

        // Message should NOT be added, input should remain, notice should alert user
        assert_eq!(app.messages.len(), 0);
        assert_eq!(app.input, "Explain quantum physics");
        let (notice_msg, _) = app.notice.unwrap();
        assert!(notice_msg.contains("No models detected, make sure Ollama is running"));
    }
}
