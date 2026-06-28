#!/bin/bash
# setup-issues.sh
# Создаёт стартовый набор Issues для vimit с правильными labels
# Запускать один раз: bash setup-issues.sh

set -e

REPO="xodapi/vimit"

echo "=== Создание labels ==="

labels=(
  "agent|0075ca|Готово к выполнению агентом"
  "blocked|e11d48|Заблокировано"
  "gui|7c3aed|Изменения в Slint GUI"
  "tui|0891b2|Изменения в ratatui TUI"
  "priority:high|dc2626|Высокий приоритет"
  "priority:low|6b7280|Низкий приоритет"
)

for label_def in "${labels[@]}"; do
  IFS='|' read -r name color desc <<< "$label_def"
  gh label create "$name" --color "$color" --description "$desc" --repo "$REPO" 2>/dev/null || \
  gh label edit "$name" --color "$color" --description "$desc" --repo "$REPO" 2>/dev/null || true
done

echo "=== Labels созданы ==="

echo "=== Создание Issues ==="

# Issue 1: CI fix (самый срочный)
gh issue create \
  --repo "$REPO" \
  --title "fix(ci): install gtk deps for linux, split gui feature matrix" \
  --label "ci,agent,priority:high" \
  --body "## Что сделать

В \`.github/workflows/ci.yml\` исправить провал Linux-сборки с \`--features gui\`.

## Конкретные изменения

1. В job \`check\` для \`ubuntu-latest\` добавить шаг перед \`cargo check\`:
\`\`\`yaml
- name: Install Linux system deps
  if: matrix.os == 'ubuntu-latest'
  run: sudo apt-get update && sudo apt-get install -y libglib2.0-dev libgtk-3-dev
\`\`\`

2. Либо (проще): для \`ubuntu-latest\` убрать \`--features gui\` из clippy и test — GUI-фичу проверять только на Windows/macOS.

3. В README.md и README.ru.md заменить бадж \`rust-2021\` на \`rust-2024\`.

## Критерии приёмки

- CI #48+ проходит зелёным на всех трёх ОС
- \`cargo test --locked\` локально чистый
- Бадж в README обновлён

## Что нельзя трогать

- \`src/\` — никаких изменений кода
- \`SUBMISSION.ru.md\`"

# Issue 2: Theme system в Slint
gh issue create \
  --repo "$REPO" \
  --title "feat(gui): add theme.slint with dark/light mode support" \
  --label "feat,gui,agent" \
  --body "## Что сделать

Создать \`ui/theme.slint\` с глобальной цветовой системой и подключить в \`ui/app.slint\`.

## Конкретные изменения

1. Создать \`ui/theme.slint\`:
\`\`\`slint
export global Theme {
    in-out property <bool> dark-mode: true;
    property <color> bg-primary:   dark-mode ? #0f131a : #f8fafc;
    property <color> bg-card:      dark-mode ? #1a202c : #ffffff;
    property <color> bg-surface:   dark-mode ? #111827 : #f1f5f9;
    property <color> border:       dark-mode ? #334155 : #e2e8f0;
    property <color> text-primary: dark-mode ? #f1f5f9 : #0f172a;
    property <color> text-muted:   dark-mode ? #64748b : #94a3b8;
    property <color> success: #22c55e;
    property <color> warning: #f59e0b;
    property <color> danger:  #ef4444;
}
\`\`\`

2. В \`LimitCard\` добавить:
\`\`\`slint
private property <color> accent:
    root.percent >= 90 ? Theme.danger :
    root.percent >= 75 ? Theme.warning : Theme.success;
\`\`\`

3. Большой процент: \`font-size: 44px; font-weight: 800; color: root.accent\`

4. В панели настроек добавить CheckBox «Светлая тема» → переключает \`Theme.dark-mode\`

5. Переименовать «пик» → «макс» везде в UI

## Критерии приёмки

- \`cargo build --features gui --locked\` без ошибок
- GUI запускается в demo-режиме, переключение темы работает
- Большой % виден сразу при открытии окна

## Что нельзя трогать

- \`src/\` Rust-код — только \`ui/\` файлы
- Логика polling и API"

# Issue 3: Дневной лимит
gh issue create \
  --repo "$REPO" \
  --title "feat(core): add daily credit limit tracking" \
  --label "feat,agent,tui,gui" \
  --body "## Что сделать

Добавить отслеживание дневного лимита расхода кредитов.

## Логика

Расчётный рекомендуемый лимит/день = \`остаток_7d / дней_до_сброса_7d\`

Если пользователь задал свой лимит через \`--daily-limit N\` — использовать его.

Хранить в \`~/.config/vimit/daily.toml\`:
\`\`\`toml
limit = 56.0      # кредиты на день (0 = не задан, считать авто)
date = \"2026-06-28\"  # при смене даты сбрасывать
spent_today = 12.3
\`\`\`

## Что показывать

В TUI (compact и full preset) — строка под окном 7d:
\`\`\`
День: 12.3 / 56.0 (22%)  [████░░░░░░]
\`\`\`

В JSON-выводе — поле \`daily\` рядом с \`windows\`.

## CLI

\`\`\`
vimit --daily-limit 56   # задать лимит
vimit --daily-limit 0    # сбросить (использовать авто)
\`\`\`

## Критерии приёмки

- \`cargo test --locked\` — все тесты зелёные
- \`vimit --demo --daily-limit 50\` показывает строку дневного лимита
- \`vimit --demo --json\` содержит поле \`daily\`
- При смене даты spent_today сбрасывается в 0

## Что нельзя трогать

- \`ui/app.slint\` (GUI версия — отдельный Issue)
- \`SUBMISSION.ru.md\`"

# Issue 4: Скриншоты в README
gh issue create \
  --repo "$REPO" \
  --title "docs(readme): add demo screenshots to README" \
  --label "docs,agent,priority:high" \
  --body "## Что сделать

Добавить скриншоты TUI-монитора и GUI в README.md и README.ru.md.

## Шаги

1. Запустить \`.\target\release\vimit.exe --demo --monitor\`, сделать скриншот терминала → сохранить в \`assets/demo-tui.png\`

2. Запустить \`.\target\release\vimit-gui.exe --demo\` (или \`--features gui\`), сделать скриншот → сохранить в \`assets/demo-gui.png\`

3. В README.md после раздела \`## Features\` добавить:
\`\`\`markdown
## Screenshots

![TUI Monitor](assets/demo-tui.png)
![GUI](assets/demo-gui.png)
\`\`\`

4. То же самое в README.ru.md

## Критерии приёмки

- Оба файла \`assets/demo-tui.png\` и \`assets/demo-gui.png\` существуют в репо
- В обоих README изображения отображаются на странице GitHub
- Размер каждого PNG < 500KB

## Что нельзя трогать

- \`src/\`, \`ui/\`, \`Cargo.toml\`"

# Issue 5: SUBMISSION update
gh issue create \
  --repo "$REPO" \
  --title "docs: update SUBMISSION.ru.md for v0.6.2 final" \
  --label "docs,agent,priority:high" \
  --body "## Что сделать

Обновить \`SUBMISSION.ru.md\` для финальной подачи на конкурс (дедлайн 1 июля).

## Конкретные изменения

1. В разделе «Что уже работает» добавить:
   - Система self-update (\`vimit update\` / \`vimit update --check\`)
   - System tray tooltip с процентами окон
   - Панель настроек с авто-проверкой обновлений
   - Slint GUI с поддержкой dark/light темы (если Issue #N уже закрыт)

2. Из раздела «Что ещё не готово» убрать пункты которые уже реализованы.

3. Обновить версию в заголовке на v0.6.2.

4. Формат подачи — заполнить все поля из объявления конкурса:
\`\`\`
Название: vimit
Что делает: ...
Для кого полезно: ...
GitHub: https://github.com/xodapi/vimit
Как запустить: ...
Какие ОС поддерживаются: Windows, Linux, macOS, Android/Termux
Что уже работает: ...
Что ещё не готово: ...
\`\`\`

## Критерии приёмки

- Версия в заголовке = v0.6.2
- Все поля конкурсной формы заполнены
- Ни один реализованный пункт не стоит в «не готово»

## Что нельзя трогать

- \`src/\`, \`ui/\`, \`Cargo.toml\`, CI-файлы"

echo ""
echo "=== Готово! ==="
echo "Открытые Issues:"
gh issue list --repo "$REPO" --state open --label agent
