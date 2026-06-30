# Заявка на конкурс: vimit

## Название

`vimit` - безопасный монитор лимитов VibeMode / VibeMod.PRO для вайбкодеров.

## Что делает

`vimit` показывает текущий расход и остатки лимитов VibeMode по данным `GET /v1/me`:

- CLI-вывод для быстрых проверок и скриптов.
- TUI-dashboard в терминале для постоянного мониторинга.
- Slint GUI с карточками лимитов, недельным donut chart и tray-индикатором.
- Маленькое floating overlay поверх окон с графиком расхода, pin, compact-режимом, drag/resize и переключением единиц (`кред/мин`, `ток/мин`, `%/час`).
- Desktop notifications по warning/danger/recovery.
- `doctor` и `init` для диагностики и первичной настройки.
- Demo/mock режимы без реального API-ключа.

Проект решает практическую боль: во время работы в Codex, Droid, Claude Code, Cursor и других инструментах пользователь видит лимиты заранее, а не узнаёт о проблеме после отказа API.

## Для кого полезно

- Пользователям VibeMode / VibeMod.PRO, которые активно работают через API.
- Вайбкодерам, которые держат открытыми Codex, Droid, Claude Code, Cursor и хотят видеть лимиты рядом с рабочим окном.
- Тем, кому нужен локальный, безопасный и проверяемый монитор без отправки ключей в сторонние сервисы.
- Тем, кто хочет диагностировать конфиги, `.env`, endpoint и сетевые ошибки без ручного разбора логов.

## GitHub

https://github.com/xodapi/vimit

## Как запустить

### Быстрая проверка без ключа

```powershell
git clone https://github.com/xodapi/vimit.git
cd vimit
cargo run --locked -- --demo
cargo run --locked -- --monitor --demo
cargo run --features gui --locked --bin vimit-gui -- --demo
```

### Сборка release-бинарей

```powershell
cargo build --release --features gui --locked
```

После сборки:

```powershell
.\target\release\vimit.exe --demo
.\target\release\vimit.exe --monitor --demo
.\target\release\vimit.exe --overlay --demo
.\target\release\vimit-gui.exe --demo
```

### Настройка live-лимитов

```powershell
.\target\release\vimit.exe --init
.\target\release\vimit.exe doctor
```

Основной ключ:

```powershell
VIBEMODE_API_KEY=<your-api-key>
```

Опциональный endpoint:

```powershell
VIBEMODE_API_BASE=https://r-api.vibemod.pro
```

Для VPN-режима используется актуальный endpoint VibeMod.PRO:

```powershell
.\target\release\vimit.exe --vpn
```

## Какие ОС поддерживаются

- Windows x86_64.
- Linux x86_64 / aarch64.
- macOS aarch64.

GUI построен на Slint, TUI построен на ratatui. CI проверяет Windows, Linux и macOS.

## Что уже работает

- Получение лимитов через `GET /v1/me`.
- Окна лимитов: 5h, 24h, 7d, 30d.
- Расчёт процентов, remaining, reset countdown и уровней `ok` / `warning` / `danger`.
- CLI human / compact / JSON вывод.
- TUI-monitor с темами и режимами отображения.
- GUI-dashboard с карточками, donut chart и настройками.
- Floating overlay поверх окон, включая compact-режим, drag/resize, pin и график расхода.
- Tray tooltip/icon синхронизированы с текущим dashboard state.
- Desktop notifications и recovery notifications.
- Cache fallback без хранения API-ключа в открытом виде в cache key.
- `vimit --init`, `vimit doctor`, account config, demo/mock режимы.
- Защита от spam старым warning про `NEUROGATE_API_BASE`.
- Базовые GUI-enabled тесты для overlay helpers и tray status formatting.
- Локальные проверки: `cargo fmt --check`, `cargo test --locked`, `cargo test --locked --features gui`, `cargo clippy --all-targets -- -D warnings`, `cargo clippy --all-targets --features gui -- -D warnings`, `cargo build --release --features gui --locked`.

## Что ещё не готово

- Финальные скриншоты/видео нужно добавить после визуального подтверждения текущего GUI/overlay.
- GitHub Release с тегом и готовыми архивами нужно создавать отдельно после ручного решения о релизе.
- Полноценные Slint UI interaction tests через публичный `slint::testing::*` зависят от доступности такого API/backend в используемой версии Slint. Сейчас добавлены поддерживаемые GUI-enabled unit tests для логики overlay/tray.

## Безопасность

- API-ключ берётся из `VIBEMODE_API_KEY` или локального `.env`.
- Ключи, токены, cookies и личные данные не логируются.
- Demo/mock режимы позволяют проверить интерфейс без реального ключа.
- Сетевые запросы для лимитов идут к VibeMode / VibeMod.PRO API, основной публичный endpoint: `https://r-api.vibemod.pro`, VPN endpoint: `https://api.vibemod.pro`.
- Старые `NEUROGATE_*` переменные поддерживаются только как legacy fallback после ребрендинга, пользователю показывается актуальное имя `VIBEMODE_*`.

## Скрин/видео

Финальные материалы нужно приложить к Telegram-заявке после визуальной проверки:

- Screenshot CLI/TUI.
- Screenshot основного GUI.
- Screenshot floating overlay поверх рабочего окна.
- Короткое видео: запуск `vimit --overlay --demo`, drag/resize overlay, переключение единиц и tray tooltip.

## Готовый текст для отправки в ветку конкурса

Название: vimit

Что делает: локально показывает лимиты VibeMode / VibeMod.PRO из `GET /v1/me`: CLI, TUI, GUI, tray, desktop notifications и маленькое overlay поверх окон с графиком расхода.

Для кого полезно: для вайбкодеров, которые работают через VibeMode API в Codex, Droid, Claude Code, Cursor и хотят заранее видеть расход, reset и риск достижения лимитов.

GitHub: https://github.com/xodapi/vimit

Как запустить:

```powershell
git clone https://github.com/xodapi/vimit.git
cd vimit
cargo build --release --features gui --locked
.\target\release\vimit.exe --demo
.\target\release\vimit.exe --monitor --demo
.\target\release\vimit.exe --overlay --demo
```

Live-режим: создать `.env` с `VIBEMODE_API_KEY`, затем запустить `vimit doctor` и `vimit`.

Какие ОС поддерживаются: Windows x86_64, Linux x86_64/aarch64, macOS aarch64.

Что уже работает: polling `GET /v1/me`, проценты по 5h/24h/7d/30d, reset countdown, CLI/TUI/GUI, draggable/resizable overlay, tray sync, notifications, setup/doctor, demo/mock, базовые тесты и CI.

Что ещё не готово: финальный tagged GitHub Release и скрин/видео после визуального approve.

Скрин/видео: приложить screenshots GUI/overlay/TUI и короткое видео overlay после финального визуального просмотра.
