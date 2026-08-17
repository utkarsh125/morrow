mod app;
mod commands;
mod config;
mod db;
mod models;
mod providers;
mod theme;
mod tui;

use anyhow::Result;
use app::{App, Connection, Modal};
use clap::Parser;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use db::Database;
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{io, time::Duration};

#[derive(Parser)]
#[command(
    name = "morrow",
    version,
    about = "Your private AI workspace, always on your machine."
)]
struct Cli;

struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self> {
        enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = Cli::parse();
    let config = config::load_or_create()?;
    let (_, db_path) = config::paths()?;
    let db = Database::open(&db_path)?;
    let provider = providers::from_config(&config);
    let mut app = App::new(config, db, provider).await?;

    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut last_model_check = std::time::Instant::now();
    loop {
        terminal.draw(|frame| tui::draw(frame, &mut app))?;
        app.dirty = false;

        if event::poll(Duration::from_millis(25))? {
            if let Event::Key(key) = event::read()? {
                handle_key(&mut app, key)?;
            }
        }

        if (app.connection == Connection::Disconnected || app.model_options.is_empty())
            && last_model_check.elapsed() > Duration::from_secs(3)
        {
            app.refresh_models().await;
            last_model_check = std::time::Instant::now();
        }

        app.poll_stream()?;
        app.advance_animation();

        if app.should_quit {
            break;
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

fn handle_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // macOS terminal emulators that forward Command report it as SUPER. Treat it
    // as the platform-equivalent shortcut modifier alongside Control.
    let shortcut_modifier = key
        .modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::SUPER);

    // Global interrupt: Ctrl-C / Cmd-C
    if shortcut_modifier && matches!(key.code, KeyCode::Char('c')) {
        if app.connection == Connection::Generating {
            app.stop_generation()?;
            return Ok(());
        }
        if app.modal != Modal::None {
            app.modal = Modal::None;
            app.dirty = true;
            return Ok(());
        }
        app.should_quit = true;
        return Ok(());
    }

    match app.modal {
        Modal::Help => match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.modal = Modal::None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.modal_scroll = app.modal_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.modal_scroll = app.modal_scroll.saturating_add(1);
            }
            KeyCode::PageUp => {
                app.modal_scroll = app.modal_scroll.saturating_sub(6);
            }
            KeyCode::PageDown => {
                app.modal_scroll = app.modal_scroll.saturating_add(6);
            }
            _ => {}
        },

        Modal::History => match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Up => {
                let filtered = app.filtered_conversations();
                if !filtered.is_empty() {
                    app.modal_selected = app.modal_selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                let filtered = app.filtered_conversations();
                if !filtered.is_empty() {
                    app.modal_selected =
                        (app.modal_selected + 1).min(filtered.len().saturating_sub(1));
                }
            }
            KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Char('d') if app.search_query.is_empty() => {
                let selected_id = app
                    .filtered_conversations()
                    .get(app.modal_selected)
                    .map(|c| c.id);
                if let Some(id) = selected_id {
                    if app.temporary_conversations.remove(&id) {
                        app.temporary_messages.remove(&id);
                        app.conversations.retain(|x| x.id != id);
                    } else {
                        let _ = app.db.delete(id);
                        app.refresh_conversations()?;
                    }
                    if app.current == Some(id) {
                        app.current = None;
                        app.messages.clear();
                        if !app.conversations.is_empty() {
                            app.select(0)?;
                        }
                    }
                    if app.modal_selected >= app.conversations.len() {
                        app.modal_selected = app.conversations.len().saturating_sub(1);
                    }
                    app.set_notice("Session deleted.");
                }
            }
            KeyCode::Char('r') if app.search_query.is_empty() => {
                let selected = app
                    .filtered_conversations()
                    .get(app.modal_selected)
                    .map(|c| (c.id, c.title.clone()));
                if let Some((id, title)) = selected {
                    app.modal = Modal::Rename;
                    app.rename_target = Some(id);
                    app.modal_input = title.clone();
                    app.modal_cursor = title.len();
                }
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.modal_selected = 0;
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.modal_selected = 0;
            }
            _ => {}
        },

        Modal::Models => match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Up => {
                let filtered = app.filtered_models();
                if !filtered.is_empty() {
                    app.modal_selected = app.modal_selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                let filtered = app.filtered_models();
                if !filtered.is_empty() {
                    app.modal_selected =
                        (app.modal_selected + 1).min(filtered.len().saturating_sub(1));
                }
            }
            KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.modal_selected = 0;
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.modal_selected = 0;
            }
            _ => {}
        },

        Modal::Themes => match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Up => {
                let filtered = theme::search_themes(&app.search_query);
                if !filtered.is_empty() {
                    app.modal_selected = app.modal_selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                let filtered = theme::search_themes(&app.search_query);
                if !filtered.is_empty() {
                    app.modal_selected =
                        (app.modal_selected + 1).min(filtered.len().saturating_sub(1));
                }
            }
            KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Backspace => {
                app.search_query.pop();
                app.modal_selected = 0;
            }
            KeyCode::Char(c) => {
                app.search_query.push(c);
                app.modal_selected = 0;
            }
            _ => {}
        },

        Modal::Rename => match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Backspace => {
                app.modal_input.pop();
            }
            KeyCode::Char(c) => {
                app.modal_input.push(c);
            }
            _ => {}
        },

        Modal::SystemPrompt => match key.code {
            KeyCode::Esc => {
                app.modal = Modal::None;
            }
            KeyCode::Enter if shortcut_modifier => {
                app.confirm_modal()?;
            }
            KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Backspace => {
                app.modal_input.pop();
            }
            KeyCode::Char(c) => {
                app.modal_input.push(c);
            }
            _ => {}
        },

        Modal::Stats => match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                app.modal = Modal::None;
            }
            _ => {}
        },

        Modal::ConfirmDeleteAll => match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                app.confirm_modal()?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.modal = Modal::None;
            }
            _ => {}
        },

        Modal::None => {
            // Check Autocomplete Popup interactions
            if app.autocomplete_active && !app.autocomplete_items.is_empty() {
                match key.code {
                    KeyCode::Tab => {
                        app.apply_autocomplete();
                        return Ok(());
                    }
                    KeyCode::Up => {
                        app.autocomplete_idx = app.autocomplete_idx.saturating_sub(1);
                        app.dirty = true;
                        return Ok(());
                    }
                    KeyCode::Down => {
                        if app.autocomplete_idx + 1 < app.autocomplete_items.len() {
                            app.autocomplete_idx += 1;
                        }
                        app.dirty = true;
                        return Ok(());
                    }
                    KeyCode::Esc => {
                        app.autocomplete_active = false;
                        app.dirty = true;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // Control modifier shortcuts
            if shortcut_modifier {
                match key.code {
                    KeyCode::Char('n') => {
                        app.new_conversation()?;
                        return Ok(());
                    }
                    KeyCode::Char('h') => {
                        app.modal = Modal::History;
                        app.search_query.clear();
                        app.modal_selected = app.selected;
                        return Ok(());
                    }
                    KeyCode::Char('t') => {
                        app.modal = Modal::Themes;
                        app.search_query.clear();
                        app.modal_selected = app.theme_selected;
                        return Ok(());
                    }
                    KeyCode::Char('p') | KeyCode::Char('m') => {
                        app.modal = Modal::Models;
                        app.search_query.clear();
                        app.modal_selected = app.model_selected;
                        return Ok(());
                    }
                    KeyCode::Char('b') => {
                        app.toggle_sidebar()?;
                        return Ok(());
                    }
                    KeyCode::Char('y') => {
                        app.copy_last_response()?;
                        return Ok(());
                    }
                    // Send message
                    KeyCode::Enter | KeyCode::Char('s') => {
                        app.send()?;
                        return Ok(());
                    }
                    KeyCode::Char('u') => {
                        app.scroll = app.scroll.saturating_sub(8);
                        return Ok(());
                    }
                    KeyCode::Char('d') => {
                        app.scroll = app.scroll.saturating_add(8);
                        return Ok(());
                    }
                    KeyCode::Char('l') => {
                        app.clear_messages()?;
                        return Ok(());
                    }
                    _ => {}
                }
            }

            // Regular keys
            match key.code {
                KeyCode::Backspace => app.backspace(),
                KeyCode::Delete => app.delete_char(),
                KeyCode::Enter => {
                    if app.input.starts_with('/') {
                        app.send()?;
                    } else {
                        app.insert_newline();
                    }
                }
                KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(10),
                KeyCode::PageDown => app.scroll = app.scroll.saturating_add(10),
                KeyCode::Up => {
                    if app.input.is_empty() || !app.input.contains('\n') {
                        app.history_prev();
                    } else {
                        app.scroll = app.scroll.saturating_sub(2);
                    }
                }
                KeyCode::Down => {
                    if app.input.is_empty() || !app.input.contains('\n') {
                        app.history_next();
                    } else {
                        app.scroll = app.scroll.saturating_add(2);
                    }
                }
                KeyCode::Left => app.cursor_left(),
                KeyCode::Right => app.cursor_right(),
                KeyCode::Home => app.cursor_home(),
                KeyCode::End => app.cursor_end(),
                KeyCode::Tab => {
                    if app.input.starts_with('/') {
                        app.apply_autocomplete();
                    } else {
                        app.insert_char(' ');
                        app.insert_char(' ');
                    }
                }
                KeyCode::Char(c) => app.insert_char(c),
                _ => {}
            }
        }
    }

    app.dirty = true;
    Ok(())
}
