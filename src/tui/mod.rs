use crate::{
    app::{App, Connection, Modal},
    models::Role,
    theme::{Theme, search_themes},
};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let theme = app.current_theme();
    let area = frame.area();

    // Fill background
    frame.render_widget(Block::default().style(Style::default().bg(theme.bg)), area);

    // Main layout
    let chunks = Layout::vertical([
        Constraint::Length(3), // Header
        Constraint::Min(6),    // Main chat + sidebar
        Constraint::Length(1), // Telemetry & Status bar
        Constraint::Length(3), // Input area
    ])
    .split(area);

    header(frame, chunks[0], app, theme);

    // Body with optional sidebar
    if app.config.ui.show_sidebar {
        let body_chunks = Layout::horizontal([
            Constraint::Length(app.config.ui.sidebar_width.min(area.width / 3)),
            Constraint::Min(30),
        ])
        .split(chunks[1]);

        sidebar(frame, body_chunks[0], app, theme);
        chat(frame, body_chunks[1], app, theme);
    } else {
        chat(frame, chunks[1], app, theme);
    }

    status_bar(frame, chunks[2], app, theme);
    input_box(frame, chunks[3], app, theme);

    // Floating autocomplete suggestions if active
    if app.autocomplete_active && !app.autocomplete_items.is_empty() {
        render_autocomplete_popup(frame, chunks[3], app, theme);
    }

    // Modal Overlays
    match app.modal {
        Modal::None => {}
        Modal::Help => render_modal_help(frame, app, theme),
        Modal::History => render_modal_history(frame, app, theme),
        Modal::Models => render_modal_models(frame, app, theme),
        Modal::Themes => render_modal_themes(frame, app, theme),
        Modal::Rename => render_modal_rename(frame, app, theme),
        Modal::SystemPrompt => render_modal_system_prompt(frame, app, theme),
        Modal::Stats => render_modal_stats(frame, app, theme),
        Modal::ConfirmDeleteAll => render_modal_confirm_delete(frame, theme),
    }
}

fn rounded_block<'a>(title: &'a str, theme: &'static Theme, border_color: Color) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme.panel))
        .border_style(Style::default().fg(border_color))
        .title(Line::from(vec![
            Span::styled(" ", Style::default()),
            Span::styled(title, Style::default().fg(theme.text).bold()),
            Span::styled(" ", Style::default()),
        ]))
}

fn header(f: &mut Frame, a: Rect, app: &App, theme: &'static Theme) {
    let state_badge = match app.connection {
        Connection::Connected => {
            if app.model_options.is_empty() {
                ("● NO MODELS", theme.warning)
            } else {
                ("● CONNECTED", theme.success)
            }
        }
        Connection::Generating => {
            let frames = ["◐ STREAMING", "◓ STREAMING", "◑ STREAMING", "◒ STREAMING"];
            let idx = (app.animation_frame as usize / 2) % frames.len();
            (frames[idx], theme.accent)
        }
        Connection::Disconnected => ("○ OFFLINE", theme.error),
    };

    let mode_badge = if app.temporary_mode {
        (" [EPHEMERAL] ", theme.warning)
    } else {
        (" [LOCAL DB] ", theme.muted)
    };

    let cols = Layout::horizontal([
        Constraint::Length(28),
        Constraint::Min(20),
        Constraint::Length(32),
    ])
    .split(a);

    // Left brand
    let brand = Line::from(vec![
        Span::styled(
            " MORROW ",
            Style::default().bg(theme.accent).fg(theme.bg).bold(),
        ),
        Span::styled(" v0.1 ", Style::default().fg(theme.muted)),
        Span::styled(mode_badge.0, Style::default().fg(mode_badge.1).bold()),
    ]);
    f.render_widget(
        Paragraph::new(brand).block(rounded_block("", theme, theme.border)),
        cols[0],
    );

    // Center active conversation title & model
    let conv_title = app
        .current
        .and_then(|id| app.conversations.iter().find(|c| c.id == id))
        .map(|c| c.title.as_str())
        .unwrap_or("New Session");

    let model_label = if app.model_options.is_empty() {
        Span::styled("No Models Detected", Style::default().fg(theme.error).bold())
    } else {
        Span::styled(
            format!("◆ {}", app.config.model),
            Style::default().fg(theme.assistant),
        )
    };

    let center_text = Line::from(vec![
        Span::styled("◈ ", Style::default().fg(theme.accent)),
        Span::styled(conv_title, Style::default().fg(theme.text).bold()),
        Span::styled("  ·  ", Style::default().fg(theme.muted)),
        model_label,
    ]);
    f.render_widget(
        Paragraph::new(center_text)
            .alignment(Alignment::Center)
            .block(rounded_block("", theme, theme.border)),
        cols[1],
    );

    // Right status
    let status_text = Line::from(vec![
        Span::styled("Ollama  ", Style::default().fg(theme.muted)),
        Span::styled(state_badge.0, Style::default().fg(state_badge.1).bold()),
    ]);
    f.render_widget(
        Paragraph::new(status_text)
            .alignment(Alignment::Right)
            .block(rounded_block("", theme, theme.border)),
        cols[2],
    );
}

