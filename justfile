set shell := ["bash", "-cu"]

# Show available recipes
@default:
  just --list

# Generate macOS .icns file from assets/termy_icon@1024px.png
generate-icon:
  ./scripts/generate-icon.sh

# Build macOS app bundle and DMG
# Example:
#   just build-dmg -- --version 0.1.0 --arch arm64 --target aarch64-apple-darwin
build-dmg *args:
  ./scripts/build-dmg.sh {{args}}
