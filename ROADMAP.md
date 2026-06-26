# Roadmap / План улучшений

This file keeps public improvement ideas in one place. Please discuss and vote
in GitHub Discussions:

https://github.com/xodapi/vimit/discussions

## High Impact / Самое полезное

- Improve `--notify` with optional reset/recovery notices and Termux:API support.
- Per-window thresholds like `5h=80,7d=90`.
- Monitor presets/layouts for very narrow Droid/Termux terminals.
- More compact presets for Droid widgets, tmux, and CI logs.
- Better `/v1/me` schema tolerance when VibeMode adds new groups or model rows.
- PowerShell install script and uninstall script for Windows users.

## Droid / Termux

- Termux install guide.
- Termux:Widget example that runs `vimit --demo` or `vimit --json`.
- Optional Android notification through Termux:API.

## abtop / Agent Monitoring

- Expand the new `--monitor` dashboard with ports/process panes when safe
  privacy-preserving local sources are available.
- Add examples for Codex, Claude Code, Droid, and Cursor workflows.
- Keep all local paths, prompts, session IDs, and tool calls out of exported JSON.

## Native Binaries / Нативные бинарники

The project is now Rust-first. The next release goal is to publish binaries for:

- `x86_64-pc-windows-msvc`
- `x86_64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- Termux/Android target after the build recipe is verified

Русский: проект теперь Rust-first. Основная цель релизов — один бинарник без
Python, pip, venv и runtime-зависимостей.
