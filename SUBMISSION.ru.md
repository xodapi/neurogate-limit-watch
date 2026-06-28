# Заявка на конкурс (v0.6.2)

Название: vimit
Что делает: Нативная Rust CLI-утилита одним бинарником. Показывает текущий расход лимитов VibeMode по окнам 5 часов, 24 часа, 7 дней и 30 дней через безопасный polling `GET /v1/me`. Умеет выводить человекочитаемый статус, JSON для виджетов/автоматизаций, полноэкранный live-monitor в стиле `abtop` и опционально добавлять локальный статус AI-агентов из `abtop --status-json`. Может показывать локальные desktop-уведомления при переходе окна лимита в warning/danger.
Для кого полезно: Для пользователей VibeMode, которые работают через Codex, Droid, Claude Code, Cursor или другие OpenAI-compatible клиенты и хотят заранее видеть риск упереться в лимиты.
GitHub: https://github.com/xodapi/vimit
Как запустить: Скачать архив под свою ОС из GitHub Releases, распаковать и выполнить `vimit --demo`.
Какие ОС поддерживаются: Windows, Linux, macOS, Android/Termux
Что уже работает:
- Нативный Rust CLI без Python/pip/venv.
- Polling VibeMode `/v1/me`.
- Расчет credit/request usage для 5h / 24h / 7d / 30d.
- Human output и JSON output.
- Compact output для виджетов/status bar.
- Full-screen live monitor в стиле abtop: quota, alerts, reset timers, remaining credits/requests, hotkeys `r`, `q`, `Esc`.
- `.env` рядом с бинарником или в рабочей директории.
- Windows double-click mode без мгновенного закрытия окна.
- Настраиваемые пороги `--warning` / `--danger`.
- Локальные desktop-уведомления `--notify` с дедупликацией по эскалации уровня.
- Demo/mock-режим без ключа и сети.
- Опциональная интеграция с `abtop --status-json`.
- CI и release workflow для бинарников.
- Система self-update (`vimit update` / `vimit update --check`).
- System tray tooltip с процентами окон.
- Панель настроек с авто-проверкой обновлений.
- Slint GUI с поддержкой dark/light темы.

Что ещё не готово:
- Нативный Android UI.
- Termux prebuilt binary.
- Remote push/webhook-уведомления, потому что публичного webhook/SSE для лимитов сейчас нет.
- Статистика по отдельным API-ключам, если VibeMode не отдаёт ее через публичный `/v1/*` endpoint.

Релизы:
https://github.com/xodapi/vimit/releases

Обсуждения и идеи:
https://github.com/xodapi/vimit/discussions

Бонус для новых пользователей VibeMode:
Регистрация по ссылке дает новому пользователю $5 на счет:
https://portal.vibemod.pro/?ref=cbvBMDP06DSwPL9u

Скрин/видео:
`assets/demo.svg`
