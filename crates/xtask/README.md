# xtask

Repository automation binary.

## Owner

This crate owns maintainer commands that generate or verify repository artifacts, such as generated keybinding and configuration documentation.

Keep product runtime code out of this crate. If an automation command needs shared domain data, depend on the smallest domain crate that owns that data.

## Validation

```sh
cargo test -p xtask
cargo run -p xtask -- generate-keybindings-doc --check
cargo run -p xtask -- generate-config-doc --check
```

## Forbidden Dependencies

- `termy_ffi`
- `termy` / `crates/desktop_app`
- product runtime workflows

Native macOS app automation:

```sh
cargo macos run
cargo macos verify
cargo macos logs
```

`cargo macos` builds `termy_ffi`, builds the SwiftPM host, stages `macos/dist/Termy.app`, and then runs the requested app command.
