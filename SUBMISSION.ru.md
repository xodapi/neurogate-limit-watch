# Заявка на конкурс

Название:
neurogate-limit-watch

Что делает:
Нативная Rust CLI-утилита одним бинарником. Показывает текущий расход лимитов
NeuroGate по окнам 5 часов, 24 часа, 7 дней и 30 дней через безопасный polling
`GET /v1/me`. Умеет выводить человекочитаемый статус, JSON для
виджетов/автоматизаций, полноэкранный live-monitor в стиле `abtop` и
опционально добавлять локальный статус AI-агентов из `abtop --status-json`.

Для кого полезно:
Для пользователей NeuroGate, которые работают через Codex, Droid, Claude Code,
Cursor или другие OpenAI-compatible клиенты и хотят заранее видеть риск
упереться в лимиты.

GitHub:
https://github.com/xodapi/neurogate-limit-watch

Релизы:
https://github.com/xodapi/neurogate-limit-watch/releases

Обсуждения и идеи:
https://github.com/xodapi/neurogate-limit-watch/discussions

Бонус для новых пользователей NeuroGate:
Регистрация по ссылке дает новому пользователю $5 на счет:
https://portal.neurogate.space/invite?ref=cbvBMDP06DSwPL9u

Как запустить:
Скачать архив под свою ОС из GitHub Releases, распаковать и выполнить:

```bash
nglimit --version
nglimit --demo
```

Windows:

```powershell
.\nglimit.exe --version
.\nglimit.exe --demo
```

Если запускать двойным кликом из Explorer, окно не закрывается сразу. Для
простого запуска в Windows-архиве также есть `nglimit-open.cmd`.

Из исходников:

```bash
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
cargo build --release --locked
target/release/nglimit --demo
```

С реальным ключом:

```bash
cp .env.example .env
# затем вписать ключ в .env:
# NEUROGATE_API_KEY=ваш_ключ
nglimit
nglimit --json
nglimit --compact
nglimit --monitor --with-abtop
```

Какие ОС поддерживаются:
Windows, macOS, Linux. Android/Termux планируется как отдельный build-рецепт.

Что уже работает:
- Нативный Rust CLI без Python/pip/venv.
- Polling NeuroGate `/v1/me`.
- Расчет credit/request usage для 5h / 24h / 7d / 30d.
- Human output и JSON output.
- Compact output для виджетов/status bar.
- Full-screen live monitor в стиле abtop: quota, alerts, reset timers,
  remaining credits/requests, hotkeys `r`, `q`, `Esc`.
- `.env` рядом с бинарником или в рабочей директории.
- Windows double-click mode без мгновенного закрытия окна.
- Настраиваемые пороги `--warning` / `--danger`.
- Demo/mock-режим без ключа и сети.
- Опциональная интеграция с `abtop --status-json`.
- CI и release workflow для бинарников.

Что ещё не готово:
- Нативный Android UI.
- Termux prebuilt binary.
- Push-уведомления, потому что публичного webhook/SSE для лимитов сейчас нет.
- Статистика по отдельным API-ключам, если NeuroGate не отдаёт ее через
  публичный `/v1/*` endpoint.

Скрин/видео:
`assets/demo.svg`
