use crate::models::{Conversation, Message, Role};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;
use uuid::Uuid;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY);
             CREATE TABLE IF NOT EXISTS conversations (
                 id TEXT PRIMARY KEY,
                 title TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 conversation_id TEXT NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);",
        )?;
        Ok(Self { conn })
    }

    pub fn conversations(&self) -> Result<Vec<Conversation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, created_at, updated_at FROM conversations ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Conversation {
                id: Uuid::parse_str(&r.get::<_, String>(0)?).unwrap_or_else(|_| Uuid::nil()),
                title: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn create_conversation(&self, c: &Conversation) -> Result<()> {
        self.conn.execute(
            "INSERT INTO conversations (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
            params![c.id.to_string(), c.title, c.created_at, c.updated_at],
        )?;
        Ok(())
    }

    pub fn messages(&self, id: Uuid) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([id.to_string()], |r| {
            Ok(Message {
                id: Some(r.get(0)?),
                conversation_id: id,
                role: Role::from_str(&r.get::<_, String>(1)?).unwrap_or(Role::Assistant),
                content: r.get(2)?,
                created_at: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn add_message(&self, m: &Message) -> Result<()> {
        self.conn.execute(
            "INSERT INTO messages (conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                m.conversation_id.to_string(),
                m.role.as_str(),
                m.content,
                m.created_at
            ],
        )?;
        self.conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![m.created_at, m.conversation_id.to_string()],
        )?;
        Ok(())
    }

    pub fn rename(&self, id: Uuid, title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE conversations SET title = ?1 WHERE id = ?2",
            params![title, id.to_string()],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        self.conn
            .execute("DELETE FROM conversations WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }

    pub fn delete_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM messages", [])?;
        self.conn.execute("DELETE FROM conversations", [])?;
        Ok(())
    }

    pub fn count_conversations(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM conversations")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;
        Ok(count as usize)
    }

    pub fn count_messages(&self) -> Result<usize> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM messages")?;
        let count: i64 = stmt.query_row([], |r| r.get(0))?;
        Ok(count as usize)
    }

    pub fn clear_messages(&self, id: Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM messages WHERE conversation_id = ?1",
            [id.to_string()],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;

    #[test]
    fn persists_conversation_and_messages() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let c = Conversation::new("Test Conversation".into());
        db.create_conversation(&c).unwrap();

        let msg = Message::new(c.id, Role::User, "hello world".into());
        db.add_message(&msg).unwrap();

        let list = db.messages(c.id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].content, "hello world");

        db.rename(c.id, "Renamed Title").unwrap();
        let convs = db.conversations().unwrap();
        assert_eq!(convs[0].title, "Renamed Title");

        assert_eq!(db.count_conversations().unwrap(), 1);
        assert_eq!(db.count_messages().unwrap(), 1);

        db.delete(c.id).unwrap();
        assert_eq!(db.count_conversations().unwrap(), 0);
        assert_eq!(db.count_messages().unwrap(), 0);
    }
}
