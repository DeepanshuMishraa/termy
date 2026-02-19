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

## Config

Yes config file is inspired by ghostty

Config file:
- `~/.config/termy/config.txt`

Example:

```txt
theme = termy
working_dir = ~/Documents
use_tabs = true
window_width = 1100
window_height = 720
font_family = "JetBrains Mono"
font_size = 14
padding_x = 12
padding_y = 8
```

Themes:
- `termy`, `tokyonight`, `catppuccin`, `dracula`, `gruvbox`, `nord`, `solarized`, `onedark`, `monokai`, `material`, `palenight`, `tomorrow`, `oceanic`

## License

MIT. See [LICENSE](LICENSE).
