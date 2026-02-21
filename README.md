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
```

Full configuration reference:
- `docs/configuration.md`

## License

MIT. See [LICENSE](LICENSE).