fn sidebar(f: &mut Frame, a: Rect, app: &App, theme: &'static Theme) {
    let title = format!(" SESSIONS ({}) ", app.conversations.len());
    let items: Vec<ListItem> = if app.conversations.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            " No saved sessions",
            Style::default().fg(theme.muted).italic(),
        )))]
    } else {
        app.conversations
            .iter()
            .enumerate()
            .map(|(_idx, c)| {
                let is_current = app.current == Some(c.id);
                let pointer = if is_current { "▶ " } else { "  " };
                let is_temp = app.temporary_conversations.contains(&c.id);

                let title_style = if is_current {
                    Style::default().fg(theme.accent).bold()
                } else {
                    Style::default().fg(theme.text)
                };

                let rel_time = c.relative_time();
                let display_title = if c.title.len() > 16 {
                    format!("{}…", &c.title[..15])
                } else {
                    c.title.clone()
                };

                let mut spans = vec![
                    Span::styled(pointer, Style::default().fg(theme.accent).bold()),
                    Span::styled(display_title, title_style),
                ];

                if is_temp {
                    spans.push(Span::styled(" [temp]", Style::default().fg(theme.warning)));
                }

                spans.push(Span::styled(
                    format!("  {}", rel_time),
                    Style::default().fg(theme.muted),
                ));

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !app.conversations.is_empty() {
        state.select(Some(app.selected));
    }

    let list = List::new(items)
        .block(rounded_block(&title, theme, theme.border))
        .highlight_style(Style::default().bg(theme.surface).fg(theme.accent).bold())
        .highlight_symbol("");

    f.render_stateful_widget(list, a, &mut state);
}

fn chat(f: &mut Frame, a: Rect, app: &App, theme: &'static Theme) {
    let mut lines = Vec::new();

    if app.messages.is_empty() {
        // Hermes-style Hero / Welcome Banner
        lines.extend([
            Line::from(""),
            Line::from(Span::styled(
                "       ███╗   ███╗ ██████╗ ██████╗ ██████╗  ██████╗ ██╗    ██╗",
                Style::default().fg(theme.accent).bold(),
            )),
            Line::from(Span::styled(
                "       ████╗ ████║██╔═══██╗██╔══██╗██╔══██╗██╔═══██╗██║    ██║",
                Style::default().fg(theme.accent).bold(),
            )),
            Line::from(Span::styled(
                "       ██╔████╔██║██║   ██║██████╔╝██████╔╝██║   ██║██║ █╗ ██║",
                Style::default().fg(theme.accent).bold(),
            )),
            Line::from(Span::styled(
                "       ██║╚██╔╝██║██║   ██║██╔══██╗██╔══██╗██║   ██║██║███╗██║",
                Style::default().fg(theme.assistant).bold(),
            )),
            Line::from(Span::styled(
                "       ██║ ╚═╝ ██║╚██████╔╝██║  ██║██║  ██║╚██████╔╝╚███╔███╔╝",
                Style::default().fg(theme.assistant).bold(),
            )),
            Line::from(Span::styled(
                "       ╚═╝     ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝  ╚══╝╚══╝ ",
                Style::default().fg(theme.assistant).bold(),
            )),
            Line::from(""),
            Line::from(vec![
                Span::styled("   ◆ ", Style::default().fg(theme.accent)),
                Span::styled(
                    "Morrow AI Workspace",
                    Style::default().fg(theme.text).bold(),
                ),
                Span::styled(
                    " · 100% Private, Local & Uncompromised",
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("   Backend:   ", Style::default().fg(theme.muted)),
                Span::styled(&app.config.ollama.url, Style::default().fg(theme.accent)),
                Span::styled("   Model: ", Style::default().fg(theme.muted)),
                if app.model_options.is_empty() {
                    Span::styled(
                        "No models detected",
                        Style::default().fg(theme.error).bold(),
                    )
                } else {
                    Span::styled(
                        &app.config.model,
                        Style::default().fg(theme.assistant).bold(),
                    )
                },
                Span::styled("   Theme: ", Style::default().fg(theme.muted)),
                Span::styled(theme.name, Style::default().fg(theme.code_fg)),
            ]),
            if app.model_options.is_empty() {
                Line::from(vec![
                    Span::styled("   Status:    ", Style::default().fg(theme.muted)),
                    Span::styled(
                        "No models detected. Make sure Ollama is running (`ollama serve`) and a model is pulled (`ollama pull qwen2.5:7b`).",
                        Style::default().fg(theme.error).bold(),
                    ),
                ])
            } else {
                Line::from("")
            },
            Line::from(""),
            Line::from(Span::styled(
                "   Quick Commands:",
                Style::default().fg(theme.text).bold(),
            )),
            Line::from(vec![
                Span::styled(
                    "     /help       ",
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(
                    "Open interactive command palette & shortcut guide",
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "     /model      ",
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(
                    "Switch local Ollama models (or /model <name>)",
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "     /theme      ",
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(
                    "Browse 65+ Kitty terminal themes (or Ctrl-T)",
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "     /temp       ",
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(
                    "Start an unsaved ephemeral chat session",
                    Style::default().fg(theme.muted),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "   Type your question below and press Ctrl-S (or Ctrl-Enter) to send.",
                Style::default().fg(theme.muted).italic(),
            )),
            Line::from(""),
        ]);
    } else {
        for (idx, msg) in app.messages.iter().enumerate() {
            let is_last = idx == app.messages.len() - 1;
            let time_str = if app.config.ui.show_timestamps {
                format!(" · {}", msg.formatted_time())
            } else {
                String::new()
            };

            match msg.role {
                Role::User => {
                    let header_line = Line::from(vec![
                        Span::styled("┌─ ", Style::default().fg(theme.user)),
                        Span::styled("YOU", Style::default().fg(theme.user).bold()),
                        Span::styled(time_str, Style::default().fg(theme.muted)),
                        Span::styled(" ─", Style::default().fg(theme.user)),
                    ]);
                    lines.push(header_line);
                    lines.extend(format_markdown(&msg.content, theme));
                    lines.push(Line::from(""));
                }
                Role::Assistant => {
                    let model_label = format!(" · {}", app.config.model);
                    let header_line = Line::from(vec![
                        Span::styled("┌─ ", Style::default().fg(theme.assistant)),
                        Span::styled("MORROW", Style::default().fg(theme.assistant).bold()),
                        Span::styled(model_label, Style::default().fg(theme.muted)),
                        Span::styled(time_str, Style::default().fg(theme.muted)),
                        Span::styled(" ─", Style::default().fg(theme.assistant)),
                    ]);
                    lines.push(header_line);

                    if msg.content.is_empty() && is_last && app.connection == Connection::Generating
                    {
                        lines.push(render_streaming_dots(app.animation_frame, theme));
                    } else {
                        lines.extend(format_markdown(&msg.content, theme));
                        if is_last && app.connection == Connection::Generating {
                            lines.push(render_streaming_dots(app.animation_frame, theme));
                        }
                    }
                    lines.push(Line::from(""));
                }
                Role::System => {
                    let header_line = Line::from(vec![Span::styled(
                        "┌─ SYSTEM ─",
                        Style::default().fg(theme.muted).bold(),
                    )]);
                    lines.push(header_line);
                    lines.push(Line::from(Span::styled(
                        &msg.content,
                        Style::default().fg(theme.muted).italic(),
                    )));
                    lines.push(Line::from(""));
                }
            }
        }
    }

    let p = Paragraph::new(lines)
        .block(rounded_block(" CHAT ", theme, theme.border))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));

    f.render_widget(p, a);
}

fn render_streaming_dots(frame: u8, theme: &'static Theme) -> Line<'static> {
    let waves = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spinner = waves[(frame as usize) % waves.len()];
    Line::from(vec![
        Span::styled(
            format!("  {} Thinking & generating...", spinner),
            Style::default().fg(theme.accent).bold(),
        ),
        Span::styled(" ▌", Style::default().fg(theme.user).bold()),
    ])
}

fn format_markdown(content: &str, theme: &'static Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Check for thinking blocks <think> ... </think>
    if let Some(think_start) = content.find("<think>") {
        let before_think = &content[..think_start];
        if !before_think.trim().is_empty() {
            lines.extend(parse_markdown_chunk(before_think, theme));
        }

        if let Some(think_end) = content.find("</think>") {
            let think_content = &content[think_start + 7..think_end];
            lines.push(Line::from(vec![Span::styled(
                "  ┌─ Reasoning / Thought Process ──",
                Style::default().fg(theme.muted).bold(),
            )]));
            for tline in think_content.lines() {
                lines.push(Line::from(vec![
                    Span::styled("  │ ", Style::default().fg(theme.muted)),
                    Span::styled(tline.to_string(), Style::default().fg(theme.muted).italic()),
                ]));
            }
            lines.push(Line::from(vec![Span::styled(
                "  └──────────────────────────────────",
                Style::default().fg(theme.muted),
            )]));

            let after_think = &content[think_end + 8..];
            if !after_think.trim().is_empty() {
                lines.extend(parse_markdown_chunk(after_think, theme));
            }
        } else {
            // Streaming think block
            let think_content = &content[think_start + 7..];
            lines.push(Line::from(vec![Span::styled(
                "  ┌─ Reasoning (In Progress)... ──",
                Style::default().fg(theme.accent).bold(),
            )]));
            for tline in think_content.lines() {
                lines.push(Line::from(vec![
                    Span::styled("  │ ", Style::default().fg(theme.accent)),
                    Span::styled(tline.to_string(), Style::default().fg(theme.muted).italic()),
                ]));
            }
        }
        return lines;
    }

    parse_markdown_chunk(content, theme)
}

fn parse_markdown_chunk(markdown: &str, theme: &'static Theme) -> Vec<Line<'static>> {
    let mut output = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut in_code_block = false;
    let mut current_style = Style::default().fg(theme.text);

    let flush_line = |output: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>| {
        if !spans.is_empty() {
            output.push(Line::from(spans.clone()));
            spans.clear();
        }
    };

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush_line(&mut output, &mut current_spans);
                current_style = Style::default().fg(theme.accent).bold();
                current_spans.push(Span::styled(
                    "  ◈ ",
                    Style::default().fg(theme.accent).bold(),
                ));
            }
            Event::End(TagEnd::Heading(_)) => {
                flush_line(&mut output, &mut current_spans);
                current_style = Style::default().fg(theme.text);
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                flush_line(&mut output, &mut current_spans);
                in_code_block = true;
                let code_lang = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => lang.to_string(),
                    _ => "code".into(),
                };
                output.push(Line::from(vec![
                    Span::styled("  ┌─ [", Style::default().fg(theme.muted)),
                    Span::styled(
                        if code_lang.is_empty() {
                            "code".into()
                        } else {
                            code_lang.clone()
                        },
                        Style::default().fg(theme.code_fg).bold(),
                    ),
                    Span::styled(
                        "] ──────────────────────────────────────────",
                        Style::default().fg(theme.muted),
                    ),
                ]));
            }
            Event::End(TagEnd::CodeBlock) => {
                flush_line(&mut output, &mut current_spans);
                in_code_block = false;
                output.push(Line::from(vec![Span::styled(
                    "  └───────────────────────────────────────────────────",
                    Style::default().fg(theme.muted),
                )]));
            }
            Event::Start(Tag::Item) => {
                flush_line(&mut output, &mut current_spans);
                current_spans.push(Span::styled(
                    "    • ",
                    Style::default().fg(theme.accent).bold(),
                ));
            }
            Event::End(TagEnd::Item) => {
                flush_line(&mut output, &mut current_spans);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                flush_line(&mut output, &mut current_spans);
                current_spans.push(Span::styled(
                    "  │ ",
                    Style::default().fg(theme.quote_fg).bold(),
                ));
                current_style = Style::default().fg(theme.muted).italic();
            }
            Event::End(TagEnd::BlockQuote(_)) => {
                flush_line(&mut output, &mut current_spans);
                current_style = Style::default().fg(theme.text);
            }
            Event::Start(Tag::Emphasis) => {
                current_style = current_style.add_modifier(Modifier::ITALIC);
            }
            Event::End(TagEnd::Emphasis) => {
                current_style = current_style.remove_modifier(Modifier::ITALIC);
            }
            Event::Start(Tag::Strong) => {
                current_style = current_style.add_modifier(Modifier::BOLD);
            }
            Event::End(TagEnd::Strong) => {
                current_style = current_style.remove_modifier(Modifier::BOLD);
            }
            Event::Text(value) => {
                if in_code_block {
                    for line in value.lines() {
                        output.push(Line::from(vec![
                            Span::styled("  │ ", Style::default().fg(theme.muted)),
                            Span::styled(
                                line.to_string(),
                                Style::default().fg(theme.code_fg).bg(theme.code_bg),
                            ),
                        ]));
                    }
                } else {
                    if current_spans.is_empty() {
                        current_spans.push(Span::styled("  ", Style::default()));
                    }
                    current_spans.push(Span::styled(value.to_string(), current_style));
                }
            }
            Event::Code(value) => {
                current_spans.push(Span::styled(
                    format!("`{value}`"),
                    Style::default().fg(theme.code_fg).bg(theme.surface),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                flush_line(&mut output, &mut current_spans);
            }
            Event::End(TagEnd::Paragraph) => {
                flush_line(&mut output, &mut current_spans);
            }
            _ => {}
        }
    }

    flush_line(&mut output, &mut current_spans);
    output
}

fn status_bar(f: &mut Frame, a: Rect, app: &App, theme: &'static Theme) {
    let tokens = app.total_tokens_in_current_chat();
    let msg_count = app.messages.len();

    let notice_text = app.active_notice();

    let left_spans = if let Some(notice) = notice_text {
        vec![
            Span::styled(" ℹ ", Style::default().fg(theme.accent).bold()),
            Span::styled(notice, Style::default().fg(theme.text).bold()),
        ]
    } else {
        vec![
            Span::styled(
                format!(" Model: {} ", app.config.model),
                Style::default().fg(theme.assistant).bold(),
            ),
            Span::styled("·", Style::default().fg(theme.muted)),
            Span::styled(
                format!(" Msgs: {} ", msg_count),
                Style::default().fg(theme.text),
            ),
            Span::styled("·", Style::default().fg(theme.muted)),
            Span::styled(
                format!(" Tokens: ~{} ", tokens),
                Style::default().fg(theme.muted),
            ),
            Span::styled("·", Style::default().fg(theme.muted)),
            Span::styled(
                format!(" Theme: {} ", theme.name),
                Style::default().fg(theme.accent),
            ),
        ]
    };

    let right_spans = vec![
        Span::styled("Ctrl-S: Send  ", Style::default().fg(theme.muted)),
        Span::styled("Tab: Auto  ", Style::default().fg(theme.muted)),
        Span::styled("Ctrl-T: Themes  ", Style::default().fg(theme.muted)),
        Span::styled("Ctrl-N: New  ", Style::default().fg(theme.muted)),
        Span::styled("Ctrl-H: History  ", Style::default().fg(theme.muted)),
        Span::styled("/help: All", Style::default().fg(theme.accent).bold()),
    ];

    let cols = Layout::horizontal([Constraint::Min(40), Constraint::Length(55)]).split(a);

    f.render_widget(Paragraph::new(Line::from(left_spans)), cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(right_spans)).alignment(Alignment::Right),
        cols[1],
    );
}

fn input_box(f: &mut Frame, a: Rect, app: &App, theme: &'static Theme) {
    let is_focused = app.modal == Modal::None;
    let border_color = if is_focused {
        theme.accent
    } else {
        theme.border
    };

    let display_text = if app.input.is_empty() {
        "Ask Morrow anything... (Ctrl-S to send, / for slash commands)"
    } else {
        &app.input
    };

    let style = if app.input.is_empty() {
        Style::default().fg(theme.muted).italic()
    } else {
        Style::default().fg(theme.text)
    };

    let input_title = if app.temporary_mode {
        " ❯ PROMPT [EPHEMERAL] "
    } else {
        " ❯ PROMPT "
    };

    let p = Paragraph::new(display_text)
        .style(style)
        .block(rounded_block(input_title, theme, border_color));

    f.render_widget(p, a);
}

fn render_autocomplete_popup(f: &mut Frame, input_rect: Rect, app: &App, theme: &'static Theme) {
    let count = app.autocomplete_items.len().min(8) as u16;
    let popup_height = count + 2;
    let popup_y = input_rect.y.saturating_sub(popup_height);
    let popup_width = 58.min(input_rect.width);
    let popup_rect = Rect::new(input_rect.x + 2, popup_y, popup_width, popup_height);

    f.render_widget(Clear, popup_rect);

    let items: Vec<ListItem> = app
        .autocomplete_items
        .iter()
        .take(8)
        .enumerate()
        .map(|(idx, spec)| {
            let is_sel = idx == app.autocomplete_idx;
            let pointer = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default().fg(theme.accent).bold().bg(theme.surface)
            } else {
                Style::default().fg(theme.text)
            };

            let line = Line::from(vec![
                Span::styled(pointer, Style::default().fg(theme.accent).bold()),
                Span::styled(
                    format!("{:<10} ", spec.name),
                    Style::default().fg(theme.accent).bold(),
                ),
                Span::styled(
                    format!("{:<8} ", spec.args),
                    Style::default().fg(theme.warning),
                ),
                Span::styled(spec.description, Style::default().fg(theme.muted)),
            ]);

            ListItem::new(line).style(style)
        })
        .collect();

    let list = List::new(items).block(rounded_block(
        " COMMANDS (Tab: Apply · ↑↓: Select) ",
        theme,
        theme.accent,
    ));

    f.render_widget(list, popup_rect);
}

fn centered_rect(a: Rect, w: u16, h: u16) -> Rect {
    let vert = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(h.min(a.height)),
        Constraint::Fill(1),
    ])
    .split(a);

    Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(w.min(a.width)),
        Constraint::Fill(1),
    ])
    .split(vert[1])[1]
}

