use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    System,
    User,
    Assistant,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Option<i64>,
    pub conversation_id: Uuid,
    pub role: Role,
    pub content: String,
    pub created_at: i64,
}

impl Message {
    #[allow(dead_code)]
    pub fn new(conversation_id: Uuid, role: Role, content: String) -> Self {
        Self {
            id: None,
            conversation_id,
            role,
            content,
            created_at: Utc::now().timestamp(),
        }
    }

    pub fn formatted_time(&self) -> String {
        let dt = DateTime::from_timestamp(self.created_at, 0).unwrap_or_else(|| Utc::now());
        let local_dt: DateTime<Local> = DateTime::from(dt);
        local_dt.format("%H:%M:%S").to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Conversation {
    #[allow(dead_code)]
    pub fn new(title: String) -> Self {
        let now = Utc::now().timestamp();
        Self {
            id: Uuid::new_v4(),
            title,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn relative_time(&self) -> String {
        let now = Utc::now().timestamp();
        let diff = now.saturating_sub(self.updated_at);
        if diff < 60 {
            "just now".into()
        } else if diff < 3600 {
            format!("{}m ago", diff / 60)
        } else if diff < 86400 {
            format!("{}h ago", diff / 3600)
        } else if diff < 604800 {
            format!("{}d ago", diff / 86400)
        } else {
            let dt = DateTime::from_timestamp(self.updated_at, 0).unwrap_or_else(|| Utc::now());
            let local_dt: DateTime<Local> = DateTime::from(dt);
            local_dt.format("%b %d").to_string()
        }
    }
}

pub fn title_from(text: &str) -> String {
    let clean = text.trim().lines().next().unwrap_or("").trim();
    let words: Vec<_> = clean.split_whitespace().take(8).collect();
    if words.is_empty() {
        return "New conversation".into();
    }
    let mut title = words.join(" ");
    if title.len() > 40 {
        title.truncate(37);
        title.push_str("...");
    }
    title
}

pub fn estimate_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // Standard rule-of-thumb: ~4 characters per token in English / code
    let char_count = text.chars().count();
    (char_count + 3) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_serialization() {
        assert_eq!(Role::User.as_str(), "user");
        assert_eq!(Role::Assistant.as_str(), "assistant");
        assert_eq!(Role::System.as_str(), "system");
        assert_eq!(Role::from_str("user"), Some(Role::User));
        assert_eq!(Role::from_str("assistant"), Some(Role::Assistant));
        assert_eq!(Role::from_str("system"), Some(Role::System));
        assert_eq!(Role::from_str("unknown"), None);
    }

    #[test]
    fn test_title_from() {
        assert_eq!(title_from(""), "New conversation");
        assert_eq!(title_from("   "), "New conversation");
        assert_eq!(title_from("Hello world"), "Hello world");
        assert_eq!(
            title_from("Write a python function to compute fibonacci"),
            "Write a python function to compute fi..."
        );
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("word"), 1);
        assert_eq!(estimate_tokens("hello world!"), 3);
    }

    #[test]
    fn test_relative_time() {
        let mut conv = Conversation::new("Test".into());
        assert_eq!(conv.relative_time(), "just now");

        conv.updated_at = Utc::now().timestamp() - 120;
        assert_eq!(conv.relative_time(), "2m ago");

        conv.updated_at = Utc::now().timestamp() - 7200;
        assert_eq!(conv.relative_time(), "2h ago");
    }
}
