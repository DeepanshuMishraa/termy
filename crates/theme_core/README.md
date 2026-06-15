# termy_theme_core

Shared theme data model.

## Owner

This crate owns theme structs, serialization-compatible color types, and data contracts used by config, bundled themes, docs, and embedders.

Keep bundled theme values in `termy_themes` and app-specific theme loading/caching in `crates/desktop_app/`.

## Validation

```sh
cargo test -p termy_theme_core
```

## Forbidden Dependencies

- `gpui`
- `termy_themes`
- `termy_config_core`
- `termy` / `crates/desktop_app`
