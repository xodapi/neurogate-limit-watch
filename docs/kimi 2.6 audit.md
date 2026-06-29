Критический анализ кодовой базы vimit
Ниже разбор по категориям: от критических ошибок компиляции до архитектурных проблем.

1. Критические ошибки (проект не собирается)
src/cli/monitor.rs:497 — mismatched types

rust

let mut final_style = pal.bold_level_style(level);
// ...
final_style = pal.danger;  // Ошибка: ожидается Style, найден Color
pal.danger имеет тип Color, а final_style — Style. Исправление: pal.danger.into().

src/cli/monitor.rs:1489 — missing field offline_duration_min

rust

StatusSnapshot { stale: false, windows: ..., daily: None, ... }
В тестовой фабрике test_snapshot() не заполнено поле offline_duration_min, добавленное позже в структуру, но не в тесты.

Результат: cargo test --locked и cargo clippy падают с ошибкой компиляции.

2. Логические ошибки / баги
2.1 Recovery-уведомления никогда не отправляются
Файл: src/cli/notify.rs, метод check_windows

rust

if let Some(msg) = next_notification(&mut self.last_levels, window) { ... }
if let Some(msg) = recovery_notification(&mut self.last_levels, window) { ... }
next_notification сначала записывает текущий уровень в last_levels. После этого recovery_notification читает уже обновленное значение, поэтому previous == current == Ok, и условие previous == AlertLevel::Ok всегда срабатывает, возвращая None. Recovery-нотификации мертвы.

2.2 Overlay-режим не работает на Linux / macOS
Файл: src/main.rs

rust

let gui_path = current.with_file_name("vimit-gui.exe");
На Unix-системах бинарь называется vimit-gui без .exe. Overlay просто не запустится.

2.3 Дублирование и рассинхрон логики daily-окна
Файл: src/cli/monitor.rs, collect_status

При успешном fetch:

вызывается daily_file.update(c.remaining) + daily_file.save()
затем вставляется "today" window
При fallback на кэш (ветка Err(error)):

daily_file.update() не вызывается
"today" window вставляется через get_state, но расход за день не обновляется
Это приводит к тому, что в offline-режиме daily-метрика показывает устаревшие или некорректные данные.

2.4 Race condition при записи daily.toml
Файл: src/cli/daily.rs

DailyFile::save() делает простое fs::write. Если vimit запущен одновременно из нескольких терминалов или в monitor + вручную, файл может повредиться.

3. Архитектурные проблемы
3.1 God-object lib.rs
Файл src/lib.rs (~800+ строк кода) смешивает:

HTTP-клиент и retry-логику
Парсинг 5 разных схем JSON
Форматирование строк и цветов
Работу с .env
Выполнение внешних процессов (abtop)
Глобальный mutable state OFFLINE_SINCE
Это нарушает SRP и затрудняет юнит-тестирование.

3.2 God-object monitor.rs
src/cli/monitor.rs (~1500 строк) объединяет:

Event loop TUI
Рендеринг 3 пресетов
Сбор данных (collect_status)
Хелперы для plain-text тестов
Управление панелями
3.3 Глобальный mutable state
rust

pub static OFFLINE_SINCE: Mutex<Option<Instant>> = Mutex::new(None);
Делает параллельные тесты невозможными без cargo test -- --test-threads=1
При мониторинге нескольких аккаунтов один неудачный запрос помечает всё приложение "offline"
3.4 Массовое дублирование в vimit-gui.rs
GUI-бинарь полностью дублирует:

Логику загрузки accounts.toml (load_gui_accounts вместо AccountsConfig::load)
Поиск .env / dirs_or_default
Конфигурацию HTTP и Router
Обработку demo/live режимов
Любое изменение в CLI-логике требует ручной синхронизации с GUI.

4. Проблемы безопасности
4.1 PowerShell injection в уведомлениях
Файл: src/cli/notify.rs

rust

fn powershell_quote(text: &str) -> String {
    text.replace('\'', "''").replace(['\r', '\n'], " ")
}
Экранируется только одиночная кавычка. Спецсимволы PowerShell ($, `, ;, |, &) не обрабатываются. Хотя входные данные приходят из API, вредоносный ответ или скомпрометированный API может выполнить код на машине пользователя.

4.2 API-ключ в открытом виде в кэше
Файл: src/cli/cache.rs

rust

fn cache_key(api_key: &str, api_base: &str) -> String {
    format!("{}|{}", api_key, api_base)
}
Ключ API хранится в открытом виде как ключ записи в redb. Файл cache.redb лежит в APPDATA / ~/.config и может быть доступен другим пользователям системы.

5. Проблемы производительности
5.1 Спам потоками в GUI
Файл: src/bin/vimit-gui.rs, start_refresh

rust

fn start_refresh(...) {
    thread::spawn(move || { ... });
}
Каждый клик "Refresh" или изменение настроек порождает новый поток без throttle / debounce / семафора. При частых кликах возможен спам запросами.

5.2 Полное сканирование trend-базы
Файл: src/cli/trends.rs, query_trends

rust

let range = table.range(start_ts..=end_ts)?;
for result in range { ... serde_json::from_slice(...) ... }
Все записи за N дней десериализуются целиком. При частых снапшотах (каждые 5 сек в monitor) объем данных растет быстро, а тренды запрашиваются редко.

6. Мелкие, но важные замечания
Проблема	Где	Почему важно
cargo fmt --check может не пройти	lib.rs, monitor.rs	Много ручного форматирования, нестандартные отступы
Смешение языков	vimit-gui.rs (русский), lib.rs (русский), output.rs (английский)	Неконсистентный UX
#[allow(clippy::collapsible_if)] в lib.rs и main.rs	глобально	Подавляет полезные линты без причины
bin/test.rs	src/bin/test.rs	Бесполезный бинарь, дублирует demo_payload
windows_console_process_count	main.rs	unsafe FFI без обработки ошибок (GetConsoleProcessList может вернуть 0)
Итоговая сводка
Хочешь, чтобы я исправил критические ошибки компиляции и основные логические баги?


Thought for 3s
Анализ завершен. Если нужно исправить найденные ошибки, дай знать.