fn render_modal_help(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 76, 24);
    f.render_widget(Clear, a);

    let mut lines = vec![
        Line::from(vec![Span::styled(
            "MORROW COMMAND PALETTE & KEYBOARD GUIDE",
            Style::default().fg(theme.accent).bold(),
        )]),
        Line::from(""),
    ];

    let commands_table = [
        ("/help", "", "Show this guide & all shortcuts"),
        ("/new", "", "Start a clean conversation session"),
        (
            "/history",
            "",
            "Interactive session manager (search, preview, delete)",
        ),
        (
            "/model",
            "[name]",
            "Switch or browse local Ollama LLM models",
        ),
        (
            "/theme",
            "[name]",
            "Browse 65+ Kitty terminal themes with live preview",
        ),
        (
            "/temp",
            "[on|off]",
            "Toggle ephemeral incognito chat (no SQLite saving)",
        ),
        ("/rename", "[title]", "Rename current conversation"),
        ("/delete", "", "Delete active conversation"),
        (
            "/delete all",
            "",
            "Permanently purge all local conversation history",
        ),
        ("/clear", "", "Clear messages in this current view"),
        (
            "/system",
            "[prompt]",
            "View or edit the AI system prompt instructions",
        ),
        ("/sidebar", "", "Toggle sidebar visibility (or Ctrl-B)"),
        ("/timestamps", "", "Toggle message timestamp headers"),
        (
            "/copy",
            "",
            "Copy last assistant response to clipboard (OSC 52)",
        ),
        (
            "/export",
            "[md|json]",
            "Export conversation to Markdown or JSON file",
        ),
        ("/retry", "", "Regenerate the last response"),
        ("/stop", "", "Abort active generation stream"),
        ("/stats", "", "View telemetry, tokens, DB and Ollama info"),
        ("/url", "[url]", "View or configure Ollama server URL"),
        ("/bye, /quit", "", "Exit Morrow"),
    ];

    for (cmd, arg, desc) in commands_table {
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<14}", cmd),
                Style::default().fg(theme.accent).bold(),
            ),
            Span::styled(format!("{:<10}", arg), Style::default().fg(theme.warning)),
            Span::styled(desc, Style::default().fg(theme.text)),
        ]));
    }

    lines.extend([
        Line::from(""),
        Line::from(vec![Span::styled(
            "KEYBOARD SHORTCUTS:",
            Style::default().fg(theme.assistant).bold(),
        )]),
        Line::from(vec![
            Span::styled("  Ctrl-S / Ctrl-Enter: ", Style::default().fg(theme.muted)),
            Span::styled("Send message    ", Style::default().fg(theme.text)),
            Span::styled("Ctrl-N: ", Style::default().fg(theme.muted)),
            Span::styled("New chat    ", Style::default().fg(theme.text)),
            Span::styled("Ctrl-H: ", Style::default().fg(theme.muted)),
            Span::styled("History", Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("  Ctrl-T: ", Style::default().fg(theme.muted)),
            Span::styled("Themes (65+)    ", Style::default().fg(theme.text)),
            Span::styled("Ctrl-B: ", Style::default().fg(theme.muted)),
            Span::styled("Sidebar         ", Style::default().fg(theme.text)),
            Span::styled("Ctrl-C: ", Style::default().fg(theme.muted)),
            Span::styled("Quit / Abort", Style::default().fg(theme.text)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or Enter to close this guide.",
            Style::default().fg(theme.muted).italic(),
        )),
    ]);

    let p = Paragraph::new(lines)
        .block(rounded_block(
            " COMMANDS & HELP · Esc to Close ",
            theme,
            theme.accent,
        ))
        .scroll((app.modal_scroll, 0));

    f.render_widget(p, a);
}

fn render_modal_history(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 82, 22);
    f.render_widget(Clear, a);

    let chunks = Layout::vertical([
        Constraint::Length(3), // Search input
        Constraint::Min(12),   // List + Preview
        Constraint::Length(1), // Help footer
    ])
    .split(a);

    // Search bar
    let search_bar = Paragraph::new(if app.search_query.is_empty() {
        "Type to filter sessions..."
    } else {
        &app.search_query
    })
    .style(if app.search_query.is_empty() {
        Style::default().fg(theme.muted).italic()
    } else {
        Style::default().fg(theme.text)
    })
    .block(rounded_block(" SEARCH SESSIONS ", theme, theme.accent));

    f.render_widget(search_bar, chunks[0]);

    // Split list + preview
    let content_cols = Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    let filtered = app.filtered_conversations();
    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No sessions found",
            Style::default().fg(theme.muted),
        )))]
    } else {
        filtered
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                let is_sel = idx == app.modal_selected;
                let pointer = if is_sel { "▶ " } else { "  " };
                let is_temp = app.temporary_conversations.contains(&c.id);

                let mut spans = vec![
                    Span::styled(pointer, Style::default().fg(theme.accent).bold()),
                    Span::styled(
                        &c.title,
                        if is_sel {
                            Style::default().fg(theme.accent).bold()
                        } else {
                            Style::default().fg(theme.text)
                        },
                    ),
                ];
                if is_temp {
                    spans.push(Span::styled(" [temp]", Style::default().fg(theme.warning)));
                }
                spans.push(Span::styled(
                    format!(" ({})", c.relative_time()),
                    Style::default().fg(theme.muted),
                ));
                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(app.modal_selected));
    }

    let list = List::new(items)
        .block(rounded_block(" SESSIONS ", theme, theme.border))
        .highlight_style(Style::default().bg(theme.surface).fg(theme.accent).bold());

    f.render_stateful_widget(list, content_cols[0], &mut state);

    // Message preview
    let preview_lines = if let Some(c) = filtered.get(app.modal_selected) {
        let msgs = if app.temporary_conversations.contains(&c.id) {
            app.temporary_messages
                .get(&c.id)
                .cloned()
                .unwrap_or_default()
        } else {
            app.db.messages(c.id).unwrap_or_default()
        };

        if msgs.is_empty() {
            vec![Line::from(Span::styled(
                "No messages in this session.",
                Style::default().fg(theme.muted).italic(),
            ))]
        } else {
            let mut out = Vec::new();
            for m in msgs.iter().take(6) {
                let (role, color) = match m.role {
                    Role::User => ("You: ", theme.user),
                    Role::Assistant => ("Morrow: ", theme.assistant),
                    Role::System => ("System: ", theme.muted),
                };
                out.push(Line::from(vec![
                    Span::styled(role, Style::default().fg(color).bold()),
                    Span::styled(m.content.clone(), Style::default().fg(theme.text)),
                ]));
                out.push(Line::from(""));
            }
            out
        }
    } else {
        vec![Line::from(Span::styled(
            "Select a session to preview",
            Style::default().fg(theme.muted),
        ))]
    };

    let preview = Paragraph::new(preview_lines)
        .block(rounded_block(" PREVIEW ", theme, theme.border))
        .wrap(Wrap { trim: false });

    f.render_widget(preview, content_cols[1]);

    let footer = Paragraph::new("Enter: Open  ·  d: Delete  ·  r: Rename  ·  Esc: Close")
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center);

    f.render_widget(footer, chunks[2]);
}

