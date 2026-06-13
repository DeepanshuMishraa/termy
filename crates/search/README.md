# termy_search

Reusable terminal search primitives.

## Owner

This crate owns text matching and search state that can be shared by the desktop app, headless runtime, and tests. Keep GPUI rendering, selection visuals, and command-palette behavior outside this crate.

Use this crate when changing search matching semantics or reusable search state.

## Validation

```sh
cargo test -p termy_search
```

## Forbidden Dependencies

- `gpui`
- `termy_config_core`
- `termy_terminal_ui`
- `termy` / `crates/desktop_app`
