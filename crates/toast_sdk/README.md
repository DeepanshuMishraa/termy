# termy_toast

Small notification primitives.

## Owner

This crate owns lightweight toast/notification data structures intended to be reused without pulling in the full desktop app. Keep rendering and app placement decisions in `crates/desktop_app/`.

Use this crate when notification contracts need to be shared.

## Validation

```sh
cargo test -p termy_toast
```

## Forbidden Dependencies

- `gpui`
- `termy_terminal_ui`
- `termy` / `crates/desktop_app`
