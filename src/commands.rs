#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    New,
    History,
    Model(Option<String>),
    Theme(Option<String>),
    Temporary(Option<String>),
    Rename(Option<String>),
    Delete,
    DeleteAll,
    Clear,
    System(Option<String>),
    Sidebar,
    Timestamps,
    Copy,
    Export(Option<String>),
    Retry,
    Stop,
    Stats,
    Url(Option<String>),
    Provider(Option<String>),
    Attach(Option<String>),
    Animations(Option<String>),
    Bye,
    Quit,
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub name: &'static str,
    pub args: &'static str,
    pub description: &'static str,
    #[allow(dead_code)]
    pub category: &'static str,
}

pub static COMMAND_SPECS: &[CommandSpec] = &[
    CommandSpec {
        name: "/provider",
        args: "[ollama|local]",
        description: "Choose Ollama or a local OpenAI-compatible server",
        category: "Model",
    },
    CommandSpec {
        name: "/attach",
        args: "<path>",
        description: "Attach a local text file to the next message",
        category: "Tools",
    },
    CommandSpec {
        name: "/animations",
        args: "[on|off]",
        description: "Toggle streaming and loading animations",
        category: "Appearance",
    },
    CommandSpec {
        name: "/help",
        args: "",
        description: "Open command palette and shortcut guide",
        category: "General",
    },
    CommandSpec {
        name: "/new",
        args: "",
        description: "Start a new conversation",
        category: "Chat",
    },
    CommandSpec {
        name: "/history",
        args: "",
        description: "Browse, preview & switch conversation sessions",
        category: "Chat",
    },
    CommandSpec {
        name: "/model",
        args: "[name]",
        description: "Switch active LLM model or browse installed models",
        category: "Model",
    },
    CommandSpec {
        name: "/theme",
        args: "[name]",
        description: "Browse & switch themes (65+ Kitty terminal themes)",
        category: "Appearance",
    },
    CommandSpec {
        name: "/temp",
        args: "[on|off]",
        description: "Toggle temporary chat mode (not saved to SQLite)",
        category: "Chat",
    },
    CommandSpec {
        name: "/rename",
        args: "[title]",
        description: "Rename current conversation",
        category: "Chat",
    },
    CommandSpec {
        name: "/delete",
        args: "",
        description: "Delete the current conversation",
        category: "Chat",
    },
    CommandSpec {
        name: "/delete all",
        args: "",
        description: "Purge all locally stored conversation history",
        category: "Chat",
    },
    CommandSpec {
        name: "/clear",
        args: "",
        description: "Clear message view in the current session",
        category: "Chat",
    },
    CommandSpec {
        name: "/system",
        args: "[prompt]",
        description: "View or customize the AI system instructions",
        category: "Model",
    },
    CommandSpec {
        name: "/sidebar",
        args: "",
        description: "Toggle conversation sidebar on/off",
        category: "Appearance",
    },
    CommandSpec {
        name: "/timestamps",
        args: "",
        description: "Toggle message timestamp headers",
        category: "Appearance",
    },
    CommandSpec {
        name: "/copy",
        args: "",
        description: "Copy the last assistant response to clipboard",
        category: "Tools",
    },
    CommandSpec {
        name: "/export",
        args: "[md|json]",
        description: "Export current chat to a Markdown or JSON file",
        category: "Tools",
    },
    CommandSpec {
        name: "/retry",
        args: "",
        description: "Regenerate the last AI response",
        category: "Chat",
    },
    CommandSpec {
        name: "/stop",
        args: "",
        description: "Abort the active generation stream",
        category: "Chat",
    },
    CommandSpec {
        name: "/stats",
        args: "",
        description: "View session telemetry, token count & database stats",
        category: "General",
    },
    CommandSpec {
        name: "/url",
        args: "[url]",
        description: "View or change the Ollama backend API URL",
        category: "Model",
    },
    CommandSpec {
        name: "/bye",
        args: "",
        description: "Exit Morrow with session confirmation",
        category: "General",
    },
    CommandSpec {
        name: "/quit",
        args: "",
        description: "Quit Morrow immediately",
        category: "General",
    },
];

