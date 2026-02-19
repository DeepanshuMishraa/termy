# Termy

A fast, minimal terminal emulator built with [GPUI](https://github.com/zed-industries/zed) and [alacritty_terminal](https://github.com/alacritty/alacritty).

<p align="center">
  <img src="assets/termy_icon.png" width="128" alt="Termy Logo">
</p>

## ✨ Features

- 🚀 **GPU-accelerated** rendering via GPUI
- 🎨 **12 beautiful themes** built-in
- ⚡ **Fast & lightweight** — minimal resource usage
- 🔧 **Configurable** via simple config file
- 🍎 **Native macOS app** with DMG packaging

## 🎨 Themes

| Theme | Description |
|-------|-------------|
| `tokyonight` | Deep blue, inspired by Tokyo Night |
| `catppuccin` | Soothing pastel theme |
| `dracula` | Dark theme with vibrant colors |
| `gruvbox` | Retro groove color scheme |
| `nord` | Arctic, north-bluish palette |
| `solarized` | Precision colors for machines and people |
| `onedark` | Atom's iconic One Dark theme |
| `monokai` | Classic vibrant syntax colors |
| `material` | Material Design inspired colors |
| `palenight` | Soft purple-tinted dark theme |
| `tomorrow` | Tomorrow Night color scheme |
| `oceanic` | Deep sea blue-green tones |

## 🚀 Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- macOS (currently the primary target)

### Run

```sh
cargo run --release
```

### Build

```sh
cargo build --release
```

The compiled binary will be at `target/release/termy`.

## 📦 macOS App Bundle

Build a native `.app` bundle with an unsigned DMG for distribution:

### 1. Install cargo-bundle (one-time)

```sh
cargo install cargo-bundle
```

### 2. Build DMG

```sh
./scripts/build-dmg.sh
```

The DMG will be created at `target/release/termy.dmg` with the app and an Applications shortcut ready for drag-and-drop installation.

## ⚙️ Configuration

Create a config file at:

```
~/.config/termy/config.txt
```

### Example

```txt
# Theme selection
# Available: tokyonight, catppuccin, dracula, gruvbox, nord, solarized,
#            onedark, monokai, material, palenight, tomorrow, oceanic
theme = tokyonight

# Optional startup directory (supports "~")
working_dir = ~/Documents

# Startup window size in pixels
window_width = 1100
window_height = 720

# Terminal font family (quotes optional unless you prefer them)
font_family = "JetBrains Mono"

# Terminal font size in pixels
font_size = 14

# Inner terminal padding in pixels
padding_x = 12
padding_y = 8
```

Lines starting with `#` are treated as comments. Alternative names like `tomorrow` for `tomorrownight` also work.

## 🛠️ Development

```sh
# Run with logging
RUST_LOG=info cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Run clippy
cargo clippy
```

## 📁 Project Structure

```
src/
├── main.rs           # Application entry point
├── terminal.rs       # PTY and terminal emulation
├── terminal_view.rs  # GPUI rendering
├── colors.rs         # Color utilities
├── config.rs         # Configuration management
└── themes/           # Theme definitions
```

## 🤝 Contributing

Contributions are welcome! Feel free to:

- Report bugs
- Suggest features
- Submit pull requests

## 📄 License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

<p align="center">
  Built with 🔥 using Rust, GPUI, and alacritty_terminal
</p>
