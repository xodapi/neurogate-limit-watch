# Заявка на конкурс

Название:
neurogate-limit-watch

Что делает:
Показывает текущий расход лимитов NeuroGate по окнам 5 часов, 24 часа, 7 дней и 30 дней через безопасный polling `GET /v1/me`. Умеет выводить человекочитаемый статус, JSON для виджетов/автоматизаций и опционально добавлять локальный статус AI-агентов из `abtop --status-json`.

Для кого полезно:
Для пользователей NeuroGate, которые работают через Codex, Droid, Claude Code, Cursor или другие OpenAI-compatible клиенты и хотят заранее видеть риск упереться в лимиты.

GitHub:
https://github.com/xodapi/neurogate-limit-watch

Как запустить:
```bash
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
python -m pip install -e .
export NEUROGATE_API_KEY="ваш_ключ"
nglimit
```

Без ключа для проверки:
```bash
python -m nglimit --mock tests/fixtures/me.json
python -m nglimit --mock tests/fixtures/me.json --json
```

Какие ОС поддерживаются:
Windows, macOS, Linux. Android/Termux должен работать при наличии Python 3.10+.

Что уже работает:
- Polling NeuroGate `/v1/me`.
- Расчет credit/request usage для 5h / 24h / 7d / 30d.
- Human output и JSON output.
- Mock-режим без ключа и сети.
- Опциональная интеграция с `abtop --status-json`.
- Базовые тесты и GitHub Actions CI.

Что ещё не готово:
- Нативный Android UI.
- Push-уведомления, потому что публичного webhook/SSE для лимитов сейчас нет.
- Статистика по отдельным API-ключам, если NeuroGate не отдаёт ее через публичный `/v1/*` endpoint.

Скрин/видео:
`assets/demo.svg`
