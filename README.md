# neurogate-limit-watch

[English](README.md) | [Русский](README.ru.md)

[![CI](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml/badge.svg)](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml)

**nglimit** — single native binary to monitor NeuroGate quota in real time.

Polls `GET /v1/me`, summarizes credit/request windows (5h / 24h / 7d / 30d),
renders a live TUI dashboard (or JSON / compact text), sends desktop
notifications on threshold breach, and supports multiple NeuroGate accounts.

No Python, Node, or SDK dependencies — just one executable.

![demo](assets/demo.svg)

## Quick Start

```bash
# Download from releases, then:
nglimit --demo                # try without an API key
nglimit --demo --monitor      # full-screen dashboard
nglimit --init                # interactive setup wizard
nglimit                       # real NeuroGate limits
nglimit --doctor              # system diagnostics
```

## Why

Vibe coders need to know whether they can safely keep a Codex/Droid/Claude
session running or whether they are about to hit NeuroGate limits. The tool is
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

https://github.com/xodapi/neurogate-limit-watch/releases

Pick the archive for your platform, unpack it, then run:

```bash
nglimit --version
nglimit --demo
nglimit --demo --monitor
```

Windows PowerShell:

```powershell
.\nglimit.exe --version
.\nglimit.exe --demo
.\nglimit.exe --demo --monitor
```

If you double-click `nglimit.exe` in Explorer, the Windows console will stay
open after the command finishes. The Windows archive also includes
`nglimit-open.cmd`, a double-click helper that always pauses at the end, and
`nglimit-monitor.cmd` for launching the live monitor directly.

## .env Next To The Binary

You can keep the NeuroGate API key in a local `.env` file next to `nglimit`
or in the directory where you run it. The release archive includes
`.env.example`.

```bash
cp .env.example .env
```

Edit `.env`:

```dotenv
NEUROGATE_API_KEY=YOUR_NEUROGATE_API_KEY
NEUROGATE_API_BASE=https://api.neurogate.space
```

Then run:

```bash
nglimit
nglimit --compact
nglimit --json
```

Windows PowerShell:

```powershell
Copy-Item .env.example .env
notepad .env
.\nglimit.exe --compact
```

Double-click option on Windows:

```text
nglimit-open.cmd
nglimit-monitor.cmd
```

Lookup order:

1. `--env-file <PATH>`
2. `.env` in the current directory
3. `.env` next to the `nglimit` executable

Real environment variables have priority over `.env` values. `.env` is ignored
by git and should not be committed.

## Build From Source

Requirements: Rust stable.

```bash
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
cargo build --release --locked
```

Binary location:

- Windows: `target/release/nglimit.exe`
- Linux/macOS: `target/release/nglimit`

## Diagnostics

Check system health:

```bash
nglimit --doctor
```

Example output:

```
nglimit doctor — system diagnostics

  [✓] config.toml found: /home/user/.config/nglimit/config.toml
  [✓] config.toml is valid TOML
  [✓] accounts.toml: 2 account(s): dev, prod

  environment:
       HOME: /home/user
       NEUROGATE_API_BASE: (not set, will use default)
       NEUROGATE_API_KEY: (set)

  testing API connection to https://api.neurogate.space... OK (4 window(s))

  status: all checks passed
```

## Interactive Setup

```bash
nglimit --init
```

Creates config directory, config.toml with defaults, optionally sets up
`.env` with your API key, and tests the connection.

## Usage

Try without a key or network:

```bash
nglimit --demo
nglimit --demo --json
```

Use a real NeuroGate key:

```bash
export NEUROGATE_API_KEY="YOUR_NEUROGATE_API_KEY"
nglimit
nglimit --json
nglimit --with-abtop
```

Windows PowerShell:

```powershell
$env:NEUROGATE_API_KEY = "YOUR_NEUROGATE_API_KEY"
.\nglimit.exe
.\nglimit.exe --json
```

Mock a saved `/v1/me` payload:

```bash
nglimit --mock tests/fixtures/me.json
nglimit --mock tests/fixtures/me.json --json
```

Watch mode:

```bash
nglimit --watch 60 --with-abtop
nglimit --watch 60 --notify
```

Live monitor:

```bash
nglimit --monitor
nglimit --monitor --watch 10
nglimit --monitor --with-abtop
nglimit --monitor --notify
```

Monitor presets for different terminal sizes:

```bash
nglimit --monitor --preset full      # 2-column grid, sparklines (default)
nglimit --monitor --preset compact   # single-column, gauge + metrics
nglimit --monitor --preset mini      # one line per window, minimal
```

Per-window thresholds:

```bash
nglimit --monitor --threshold 5h=80:95,7d=90
nglimit --fail-on warning --threshold 24h=85:98
```

Format: `KEY=WARNING[:DANGER]` where KEY is one of `5h`, `24h`, `7d`, `30d`.
Per-window thresholds override `--warning`/`--danger` for those windows.

In monitor mode, press `r` to refresh immediately and `q` or `Esc` to quit.
It renders an abtop-style dashboard with NeuroGate quota windows, warning
alerts, reset timers, remaining credits/requests, and optional local
Codex/Claude agent context from `abtop --status-json`.

Desktop notifications:

```bash
nglimit --notify
nglimit --watch 60 --notify
nglimit --monitor --notify
```

`--notify` sends a local desktop alert when a quota window escalates into
`warning` or `danger`. It keeps one in-process state map, so polling and
monitor loops do not spam the same alert on every refresh. De-escalation after a
window reset is silent.

CI/automation threshold:

```bash
nglimit --fail-on warning
nglimit --fail-on danger --json
nglimit --warning 80 --danger 95 --fail-on warning
```

Compact one-line output for widgets/status bars:

```bash
nglimit --compact
nglimit --compact --with-abtop
```

## Output

Human output:

```text
NeuroGate limits
  5h   warning reset in 2h 30m
       credits  39/50 (78.0%, left 11)
       requests 610/1000 (61.0%, left 390)
```

JSON output:

```json
{
  "source": "neurogate",
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

- API key is read only from the environment variable `NEUROGATE_API_KEY`.
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
- No telemetry, no external network calls except NeuroGate `/v1/me`.

## Configuration

Environment variables:

- `NEUROGATE_API_KEY`: NeuroGate API key.
- `NEUROGATE_API_BASE`: API base URL, default `https://api.neurogate.space`.
- `ABTOP_BIN`: abtop binary path, default `abtop`.

CLI options:

```bash
nglimit --help
```

## Discussions

Ideas, feature requests, and NeuroGate/Codex/Droid workflow notes are welcome
in GitHub Discussions:

https://github.com/xodapi/neurogate-limit-watch/discussions

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
- NeuroGate `/v1/me` polling with robust schema tolerance.
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
