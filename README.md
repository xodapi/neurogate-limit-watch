# vimit

[English](README.md) | [Русский](README.ru.md)

[![CI](https://github.com/xodapi/vimit/actions/workflows/ci.yml/badge.svg)](https://github.com/xodapi/vimit/actions/workflows/ci.yml)

**vimit** — single native binary to monitor VibeMode quota in real time.

Polls `GET /v1/me`, summarizes credit/request windows (5h / 24h / 7d / 30d),
renders a live TUI dashboard (or JSON / compact text), sends desktop
notifications on threshold breach, and supports multiple VibeMode accounts.

No Python, Node, or SDK dependencies — just one executable.

## Quick Start

```bash
# Download from releases, then:
vimit --demo                # try without an API key
vimit --demo --monitor      # full-screen dashboard
vimit --init                # interactive setup wizard
vimit                       # real VibeMode limits
vimit --doctor              # system diagnostics
```

## Why

Vibe coders need to know whether they can safely keep a Codex/Droid/Claude
session running or whether they are about to hit VibeMode limits. The tool is
small, local-first, and intentionally avoids storing API keys or logging
private prompts.

## Features

- **Multiple output modes**: human, JSON (`--json`), compact one-line (`--compact`)
- **Live TUI monitor** (`--monitor`): ratatui dashboard with gauges, sparklines, color themes
- **Monitor presets**: `full` (2-column grid), `compact` (single-column), `mini` (one-liner)
- **12 color themes**: btop, dracula, catppuccin, tokyo-night, gruvbox, nord, high-contrast, protanopia, deuteranopia, tritanopia, solarized, monokai
- **Multi-account**: `accounts.toml` profiles, Tab switching in TUI, dropdown in GUI
- **Desktop notifications** (`--notify`): alert on warning/danger, no-repeat logic
- **Custom thresholds** (`--warning`, `--danger`, `--threshold`): per-window warning/danger levels
- **CI integration** (`--fail-on`): exit non-zero when threshold is breached
- **Watch mode** (`--watch N`): periodic polling every N seconds
- **abtop integration** (`--with-abtop`): merge local Codex/Claude agent status
- **Diagnostics** (`--doctor`): validate config, accounts, env, API connectivity
- **Setup wizard** (`--init`): interactive config, .env, and API key setup
- **GUI** (`--features gui`): Slint-based desktop window (optional)
- **Safe by design**: API key from env only, never logged, no telemetry

## Download

Release binaries:

https://github.com/xodapi/vimit/releases

Pick the archive for your platform, unpack it, then run:

```bash
vimit --version
vimit --demo
vimit --demo --monitor
```

Windows PowerShell:

```powershell
.\vimit.exe --version
.\vimit.exe --demo
.\vimit.exe --demo --monitor
```

If you double-click `vimit.exe` in Explorer, the Windows console will stay
open after the command finishes. The Windows archive also includes
`vimit-open.cmd`, a double-click helper that always pauses at the end, and
`vimit-monitor.cmd` for launching the live monitor directly.

## .env Next To The Binary

You can keep the VibeMode API key in a local `.env` file next to `vimit`
or in the directory where you run it. The release archive includes
`.env.example`.

```bash
cp .env.example .env
```

Edit `.env`:

```dotenv
VIBEMODE_API_KEY=YOUR_VIBEMODE_API_KEY
VIBEMODE_API_BASE=https://r-api.vibemod.pro
```

Then run:

```bash
vimit
vimit --compact
vimit --json
```

Windows PowerShell:

```powershell
Copy-Item .env.example .env
notepad .env
.\vimit.exe --compact
```

Double-click option on Windows:

```text
vimit-open.cmd
vimit-monitor.cmd
```

Lookup order:

1. `--env-file <PATH>`
2. `.env` in the current directory
3. `.env` next to the `vimit` executable

Real environment variables have priority over `.env` values. `.env` is ignored
by git and should not be committed.

## Build From Source

Requirements: Rust stable.

```bash
git clone https://github.com/xodapi/vimit.git
cd vimit
cargo build --release --locked
```

Binary location:

- Windows: `target/release/vimit.exe`
- Linux/macOS: `target/release/vimit`

## Diagnostics

Check system health:

```bash
vimit --doctor
```

Example output:

```
vimit doctor — system diagnostics

  [✓] config.toml found: /home/user/.config/vimit/config.toml
  [✓] config.toml is valid TOML
  [✓] accounts.toml: 2 account(s): dev, prod

  environment:
       HOME: /home/user
       NEUROGATE_API_BASE: (not set, will use default)
       NEUROGATE_API_KEY: (set)

  testing API connection to https://r-api.vibemod.pro... OK (4 window(s))

  status: all checks passed
```

## Interactive Setup

```bash
vimit --init
```

Creates config directory, config.toml with defaults, optionally sets up
`.env` with your API key, and tests the connection.

## Usage

Try without a key or network:

```bash
vimit --demo
vimit --demo --json
```

Use a real VibeMode key:

```bash
export VIBEMODE_API_KEY="YOUR_VIBEMODE_API_KEY"
vimit
vimit --json
vimit --with-abtop
```

Windows PowerShell:

```powershell
$env:VIBEMODE_API_KEY = "YOUR_VIBEMODE_API_KEY"
.\vimit.exe
.\vimit.exe --json
```

Mock a saved `/v1/me` payload:

```bash
vimit --mock tests/fixtures/me.json
vimit --mock tests/fixtures/me.json --json
```

Watch mode:

```bash
vimit --watch 60 --with-abtop
vimit --watch 60 --notify
```

Live monitor:

```bash
vimit --monitor
vimit --monitor --watch 10
vimit --monitor --with-abtop
vimit --monitor --notify
```

Monitor presets for different terminal sizes:

```bash
vimit --monitor --preset full      # 2-column grid, sparklines (default)
vimit --monitor --preset compact   # single-column, gauge + metrics
vimit --monitor --preset mini      # one line per window, minimal
```

Per-window thresholds:

```bash
vimit --monitor --threshold 5h=80:95,7d=90
vimit --fail-on warning --threshold 24h=85:98
```

Format: `KEY=WARNING[:DANGER]` where KEY is one of `5h`, `24h`, `7d`, `30d`.
Per-window thresholds override `--warning`/`--danger` for those windows.

In monitor mode, press `r` to refresh immediately and `q` or `Esc` to quit.
It renders an abtop-style dashboard with VibeMode quota windows, warning
alerts, reset timers, remaining credits/requests, and optional local
Codex/Claude agent context from `abtop --status-json`.

Desktop notifications:

```bash
vimit --notify
vimit --watch 60 --notify
vimit --monitor --notify
```

`--notify` sends a local desktop alert when a quota window escalates into
`warning` or `danger`. It keeps one in-process state map, so polling and
monitor loops do not spam the same alert on every refresh. De-escalation after a
window reset is silent.

CI/automation threshold:

```bash
vimit --fail-on warning
vimit --fail-on danger --json
vimit --warning 80 --danger 95 --fail-on warning
```

Compact one-line output for widgets/status bars:

```bash
vimit --compact
vimit --compact --with-abtop
```

## Output

Human output:

```text
VibeMode limits
  5h   warning reset in 2h 30m
       credits  39/50 (78.0%, left 11)
       requests 610/1000 (61.0%, left 390)
```

JSON output:

```json
{
  "source": "vibemode",
  "windows": [
    {
      "window": "5h",
      "level": "warning",
      "credits": { "used": 39.0, "limit": 50.0, "remaining": 11.0, "percent": 78.0 }
    }
  ],
  "abtop": null
}
```

## Safety

- API key is read from the environment variable `VIBEMODE_API_KEY` (with `NEUROGATE_API_KEY` supported as a fallback).
- The key is never written to disk.
- Errors do not print the key.
- JSON output intentionally omits account identity fields.
- `--with-abtop` uses `abtop --status-json`, which is the compact,
  privacy-preserving abtop payload without local paths, prompts, chat text, or
  session IDs.
- `--monitor` uses the same privacy-safe summaries and keeps the API key out of
  the terminal output.
- `--notify` only passes quota summary text to a local OS notification helper:
  Windows PowerShell toast/fallback popup, macOS `osascript`, or Linux/BSD
  `notify-send`.
- No telemetry, no external network calls except VibeMode `/v1/me`.

## Configuration

Environment variables:

- `VIBEMODE_API_KEY`: VibeMode API key (fallback: `NEUROGATE_API_KEY`).
- `VIBEMODE_API_BASE`: API base URL, default `https://r-api.vibemod.pro` (fallback: `NEUROGATE_API_BASE`).
- `ABTOP_BIN`: abtop binary path, default `abtop`.

CLI options:

```bash
vimit --help
```

## Discussions

Ideas, feature requests, and VibeMode/Codex/Droid workflow notes are welcome
in GitHub Discussions:

https://github.com/xodapi/vimit/discussions

See [ROADMAP.md](ROADMAP.md) for the current improvement backlog.

## Supported OS

- Windows (x86_64)
- macOS (aarch64)
- Linux (x86_64, aarch64)
- Android/Termux — see [docs/termux.md](docs/termux.md)

## Tests

```bash
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo run --locked -- --demo --json
```

## What works

- Native Rust CLI and GUI (build with `--features gui`).
- VibeMode `/v1/me` polling with robust schema tolerance.
- 5h / 24h / 7d / 30d credit and request windows.
- Human, JSON, and compact output modes with ANSI color coding.
- Full-screen ratatui monitor with gauges, sparklines, color coding.
- Monitor presets: `full`, `compact`, `mini` for different terminal sizes.
- 12 color themes including accessibility-optimized (protanopia, deuteranopia, tritanopia, high-contrast).
- Per-window thresholds: `--threshold 5h=80:95,7d=90`.
- `.env` file next to the binary or working directory.
- Custom warning/danger thresholds.
- Desktop notifications with escalation tracking.
- Multi-account support via `accounts.toml` (Tab switching in TUI, dropdown in GUI).
- Demo/mock mode without a key.
- Optional local abtop integration.
- `--doctor` system diagnostics.
- `--init` interactive setup wizard.
- Actionable error messages with suggestions.
- CI and release workflow for native binaries (Windows, Linux, macOS, ARM).
- PowerShell install/uninstall scripts.
- Termux/Android install guide.
