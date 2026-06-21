# neurogate-limit-watch

Safe NeuroGate quota monitor for Codex/Droid workflows.

`nglimit` polls NeuroGate `GET /v1/me`, summarizes credit/request usage for
5-hour, 24-hour, 7-day, and 30-day windows, and can merge local
`abtop --status-json` agent status. It is designed for quick terminal checks,
CI guards, and future mobile/widget integrations.

![demo](assets/demo.svg)

## Why

Vibe coders need to know whether they can safely keep a Codex/Droid/Claude
session running or whether they are about to hit NeuroGate limits. The tool is
small, local-first, and intentionally avoids storing API keys or logging
private prompts.

## Install

Requirements: Python 3.10+.

```bash
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
python -m pip install -e .
```

No dependencies are required.

On Windows, if `python` opens the Microsoft Store launcher, use `py -3`
instead:

```powershell
py -3 -m pip install -e .
```

## Usage

Use a real NeuroGate key:

```bash
export NEUROGATE_API_KEY="YOUR_NEUROGATE_API_KEY"
nglimit
nglimit --json
nglimit --with-abtop
```

Try it without a key:

```bash
python -m nglimit --mock tests/fixtures/me.json
python -m nglimit --mock tests/fixtures/me.json --json
```

Windows PowerShell equivalent:

```powershell
$env:NEUROGATE_API_KEY = "YOUR_NEUROGATE_API_KEY"
nglimit
py -3 -m nglimit --mock tests\fixtures\me.json
```

Watch mode:

```bash
nglimit --watch 60 --with-abtop
```

CI/automation threshold:

```bash
nglimit --fail-on warning
nglimit --fail-on danger --json
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

## Uninstall

```bash
python -m pip uninstall neurogate-limit-watch
```

Then remove the cloned directory if you no longer need it.

## Supported OS

- Windows
- macOS
- Linux
- Android/Termux should work with Python 3.10+; `--with-abtop` requires an
  `abtop` build available in `PATH`.

## Tests

```bash
python -m unittest discover -s tests -v
```

## Contest status

What works:

- NeuroGate `/v1/me` polling.
- 5h / 24h / 7d / 30d credit and request windows.
- Human output and JSON output.
- Mock mode for demos without a key.
- Optional local abtop integration.
- Basic tests.

Not ready yet:

- Native Android UI.
- Notifications/webhooks.
- Per-key statistics, because public `/v1/*` endpoints expose aggregate usage.
