# neurogate-limit-watch

[English](README.md) | [Русский](README.ru.md)

[![CI](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml/badge.svg)](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml)

Single-binary Rust CLI for safely checking NeuroGate quota usage in
Codex/Droid/Claude/Cursor workflows.

`nglimit` polls NeuroGate `GET /v1/me`, summarizes credit/request usage for
5-hour, 24-hour, 7-day, and 30-day windows, includes an abtop-style live
terminal monitor, and can merge local
`abtop --status-json` agent status. It is built as a native executable, so
users do not need Python, pip, venv, Node, or API SDK dependencies.

![demo](assets/demo.svg)

## Why

Vibe coders need to know whether they can safely keep a Codex/Droid/Claude
session running or whether they are about to hit NeuroGate limits. The tool is
small, local-first, and intentionally avoids storing API keys or logging
private prompts.

## NeuroGate Referral Bonus

Optional: new NeuroGate users can register with this referral link and receive
`$5` on their account:

https://portal.neurogate.space/invite?ref=cbvBMDP06DSwPL9u

The referral link is not required to use this project. The CLI does not send
referral data anywhere.

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
```

Live monitor:

```bash
nglimit --monitor
nglimit --monitor --watch 10
nglimit --monitor --with-abtop
```

In monitor mode, press `r` to refresh immediately and `q` or `Esc` to quit.
It renders an abtop-style dashboard with NeuroGate quota windows, warning
alerts, reset timers, remaining credits/requests, and optional local
Codex/Claude agent context from `abtop --status-json`.

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
- No telemetry, no external calls except NeuroGate `/v1/me`.

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

- Windows
- macOS
- Linux
- Android/Termux should work from source with Rust installed; prebuilt Termux
  binaries are on the roadmap.

## Tests

```bash
cargo test --locked
cargo run --locked -- --demo --json
```

## Contest status

What works:

- Native Rust CLI.
- NeuroGate `/v1/me` polling.
- 5h / 24h / 7d / 30d credit and request windows.
- Human output and JSON output.
- Compact output for widgets/status bars.
- Full-screen live monitor with `q`/`Esc` quit and `r` refresh.
- `.env` file next to the binary or working directory.
- Custom warning/danger thresholds.
- Demo/mock mode without a key.
- Optional local abtop integration.
- CI and release workflow for native binaries.

Not ready yet:

- Native Android UI.
- Notifications/webhooks.
- Per-key statistics, because public `/v1/*` endpoints expose aggregate usage.
