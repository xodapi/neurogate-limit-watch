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

## Living Token Creature / Живое существо

The overlay creature is meant to explain quota state without guilt or
punishment mechanics. It reacts to token/credit spend, but it does not die and
does not punish inactivity. When there is no spend for a while, it sleeps.

Current implemented behavior:

- `Sleeping`: no recent credit spend, muted color, slower pulse, no sounds.
- `Awake`: normal spend below warning threshold, calm pulse.
- `Alert`: warning threshold, amber state and faster motion.
- `Critical`: danger threshold, red state and fastest motion.
- `Recovery`: detected window reset, short visual "exhale" state.

Planned improvements:

- Low-motion and silent modes for distraction-sensitive users.
- User-selectable behavior presets, sound themes, and creature skins.
- Clear event model for sleep, wake, threshold crossing, and reset events.
- Optional Android-friendly presentation that works as a notification/widget
  first, then native overlay only if Android permissions and UX are safe.

Русский: существо "питается" расходом токенов/кредитов, но не умирает и не
голодает. При простое оно спокойно спит, при приближении к лимиту становится
настороженным, при сбросе окна показывает короткий "выдох". Публичное описание
не использует чужие товарные знаки.

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
