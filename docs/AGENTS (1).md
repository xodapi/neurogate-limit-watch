# AGENTS.md — Глобальные правила для AI-агентов

> Этот файл читают все агенты: Codex, Gemini, Grok, Claude Code, Factory Droid.
> Он описывает **единственный** разрешённый рабочий процесс.
> Нарушение любого пункта = задача не принята.

---

## 0. Главное правило

**Каждое изменение в коде начинается с GitHub Issue и заканчивается закрытием этого Issue.**

Агент не пишет код "в воздух". Нет Issue — нет работы.

---

## 1. Перед началом любой задачи

### 1.1 Найти Issue

```
gh issue list --state open --label agent
```

Работать только с Issue у которых:
- label `agent` — готово к выполнению агентом
- label `blocked` отсутствует
- assignee пустой или назначен на тебя

### 1.2 Назначить себя

```
gh issue edit <NUMBER> --add-assignee "@me"
gh issue comment <NUMBER> --body "🤖 Начинаю выполнение. Агент: <AGENT_NAME>"
```

### 1.3 Прочитать Issue полностью

Issue содержит:
- **Что сделать** — конкретная задача
- **Критерии приёмки** — как проверить что готово
- **Что нельзя трогать** — файлы и модули вне задачи

Если чего-то не хватает для выполнения — оставить комментарий с вопросом,
снять assignee, **не начинать код**.

---

## 2. Рабочий процесс (обязательный порядок)

```
Issue открыт
    ↓
git checkout -b issue-<NUMBER>-<slug>
    ↓
Реализация (только в рамках Issue)
    ↓
cargo test --locked  # все тесты зелёные
cargo clippy --all-targets -- -D warnings  # ноль предупреждений
cargo fmt --check  # форматирование чистое
    ↓
git commit -m "fix/feat/docs(<scope>): <описание> (closes #<NUMBER>)"
    ↓
git push origin issue-<NUMBER>-<slug>
    ↓
gh pr create --title "..." --body "Closes #<NUMBER>" --base main
    ↓
gh issue comment <NUMBER> --body "✅ PR готов: <PR_URL>"
```

### Формат ветки

```
issue-42-add-daily-limit
issue-57-fix-ci-linux-gtk
issue-103-theme-dark-light
```

### Формат коммита

```
feat(monitor): add daily limit display (closes #42)
fix(ci): install gtk deps for linux gui build (closes #57)
docs(readme): add demo screenshots (closes #103)
```

Типы: `feat` `fix` `docs` `refactor` `test` `ci` `chore`

---

## 3. Ограничения при реализации

### 3.1 Трогать только то что в Issue

Если Issue про `src/notify.rs` — не рефакторить `src/monitor.rs` попутно.
Попутные улучшения = отдельный Issue.

### 3.2 Запрещено без явного указания в Issue

- Менять `Cargo.toml` (добавлять/удалять зависимости)
- Менять `ci.yml` или `release.yml`
- Переименовывать публичные функции и структуры
- Менять формат JSON-вывода (`--json`)
- Трогать `SUBMISSION.ru.md`
- Коммитить `.env` файлы с ключами

### 3.3 API-ключи

Никогда не логировать `VIBEMODE_API_KEY`, `NEUROGATE_API_KEY`.
Если ключ попал в вывод — задача не завершена, нужен отдельный Issue на исправление.

---

## 4. Проверка перед PR

Все три команды должны завершиться с кодом 0:

```bash
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Дополнительно для GUI-изменений:

```bash
cargo build --features gui --locked
```

Если тесты красные — **не создавать PR**. Исправить и перепроверить.

---

## 5. Оформление PR

```markdown
## Что сделано
<одна-две строки что изменилось>

## Как проверить
<конкретные команды для ручной проверки>

