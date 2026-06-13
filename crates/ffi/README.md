# termy_ffi

C-compatible libtermy surface.

## Owner

This crate is a thin FFI wrapper over `termy_core`. Keep exported structs and functions synchronized with `crates/ffi/include/termy.h`, and rebuild this crate before validating native host examples after ABI changes.

## Validation

```sh
cargo test -p termy_ffi
```

## Forbidden Dependencies

- `gpui`
- `termy_terminal_ui`
- `termy` / `crates/desktop_app`