fn render_modal_models(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 64, 18);
    f.render_widget(Clear, a);

    let chunks = Layout::vertical([
        Constraint::Length(3), // Search bar
        Constraint::Min(8),    // Models list
        Constraint::Length(1), // Footer
    ])
    .split(a);

    let search_bar = Paragraph::new(if app.search_query.is_empty() {
        "Type to filter models..."
    } else {
        &app.search_query
    })
    .style(if app.search_query.is_empty() {
        Style::default().fg(theme.muted).italic()
    } else {
        Style::default().fg(theme.text)
    })
    .block(rounded_block(
        " SEARCH LOCAL OLLAMA MODELS ",
        theme,
        theme.accent,
    ));

    f.render_widget(search_bar, chunks[0]);

    let filtered = app.filtered_models();
    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No models detected. Make sure Ollama is running ('ollama serve') and pull a model ('ollama pull <name>').",
            Style::default().fg(theme.error),
        )))]
    } else {
        filtered
            .iter()
            .enumerate()
            .map(|(idx, m)| {
                let is_current = *m == &app.config.model;
                let is_sel = idx == app.modal_selected;
                let pointer = if is_sel { "▶ " } else { "  " };
                let current_tag = if is_current { " (active)" } else { "" };

                let line = Line::from(vec![
                    Span::styled(pointer, Style::default().fg(theme.accent).bold()),
                    Span::styled(
                        *m,
                        if is_sel {
                            Style::default().fg(theme.accent).bold()
                        } else {
                            Style::default().fg(theme.text)
                        },
                    ),
                    Span::styled(current_tag, Style::default().fg(theme.assistant).bold()),
                ]);

                ListItem::new(line)
            })
            .collect()
    };

    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(app.modal_selected));
    }

    let list = List::new(items)
        .block(rounded_block(" AVAILABLE MODELS ", theme, theme.border))
        .highlight_style(Style::default().bg(theme.surface).fg(theme.accent).bold());

    f.render_stateful_widget(list, chunks[1], &mut state);

    let footer = Paragraph::new("Enter: Select Model  ·  ↑↓: Navigate  ·  Esc: Close")
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center);

    f.render_widget(footer, chunks[2]);
}

