# termy_ssh_core

Saved SSH host and credential domain logic.

## Owner

This crate owns validated saved-host data, atomic non-secret persistence, exact OpenSSH program arguments, keychain account construction, and credential lifecycle orchestration. It is headless and does not own GPUI presentation, terminal tabs, or process spawning.

Passwords and private-key passphrases are never serialized by this crate. Private keys remain user-owned files; only their paths are stored.

## Validation

```sh
cargo test -p termy_ssh_core
```

## Forbidden Dependencies

- `gpui`
- `termy` / `crates/desktop_app`
- `termy_terminal_ui`
- SSH client libraries that bypass the system OpenSSH host-key and `known_hosts` behavior
