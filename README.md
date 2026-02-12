# Termy

Minimal GPUI-powered terminal built on gpui and alacritty_terminal 🔥

## Run

```sh
cargo run --release
```

## Build

```sh
cargo build --release
```

## Build unsigned DMG (macOS)

1) Install bundler once:

```sh
cargo install cargo-bundle
```

2) Build `.app` + unsigned `.dmg`:

```sh
./scripts/build-dmg.sh
```

The DMG opens with app + `Applications` shortcut laid out for drag-and-drop install.

Output DMG:

```txt
target/release/termy.dmg
```

## Config

Config file: `~/.config/termy/config.txt`

```txt
# Will be comments using #
theme = tokyonight
```

Themes: `tokyonight`, `catppuccin`, `dracula`, `gruvbox`, `nord`, `solarized`.
