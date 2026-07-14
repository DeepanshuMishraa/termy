# termy_plugin_runtime

Persistent Bun runtime for trusted local plugins.

## Owner

This crate owns plugin discovery, command and lifecycle-event dispatch, descriptor and action validation, the read-only invocation context contract, managed TypeScript declarations, the persistent Bun host, per-plugin Worker lifecycle, protocol limits, timeouts, and warm reload caching.

It returns typed commands and actions to callers. Command-palette presentation, GPUI state, built-in command execution, terminal tabs, toasts, and settings remain owned by `crates/desktop_app/`.

## Validation

```sh
cargo test -p termy_plugin_runtime
```

## Forbidden Dependencies

- `gpui`
- `termy` / `crates/desktop_app`
- `termy_config_core`
- `termy_command_core`
- `termy_terminal_ui`
