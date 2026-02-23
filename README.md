# Termy

Termy is a minimal terminal emulator built with [GPUI](https://gpui.rs) and [alacritty_terminal](https://alacritty.org).

## Quick Start

Requirements:
- Rust (stable)
- macOS

Run:

```sh
cargo run --release
```

Build binary:

```sh
cargo build --release
```

## Build DMG

Install once:

```sh
cargo install cargo-bundle
```

Create app + DMG:

```sh
./scripts/build-dmg.sh
```

Output:
- `target/release/Termy-<version>-macos-<arch>.dmg`

## Build Signed + Notarized DMG (macOS)

Before first use:

1. Set a real bundle identifier in `Cargo.toml` under `[package.metadata.bundle]` (`identifier = "..."`).
2. Install `cargo-bundle` once:

```sh
cargo install cargo-bundle
```

3. Configure notarization credentials using either method:

Profile method (keychain profile):

```sh
xcrun notarytool store-credentials TERMY_NOTARY \
  --apple-id "<apple-id>" \
  --team-id "<team-id>" \
  --password "<app-specific-password>"
```

API key method (`.p8`):
- Get your App Store Connect key file (usually `.p8`), key ID, and issuer ID.
- Pass them directly to the build script (example below).

Create signed + notarized DMG:

```sh
./scripts/build-dmg-signed.sh \
  --sign-identity "Developer ID Application: Your Name (TEAMID)" \
  --notary-profile TERMY_NOTARY
```

Or with API key credentials:

```sh
./scripts/build-dmg-signed.sh \
  --sign-identity "Developer ID Application: Your Name (TEAMID)" \
  --notary-key "/path/to/AuthKey_ABC123XYZ.p8" \
  --notary-key-id "ABC123XYZ" \
  --notary-issuer "00000000-0000-0000-0000-000000000000"
```

Output:
- `target/release/Termy-<version>-macos-<arch>-signed.dmg`

## Build Setup.exe (Windows)

Install Inno Setup once:

```powershell
winget install JRSoftware.InnoSetup
```

Create Windows installer:

```powershell
./scripts/build-setup.ps1 -Version 0.1.0 -Arch x64 -Target x86_64-pc-windows-msvc
```

Output:
- `target/dist/Termy-<version>-<arch>-Setup.exe`

## Config

The config file is inspired by Ghostty.

Config file:
- `~/.config/termy/config.txt`

Example:

```txt
theme = termy
term = xterm-256color
working_dir = ~/Documents
use_tabs = true
window_width = 1100
window_height = 720
font_family = "JetBrains Mono"
font_size = 14
padding_x = 12
padding_y = 8
keybind = cmd-p=toggle_command_palette
```

Full configuration reference:
- `docs/configuration.md`
- `docs/keybindings.md`

## License

MIT. See [LICENSE](LICENSE).
