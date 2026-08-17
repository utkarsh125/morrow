# Morrow

> Your private AI workspace, always on your machine.

Morrow is a calm, keyboard-first, Hermes-inspired terminal workspace for chatting with local language models through [Ollama](https://ollama.com) or a local OpenAI-compatible server. Conversations stay in a local SQLite database; Morrow has no accounts, telemetry, analytics, API keys, or cloud APIs.

---

## Highlights

- **Hermes-Inspired TUI Layout**: Crisp status banners, collapsible session sidebar (`Ctrl-B`), rich markdown code fences, thought process `<think>` blocks, live token telemetry, and streaming animations.
- **65+ Kitty Theme Repository**: Curated library of 65+ official Kitty terminal color themes across Dark, Light, Retro, and Vibrant categories, with a searchable modal browser and live RGB palette preview (`Ctrl-T` or `/theme`).
- **Interactive Slash Command Autocomplete**: Type `/` to bring up a floating autocomplete popup with instant command filtering, syntax hints, and descriptions.
- **100% Local & Ephemeral Modes**: Switch seamlessly between persistent local SQLite storage and ephemeral incognito chat (`/temp` / `/temp off`).
- **Export & Clipboard Tools**: Copy assistant answers directly to clipboard (`/copy` via OSC 52) or export entire conversation threads to Markdown and JSON (`/export md` / `/export json`).
- **Local Provider Choice**: Use Ollama or a local OpenAI-compatible server such as LM Studio, llama.cpp, or LocalAI (`/provider`).
- **Prompt Attachments & Motion Controls**: Add a local UTF-8 text file with `/attach <path>` and control streaming motion with `/animations on|off`.

---

## Installation

### Option 1: Homebrew (macOS & Linux)

```bash
brew install utkarsh125/tap/morrow
```

### Option 2: Quick Install Script (Linux & macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/utkarsh125/morrow/main/install.sh | bash
```

### Option 3: Cargo from Git

```bash
cargo install --git https://github.com/utkarsh125/morrow
```

### Option 4: Cargo from crates.io

```bash
cargo install morrow
```

### Option 5: Build from Source

```bash
git clone https://github.com/utkarsh125/morrow.git
cd morrow
cargo install --path .
```

---

## Quick Start

1. Start your local Ollama server: `ollama serve`
2. Pull your model of choice (e.g. `ollama pull qwen2.5:7b` or `ollama pull deepseek-r1:8b`)
3. Run Morrow:

```bash
morrow
```

Morrow creates its configuration at `~/.config/morrow/config.toml` and its history database at `~/.local/share/morrow/history.db` on first launch.

---

## Keyboard Controls

| Key | Action |
| --- | --- |
| `Ctrl-S` / `Ctrl-Enter` (`Cmd-S` / `Cmd-Enter` on macOS terminals that forward Command) | Send message |
| `Enter` | Insert newline (or execute single-line command) |
| `Tab` | Autocomplete slash command / indent |
| `Ctrl-N` | Start new conversation session |
| `Ctrl-H` | Browse conversation history & preview |
| `Ctrl-T` | Open 65+ Kitty Themes browser with live preview |
| `Ctrl-P` / `Ctrl-M` | Open Model switcher |
| `Ctrl-B` | Toggle conversation sidebar |
| `Ctrl-Y` | Copy last assistant response to clipboard (OSC 52) |
| `PgUp` / `PgDn` | Scroll chat history |
| `Ctrl-U` / `Ctrl-D` | Half-page chat scroll |
| `Up` / `Down` | Browse input history (in prompt) or navigate modal list |
| `Esc` | Close active modal, cancel autocomplete popup, or deselect |
| `Ctrl-C` | Stop streaming generation (or quit) |

---

## Slash Commands

| Command | Arguments | Description |
| --- | --- | --- |
| `/help` | | Open interactive command palette and keyboard guide |
| `/new` | | Start a clean conversation session |
| `/history` | | Interactive session manager (search, preview, delete `d`, rename `r`) |
| `/model` | `[name]` | Switch active model or open model selector |
| `/theme` | `[name]` | Open 65+ Kitty theme picker with live preview or switch theme |
| `/temp` | `[on\|off]` | Toggle ephemeral temporary chat mode (not written to SQLite) |
| `/rename` | `[title]` | Rename the active conversation |
| `/delete` | | Delete current conversation |
| `/delete all` | | Purge all local conversation history |
| `/clear` | | Clear current session message view |
| `/system` | `[prompt]` | View or edit AI system prompt instructions |
| `/sidebar` | | Toggle conversation sidebar visibility |
| `/timestamps` | | Toggle message timestamp headers |
| `/copy` | | Copy last assistant response to clipboard (OSC 52) |
| `/export` | `[md\|json]` | Export conversation to Markdown or JSON |
| `/retry` | | Regenerate the last AI response |
| `/stop` | | Abort active streaming generation |
| `/stats` | | View session telemetry, tokens, DB and Ollama info |
| `/url` | `[url]` | View or change Ollama server endpoint |
| `/provider` | `[ollama\|local]` | Switch between Ollama and a local OpenAI-compatible server |
| `/attach` | `<path>` | Add a local UTF-8 text file (up to 256 KiB) to the next prompt |
| `/animations` | `[on\|off]` | Toggle streaming/loading animations |
| `/bye`, `/quit` | | Exit Morrow |

---

## Themes (65+ Built-in Kitty Themes)

Morrow includes over 65 curated Kitty terminal themes, including:

- **Catppuccin**: Mocha, Macchiato, Frappé, Latte
- **Tokyo Night**: Tokyo Night, Storm, Moon, Day
- **Nord**: Nord, Nord Light, Nordfox
- **Dracula**: Dracula, Dracula High Contrast
- **One Dark**: One Dark Pro, One Light
- **Gruvbox**: Gruvbox Dark, Gruvbox Light, Gruvbox Material
- **Rosé Pine**: Rosé Pine, Moon, Dawn
- **Solarized**: Solarized Dark, Solarized Light
- **Kanagawa**: Wave, Dragon, Lotus
- **Everforest**: Everforest Dark, Everforest Light
- **Monokai**: Monokai Pro, Monokai Classic
- **Ayu**: Ayu Dark, Ayu Mirage, Ayu Light
- **Nightfox Collection**: Nightfox, Duskfox, Carbonfox, Dawnfox, Terafox
- **GitHub**: GitHub Dark, GitHub Light, GitHub Dark Dimmed
- **Specialty & Retro**: SynthWave '84, Cyberpunk 2077, Cobalt2, Poimandres, Matrix Green, Andromeda, Zenburn, Melange, Eldritch, Flexoki, LaserWave, Espresso, Snazzy, Challenger Deep, Afterglow, Sonokai, and more.

Switch live with `Ctrl-T` or `/theme <name>`. Your selection persists across restarts.

---

## Privacy & Security

Morrow only communicates with the configured local provider: Ollama by default (`http://localhost:11434`) or an OpenAI-compatible local server (`http://localhost:1234/v1`). It has zero telemetry, zero cloud tracking, and never sends conversation data outside your machine.

---

## Development

```bash
cargo test
cargo fmt --check
```

---

## License

MIT