pub fn parse(input: &str) -> Result<Command, String> {
    let text = input.trim();
    let mut parts = text.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").to_lowercase();
    let arg = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    match name.as_str() {
        "/help" | "/?" => Ok(Command::Help),
        "/new" | "/n" | "/reset" => Ok(Command::New),
        "/history" | "/h" | "/sessions" | "/convs" => Ok(Command::History),
        "/model" | "/m" | "/models" => Ok(Command::Model(arg)),
        "/theme" | "/t" | "/themes" => Ok(Command::Theme(arg)),
        "/temporary" | "/temp" | "/incognito" | "/private" => Ok(Command::Temporary(arg)),
        "/rename" | "/title" => Ok(Command::Rename(arg)),
        "/delete" if arg.as_deref() == Some("all") => Ok(Command::DeleteAll),
        "/delete" | "/del" | "/rm" => Ok(Command::Delete),
        "/delete-all" | "/clear-all" | "/purge" => Ok(Command::DeleteAll),
        "/clear" | "/cls" => Ok(Command::Clear),
        "/system" | "/sys" | "/prompt" => Ok(Command::System(arg)),
        "/sidebar" | "/toggle-sidebar" | "/sb" => Ok(Command::Sidebar),
        "/timestamps" | "/time" | "/ts" => Ok(Command::Timestamps),
        "/copy" | "/y" | "/yank" | "/cp" => Ok(Command::Copy),
        "/export" | "/save" => Ok(Command::Export(arg)),
        "/retry" | "/regenerate" | "/redo" => Ok(Command::Retry),
        "/stop" | "/abort" | "/cancel" => Ok(Command::Stop),
        "/stats" | "/info" | "/status" => Ok(Command::Stats),
        "/url" | "/endpoint" | "/host" => Ok(Command::Url(arg)),
        "/provider" | "/backend" => Ok(Command::Provider(arg)),
        "/attach" | "/file" => Ok(Command::Attach(arg)),
        "/animations" | "/motion" => Ok(Command::Animations(arg)),
        "/bye" => Ok(Command::Bye),
        "/quit" | "/q" | "/exit" => Ok(Command::Quit),
        _ => Err(format!(
            "Unknown command '{name}'. Type /help for all commands."
        )),
    }
}

pub fn autocomplete_suggestions(input: &str) -> Vec<&'static CommandSpec> {
    let query = input.trim_start();
    if !query.starts_with('/') {
        return Vec::new();
    }
    let query_lower = query.to_lowercase();
    COMMAND_SPECS
        .iter()
        .filter(|spec| spec.name.starts_with(&query_lower) || spec.name.contains(&query_lower))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_commands() {
        assert_eq!(parse("/help"), Ok(Command::Help));
        assert_eq!(parse("/?"), Ok(Command::Help));
        assert_eq!(parse("/new"), Ok(Command::New));
        assert_eq!(parse("/n"), Ok(Command::New));
        assert_eq!(parse("/history"), Ok(Command::History));
        assert_eq!(parse("/h"), Ok(Command::History));
        assert_eq!(
            parse("/model llama3.1:8b"),
            Ok(Command::Model(Some("llama3.1:8b".into())))
        );
        assert_eq!(
            parse("/theme rose-pine"),
            Ok(Command::Theme(Some("rose-pine".into())))
        );
        assert_eq!(parse("/temp"), Ok(Command::Temporary(None)));
        assert_eq!(
            parse("/temp on"),
            Ok(Command::Temporary(Some("on".into())))
        );
        assert_eq!(
            parse("/temp off"),
            Ok(Command::Temporary(Some("off".into())))
        );
        assert_eq!(
            parse("/temporary"),
            Ok(Command::Temporary(None))
        );
        assert_eq!(
            parse("/incognito"),
            Ok(Command::Temporary(None))
        );
        assert_eq!(
            parse("/private"),
            Ok(Command::Temporary(None))
        );
        assert_eq!(
            parse("/rename My Project"),
            Ok(Command::Rename(Some("My Project".into())))
        );
        assert_eq!(parse("/delete"), Ok(Command::Delete));
        assert_eq!(parse("/delete all"), Ok(Command::DeleteAll));
        assert_eq!(parse("/purge"), Ok(Command::DeleteAll));
        assert_eq!(parse("/clear"), Ok(Command::Clear));
        assert_eq!(parse("/sidebar"), Ok(Command::Sidebar));
        assert_eq!(parse("/copy"), Ok(Command::Copy));
        assert_eq!(parse("/export md"), Ok(Command::Export(Some("md".into()))));
        assert_eq!(parse("/retry"), Ok(Command::Retry));
        assert_eq!(parse("/stop"), Ok(Command::Stop));
        assert_eq!(
            parse("/provider local"),
            Ok(Command::Provider(Some("local".into())))
        );
        assert_eq!(
            parse("/attach notes.md"),
            Ok(Command::Attach(Some("notes.md".into())))
        );
        assert_eq!(
            parse("/animations off"),
            Ok(Command::Animations(Some("off".into())))
        );
        assert_eq!(parse("/stats"), Ok(Command::Stats));
        assert_eq!(
            parse("/url http://127.0.0.1:11434"),
            Ok(Command::Url(Some("http://127.0.0.1:11434".into())))
        );
        assert_eq!(parse("/quit"), Ok(Command::Quit));
    }

    #[test]
    fn autocompletes_slash_commands() {
        let results = autocomplete_suggestions("/th");
        assert!(results.iter().any(|s| s.name == "/theme"));

        let results = autocomplete_suggestions("/m");
        assert!(results.iter().any(|s| s.name == "/model"));
    }
}