fn render_modal_themes(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 84, 22);
    f.render_widget(Clear, a);

    let chunks = Layout::vertical([
        Constraint::Length(3), // Search bar
        Constraint::Min(12),   // Split list + preview
        Constraint::Length(1), // Footer
    ])
    .split(a);

    let search_bar = Paragraph::new(if app.search_query.is_empty() {
        "Type to filter 65+ Kitty terminal themes (e.g. catppuccin, tokyo, dracula, rose-pine)..."
    } else {
        &app.search_query
    })
    .style(if app.search_query.is_empty() {
        Style::default().fg(theme.muted).italic()
    } else {
        Style::default().fg(theme.text)
    })
    .block(rounded_block(
        " SEARCH THEMES (65+ Themes) ",
        theme,
        theme.accent,
    ));

    f.render_widget(search_bar, chunks[0]);

    let content_cols = Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let filtered = search_themes(&app.search_query);
    let items: Vec<ListItem> = if filtered.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "No themes found",
            Style::default().fg(theme.muted),
        )))]
    } else {
        filtered
            .iter()
            .enumerate()
            .map(|(idx, t)| {
                let is_active = t.id == app.config.ui.theme;
                let is_sel = idx == app.modal_selected;
                let pointer = if is_sel { "▶ " } else { "  " };
                let cat_tag = format!(" [{}]", t.category);

                let mut spans = vec![
                    Span::styled(pointer, Style::default().fg(theme.accent).bold()),
                    Span::styled(
                        t.name,
                        if is_sel {
                            Style::default().fg(theme.accent).bold()
                        } else {
                            Style::default().fg(theme.text)
                        },
                    ),
                    Span::styled(cat_tag, Style::default().fg(theme.muted)),
                ];
                if is_active {
                    spans.push(Span::styled(
                        " (active)",
                        Style::default().fg(theme.assistant).bold(),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !filtered.is_empty() {
        state.select(Some(app.modal_selected));
    }

    let list = List::new(items)
        .block(rounded_block(" THEMES ", theme, theme.border))
        .highlight_style(Style::default().bg(theme.surface).fg(theme.accent).bold());

    f.render_stateful_widget(list, content_cols[0], &mut state);

    // Live color palette preview of highlighted theme
    let preview_lines = if let Some(sel_theme) = filtered.get(app.modal_selected) {
        vec![
            Line::from(vec![
                Span::styled("Theme: ", Style::default().fg(sel_theme.muted)),
                Span::styled(sel_theme.name, Style::default().fg(sel_theme.accent).bold()),
                Span::styled(
                    format!(" ({})", if sel_theme.is_dark { "Dark" } else { "Light" }),
                    Style::default().fg(sel_theme.muted),
                ),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Palette Samples:",
                Style::default().fg(sel_theme.text).bold(),
            )]),
            Line::from(vec![
                Span::styled(" ■ Accent: ", Style::default().fg(sel_theme.accent).bold()),
                Span::styled("████", Style::default().fg(sel_theme.accent)),
                Span::styled(
                    "  ■ Assistant: ",
                    Style::default().fg(sel_theme.assistant).bold(),
                ),
                Span::styled("████", Style::default().fg(sel_theme.assistant)),
            ]),
            Line::from(vec![
                Span::styled(" ■ User:   ", Style::default().fg(sel_theme.user).bold()),
                Span::styled("████", Style::default().fg(sel_theme.user)),
                Span::styled(
                    "  ■ Code:      ",
                    Style::default().fg(sel_theme.code_fg).bold(),
                ),
                Span::styled("████", Style::default().fg(sel_theme.code_fg)),
            ]),
            Line::from(vec![
                Span::styled(" ■ Success:", Style::default().fg(sel_theme.success).bold()),
                Span::styled("████", Style::default().fg(sel_theme.success)),
                Span::styled(
                    "  ■ Warning:   ",
                    Style::default().fg(sel_theme.warning).bold(),
                ),
                Span::styled("████", Style::default().fg(sel_theme.warning)),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "Sample Chat Rendering:",
                Style::default().fg(sel_theme.text).bold(),
            )]),
            Line::from(vec![
                Span::styled("  You: ", Style::default().fg(sel_theme.user).bold()),
                Span::styled("How fast is Morrow?", Style::default().fg(sel_theme.text)),
            ]),
            Line::from(vec![
                Span::styled(
                    "  Morrow: ",
                    Style::default().fg(sel_theme.assistant).bold(),
                ),
                Span::styled(
                    "Instant, calm, and 100% private.",
                    Style::default().fg(sel_theme.text),
                ),
            ]),
        ]
    } else {
        vec![Line::from("Select a theme to see preview")]
    };

    let preview = Paragraph::new(preview_lines).block(rounded_block(
        " LIVE PALETTE PREVIEW ",
        theme,
        theme.border,
    ));

    f.render_widget(preview, content_cols[1]);

    let footer = Paragraph::new("Enter: Apply Theme  ·  ↑↓: Navigate / Preview  ·  Esc: Close")
        .style(Style::default().fg(theme.muted))
        .alignment(Alignment::Center);

    f.render_widget(footer, chunks[2]);
}

fn render_modal_rename(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 56, 7);
    f.render_widget(Clear, a);

    let p = Paragraph::new(app.modal_input.as_str())
        .style(Style::default().fg(theme.text).bold())
        .block(rounded_block(
            " RENAME SESSION · Enter to Save · Esc to Cancel ",
            theme,
            theme.accent,
        ));

    f.render_widget(p, a);
}

fn render_modal_system_prompt(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 72, 14);
    f.render_widget(Clear, a);

    let p = Paragraph::new(app.modal_input.as_str())
        .style(Style::default().fg(theme.text))
        .wrap(Wrap { trim: false })
        .block(rounded_block(
            " SYSTEM INSTRUCTIONS (PROMPT) · Enter to Save · Esc to Cancel ",
            theme,
            theme.accent,
        ));

    f.render_widget(p, a);
}

fn render_modal_stats(f: &mut Frame, app: &App, theme: &'static Theme) {
    let a = centered_rect(f.area(), 68, 16);
    f.render_widget(Clear, a);

    let total_convs = app.db.count_conversations().unwrap_or(0);
    let total_msgs = app.db.count_messages().unwrap_or(0);
    let cur_tokens = app.total_tokens_in_current_chat();

    let lines = vec![
        Line::from(vec![Span::styled(
            "MORROW SESSION TELEMETRY",
            Style::default().fg(theme.accent).bold(),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Active Model:         ", Style::default().fg(theme.muted)),
            Span::styled(
                &app.config.model,
                Style::default().fg(theme.assistant).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Ollama Endpoint:      ", Style::default().fg(theme.muted)),
            Span::styled(&app.config.ollama.url, Style::default().fg(theme.accent)),
        ]),
        Line::from(vec![
            Span::styled("  Connection Status:    ", Style::default().fg(theme.muted)),
            Span::styled(
                match app.connection {
                    Connection::Connected => "Connected (Ready)",
                    Connection::Generating => "Streaming Generation",
                    Connection::Disconnected => "Offline / Unreachable",
                },
                Style::default()
                    .fg(if app.connection == Connection::Connected {
                        theme.success
                    } else {
                        theme.error
                    })
                    .bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Current Session:      ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{} messages (~{} tokens)", app.messages.len(), cur_tokens),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Total Local Sessions: ", Style::default().fg(theme.muted)),
            Span::styled(
                format!(
                    "{} conversations ({} messages in SQLite)",
                    total_convs, total_msgs
                ),
                Style::default().fg(theme.text),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Active Theme:         ", Style::default().fg(theme.muted)),
            Span::styled(
                format!("{} (65+ available in repo)", theme.name),
                Style::default().fg(theme.code_fg),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc or Enter to close.",
            Style::default().fg(theme.muted).italic(),
        )),
    ];

    let p = Paragraph::new(lines).block(rounded_block(
        " TELEMETRY & SYSTEM STATS ",
        theme,
        theme.accent,
    ));

    f.render_widget(p, a);
}

fn render_modal_confirm_delete(f: &mut Frame, theme: &'static Theme) {
    let a = centered_rect(f.area(), 54, 9);
    f.render_widget(Clear, a);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  DELETE ALL LOCAL CONVERSATIONS?",
            Style::default().fg(theme.error).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  This will permanently delete all SQLite chat records.",
            Style::default().fg(theme.muted),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Press ", Style::default().fg(theme.muted)),
            Span::styled("y", Style::default().fg(theme.error).bold()),
            Span::styled(" or ", Style::default().fg(theme.muted)),
            Span::styled("Enter", Style::default().fg(theme.error).bold()),
            Span::styled(" to confirm, ", Style::default().fg(theme.muted)),
            Span::styled("Esc", Style::default().fg(theme.accent).bold()),
            Span::styled(" to cancel.", Style::default().fg(theme.muted)),
        ]),
    ];

    let p = Paragraph::new(lines).block(rounded_block(" CONFIRM PURGE ", theme, theme.error));

    f.render_widget(p, a);
}
