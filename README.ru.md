# neurogate-limit-watch

[English](README.md) | [Русский](README.ru.md)

[![CI](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml/badge.svg)](https://github.com/xodapi/neurogate-limit-watch/actions/workflows/ci.yml)

Rust CLI одним бинарником для безопасной проверки лимитов NeuroGate в рабочих
процессах Codex/Droid/Claude/Cursor.

`nglimit` опрашивает NeuroGate `GET /v1/me`, показывает расход credit/request
по окнам 5 часов, 24 часа, 7 дней и 30 дней, а также может добавить локальный
статус AI-агентов из `abtop --status-json`. Это нативный исполняемый файл:
пользователю не нужны Python, pip, venv, Node или API SDK.

![demo](assets/demo.svg)

## Зачем

Пользователю NeuroGate полезно заранее понимать, можно ли спокойно продолжать
сессию Codex/Droid/Claude/Cursor или лимиты уже близко. Проект маленький,
локальный и не хранит API-ключи, промпты или приватные сообщения.

## Бонус NeuroGate

Опционально: новые пользователи NeuroGate могут зарегистрироваться по
реферальной ссылке и получить `$5` на счет:

https://portal.neurogate.space/invite?ref=cbvBMDP06DSwPL9u

Эта ссылка не нужна для работы утилиты. CLI никуда не отправляет referral-данные.

## Скачать

Бинарные релизы:

https://github.com/xodapi/neurogate-limit-watch/releases

Скачайте архив под свою платформу, распакуйте и запустите:

```bash
nglimit --version
nglimit --demo
```

Windows PowerShell:

```powershell
.\nglimit.exe --version
.\nglimit.exe --demo
```

Если запустить `nglimit.exe` двойным кликом из Explorer, Windows-консоль
останется открытой после завершения команды. В Windows-архив также входит
`nglimit-open.cmd`: helper для двойного клика, который всегда делает паузу.

## .env рядом с бинарником

Ключ NeuroGate можно держать в локальном `.env` рядом с `nglimit` или в
директории, из которой вы запускаете команду. В релизный архив входит
`.env.example`.

```bash
cp .env.example .env
```

Отредактируйте `.env`:

```dotenv
NEUROGATE_API_KEY=YOUR_NEUROGATE_API_KEY
NEUROGATE_API_BASE=https://api.neurogate.space
```

После этого:

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

Вариант для двойного клика на Windows:

```text
nglimit-open.cmd
```

Порядок поиска:

1. `--env-file <PATH>`
2. `.env` в текущей директории
3. `.env` рядом с исполняемым файлом `nglimit`

Настоящие переменные окружения имеют приоритет над значениями из `.env`.
Файл `.env` добавлен в `.gitignore`, его не нужно коммитить.

## Сборка из исходников

Требуется Rust stable.

```bash
git clone https://github.com/xodapi/neurogate-limit-watch.git
cd neurogate-limit-watch
cargo build --release --locked
```

Где будет бинарник:

- Windows: `target/release/nglimit.exe`
- Linux/macOS: `target/release/nglimit`

## Использование

Проверить без ключа и без сети:

```bash
nglimit --demo
nglimit --demo --json
```

С реальным ключом NeuroGate:

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

Проверка сохраненного payload `/v1/me`:

```bash
nglimit --mock tests/fixtures/me.json
nglimit --mock tests/fixtures/me.json --json
```

Watch mode:

```bash
nglimit --watch 60 --with-abtop
```

Порог для CI/автоматизаций:

```bash
nglimit --fail-on warning
nglimit --fail-on danger --json
nglimit --warning 80 --danger 95 --fail-on warning
```

Компактный вывод одной строкой для виджетов/status bar:

```bash
nglimit --compact
nglimit --compact --with-abtop
```

## Вывод

Человекочитаемый вывод:

```text
NeuroGate limits
  5h   warning reset in 2h 30m
       credits  39/50 (78.0%, left 11)
       requests 610/1000 (61.0%, left 390)
```

JSON:

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

## Безопасность

- API-ключ читается только из переменной окружения `NEUROGATE_API_KEY`.
- Ключ не записывается на диск.
- Ошибки не печатают ключ.
- JSON-вывод не содержит identity-поля аккаунта.
- `--with-abtop` использует `abtop --status-json`: компактный privacy-safe
  payload без локальных путей, промптов, текста чатов и session ID.
- Нет телеметрии и внешних запросов, кроме NeuroGate `/v1/me`.

## Настройка

Переменные окружения:

- `NEUROGATE_API_KEY`: API-ключ NeuroGate.
- `NEUROGATE_API_BASE`: API base URL, по умолчанию `https://api.neurogate.space`.
- `ABTOP_BIN`: путь к бинарнику abtop, по умолчанию `abtop`.

CLI:

```bash
nglimit --help
```

## Обсуждения

Идеи, предложения и заметки по NeuroGate/Codex/Droid workflow можно оставлять
в GitHub Discussions:

https://github.com/xodapi/neurogate-limit-watch/discussions

Текущий список улучшений: [ROADMAP.md](ROADMAP.md).

## Поддерживаемые ОС

- Windows
- macOS
- Linux
- Android/Termux должен работать из исходников при установленном Rust;
  prebuilt Termux-бинарники пока в roadmap.

## Тесты

```bash
cargo test --locked
cargo run --locked -- --demo --json
```

## Статус для конкурса

Уже работает:

- Нативный Rust CLI.
- Polling NeuroGate `/v1/me`.
- Окна credit/request для 5h / 24h / 7d / 30d.
- Human output и JSON output.
- Compact output для виджетов/status bar.
- `.env` рядом с бинарником или рабочей директорией.
- Настраиваемые пороги warning/danger.
- Demo/mock-режим без ключа и сети.
- Опциональная интеграция с локальным `abtop`.
- CI и release workflow для нативных бинарников.

Пока не готово:

- Нативный Android UI.
- Push-уведомления/webhooks.
- Статистика по отдельным API-ключам, если NeuroGate не отдаёт ее через
  публичный `/v1/*` endpoint.
