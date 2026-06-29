## Что сделать

`cache_key()` в `src/cli/cache.rs` использует `api_key` в открытом виде как часть ключа записи в `redb`:

```rust
fn cache_key(api_key: &str, api_base: &str) -> String {
    format!("{}|{}", api_key, api_base.trim_end_matches('/'))
}
```

Это означает, что `VIBEMODE_API_KEY` хранится в открытом виде внутри `cache.redb` (лежит в `APPDATA` / `~/.config`). Любой пользователь системы с доступом к этому файлу может извлечь ключ.

## Конкретные изменения

1. Заменить `cache_key` на хеш от `api_key` + `api_base`:
   - Использовать `std::collections::hash_map::DefaultHasher` для получения stable хеша
   - Формат ключа: `<hash>|api_base`
2. Убедиться, что кэш по-прежнему корректно работает (читается и записывается)

## Критерии приёмки

- `cargo test --locked` проходит
- `cargo clippy --all-targets -- -D warnings` чистый
- Кэш продолжает работать корректно после изменения формата ключа

## Что нельзя трогать

- `Cargo.toml` (новые зависимости не нужны)
- Формат JSON-вывода
- Любые другие модули кроме `src/cli/cache.rs`
