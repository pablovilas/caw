<p align="left">
  <img src="docs/banner.png" alt="caw — coding assistant watcher" width="460">
</p>

# caw

**coding assistant watcher**

Watch every coding agent from one quiet perch.

[![CI](https://github.com/pablovilas/caw/actions/workflows/ci.yml/badge.svg)](https://github.com/pablovilas/caw/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

`caw` watches the coding assistants running on your machine and turns them into one calm operational view. Scan status, group sessions, and jump back to the right terminal without digging around.

## Supported Assistants

| Assistant | Status |
|-----------|--------|
| Claude Code | Supported |
| OpenAI Codex CLI | Stub (process detection) |
| OpenCode | Stub (process detection) |

## Install

```sh
brew install pablovilas/tap/caw
```

Or build from source:

```sh
cargo install --git https://github.com/pablovilas/caw.git
```

## Usage

```
caw              Auto-detect (terminal → dashboard, background → tray)
caw watch        Live interactive dashboard
caw tray         Menu bar app with session overview
caw status       One-line status for shell prompts (e.g. 2w 1a 0i)
caw serve        Run as a background daemon
caw debug        Print raw process discovery info
```

### Status

| Symbol | Status | Meaning |
|--------|--------|---------|
| ● | working | Session is actively generating |
| ▲ | waiting | Waiting for user input |
| ◉ | idle | Present but inactive |
| ✕ | dead | Process ended or disconnected |

## Architecture

```
caw/
├── crates/
│   ├── caw-core/              # Plugin trait, types, monitor engine
│   ├── caw-plugin-claude/     # Claude Code plugin
│   ├── caw-plugin-codex/      # Codex CLI plugin
│   ├── caw-plugin-opencode/   # OpenCode plugin
│   └── caw/                   # Binary: tray, dashboard, CLI
└── docs/                      # Assets and documentation
```

## Development

```sh
just setup    # Configure git hooks
just ci       # Run lint + tests
just build    # Build release binary
just run      # Run in dev mode
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT
