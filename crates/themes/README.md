# termy_themes

Bundled Termy themes.

## Owner

This crate owns built-in theme definitions and their registration. It should depend on `termy_theme_core` for the data model and stay independent of GPUI and app config I/O.

Use this crate when adding or changing bundled color themes.

## Validation

```sh
cargo test -p termy_themes
```

## Forbidden Dependencies

- `gpui`
- `termy_config_core`
- `termy_terminal_ui`
- `termy` / `crates/desktop_app`