## Closes
Closes #<NUMBER>
```

Не писать: "я сделал X, Y, Z, W" — только то что в Issue.

---

## 6. После merge

```bash
gh issue close <NUMBER> --comment "✅ Выполнено в PR #<PR_NUMBER>"
git checkout main
git pull origin main
git branch -d issue-<NUMBER>-<slug>
```

---

## 7. Если что-то пошло не так

### Тесты не проходят после реализации

```bash
gh issue comment <NUMBER> --body "⚠️ Тесты не проходят: <вывод ошибки>. Нужна помощь."
```
Снять assignee. Не пушить сломанный код.

### Issue оказался больше чем ожидалось

```bash
gh issue comment <NUMBER> --body "⚠️ Задача требует изменений в <X> — это за рамками Issue. Создаю дочерний Issue."
gh issue create --title "..." --body "Дочерняя задача от #<NUMBER>" --label agent
```

### Конфликт с main

```bash
git fetch origin main
git rebase origin/main
# решить конфликты
git push --force-with-lease origin issue-<NUMBER>-<slug>
```

---

## 8. Labels (система меток)

| Label | Значение |
|-------|----------|
| `agent` | Готово к выполнению агентом |
| `blocked` | Заблокировано — не брать |
| `needs-review` | Ждёт ревью человека |
| `bug` | Баг |
| `feat` | Новая фича |
| `ci` | CI/CD |
| `docs` | Документация |
| `gui` | Изменения в Slint GUI |
| `tui` | Изменения в ratatui TUI |
| `priority:high` | Сделать в первую очередь |
| `priority:low` | Можно отложить |

Агент берёт задачи в порядке: `priority:high` → без приоритета → `priority:low`.

---

## 9. Что агент НЕ делает самостоятельно

- Не создаёт Issue сам (только человек)
- Не закрывает чужие Issue
- Не делает force push в `main`
- Не удаляет файлы без явного указания в Issue
- Не меняет версию в `Cargo.toml` (это делает человек перед релизом)
- Не создаёт теги и релизы

---

## 10. Контекст проекта

**Репозиторий:** https://github.com/xodapi/vimit  
**Язык:** Rust 2024 edition  
**Тесты:** `cargo test --locked` (58 тестов)  
**GUI:** Slint (`--features gui`)  
**TUI:** ratatui  
**API:** `GET /v1/me` → `VIBEMODE_API_KEY`  
**Платформы:** Windows x86_64, Linux x86_64/aarch64, macOS aarch64  

Структура `src/`:
```
src/
  main.rs          — точка входа, CLI парсинг
  api.rs           — запрос /v1/me, модели данных
  monitor.rs       — TUI ratatui монитор
  notify.rs        — desktop notifications
  cli/
    update.rs      — self-update логика
  bin/
    vimit-gui.rs   — Slint GUI
ui/
  app.slint        — главный Slint файл
```

---

## 11. Быстрый старт для нового агента

```bash
# 1. Посмотреть открытые задачи
gh issue list --state open --label agent --assignee ""

# 2. Взять задачу
gh issue edit <N> --add-assignee "@me"
gh issue comment <N> --body "🤖 Начинаю"

# 3. Создать ветку
git checkout -b issue-<N>-<slug>

# 4. Сделать работу, проверить
cargo test --locked && cargo clippy --all-targets -- -D warnings && cargo fmt --check

# 5. Коммит и PR
git commit -m "feat(...): ... (closes #<N>)"
git push origin issue-<N>-<slug>
gh pr create --fill --base main
```

---

## 12. Параллельная работа нескольких агентов

При работе 2+ агентов одновременно — читать **MULTI-AGENT.md**.

Краткие правила:
- Каждый Issue имеет label `zone:X` — два агента не берут одну зону
- Проверять зависимости (`Зависит от #N`) перед стартом
- Файлы `Cargo.toml`, `ui/app.slint`, `src/main.rs` — только один агент
- При rebase-конфликте в `Cargo.lock` — пересоздать через `cargo generate-lockfile`
