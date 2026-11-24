#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "GullDeb: building release binary..."
cargo build --release

BIN="$HOME/.local/bin/gulldeb"
mkdir -p "$HOME/.local/bin"
install -m 0755 target/release/gulldeb "$BIN"

APPDIR="$HOME/.local/share/applications"
mkdir -p "$APPDIR"
install -m 0644 gulldeb.desktop "$APPDIR/gulldeb.desktop"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPDIR" || true
fi

if command -v xdg-mime >/dev/null 2>&1; then
    xdg-mime default gulldeb.desktop application/vnd.debian.binary-package
fi

echo "GullDeb installed to $BIN"
echo "You can now double-click a .deb file to convert and install it."
