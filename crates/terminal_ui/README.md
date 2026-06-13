# termy_terminal_ui

GPUI-facing terminal runtime support for the desktop app.

## Owner

This crate owns the terminal grid paint cache, native pane runtime, tmux runtime/client support, and GPUI-adjacent terminal adapter behavior used by `crates/desktop_app/src/terminal_view/`. It can depend on GPUI, but shared headless behavior should live in `termy_core` or a pure domain crate instead.

## Validation

```sh
cargo test -p termy_terminal_ui
```

## Forbidden Dependencies

- `termy_ffi`
- `termy` / `crates/desktop_app`
- app settings, workspace stores, or command execution workflows
