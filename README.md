# caw

**coding assistant watcher**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A cross-platform desktop app and TUI that monitors all running AI coding assistant instances from a single place. The crow is the mascot — it watches silently from above.

![Screenshot placeholder](docs/screenshot.png)

## Supported Assistants

| Assistant | Status |
|-----------|--------|
| Claude Code | Supported |
| OpenAI Codex CLI | Stub (process detection) |
| OpenCode | Stub (process detection) |

## Install

```sh
cargo install caw
```

## Usage

```sh
caw              # start Tauri desktop app
caw tui          # interactive terminal dashboard
caw serve        # headless daemon (logs events)
caw status       # one-line output for shell prompts: "3w 1a 2i"
```

### Status Symbols

| Symbol | Status | Color |
|--------|--------|-------|
| ● | Working | Teal `#1D9E75` |
| ▲ | Waiting for input | Amber `#EF9F27` |
| ◉ | Idle | Gray `#888780` |
| ✕ | Dead | Red `#E24B4A` |

## Architecture

```
caw/
├── crates/
│   ├── caw-core/              # IPlugin trait, types, Monitor engine
│   ├── caw-plugin-claude/     # Claude Code plugin
│   ├── caw-plugin-codex/      # OpenAI Codex CLI plugin
│   ├── caw-plugin-opencode/   # OpenCode plugin
│   ├── caw-tui/               # Ratatui TUI binary
│   └── caw-app/src-tauri/     # Tauri v2 desktop app
└── ui/                        # React + TypeScript frontend
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to add a new plugin.

## License

MIT
