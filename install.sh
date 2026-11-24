#!/usr/bin/env bash
# GullDeb installer — https://github.com/neverjoskiy/gulldeb
set -euo pipefail

REPO="neverjoskiy/gulldeb"
BIN_NAME="gulldeb"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
APP_DIR="$HOME/.local/share/applications"
DESKTOP_FILE="gulldeb.desktop"

# Colors
if [[ -t 1 ]]; then
    BOLD=$'\e[1m'; GREEN=$'\e[32m'; YELLOW=$'\e[33m'; RED=$'\e[31m'; CYAN=$'\e[36m'; RESET=$'\e[0m'
else
    BOLD=""; GREEN=""; YELLOW=""; RED=""; CYAN=""; RESET=""
fi

say()  { printf '%s%s%s\n' "$GREEN" "$*" "$RESET"; }
warn() { printf '%s%s%s\n' "$YELLOW" "$*" "$RESET"; }
err()  { printf '%s%s%s\n' "$RED" "$*" "$RESET" >&2; }
step() { printf '\n%s➜ %s%s\n' "$BOLD$CYAN" "$*" "$RESET"; }

step "GullDeb installer"

# ---- Detect source tree vs. standalone run ----
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HAVE_SOURCE=0
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    HAVE_SOURCE=1
fi

mkdir -p "$INSTALL_DIR" "$APP_DIR"

# ---- Obtain binary ----
if [[ $HAVE_SOURCE -eq 1 ]] && command -v cargo >/dev/null 2>&1; then
    step "Building from source (cargo build --release)"
    (cd "$SCRIPT_DIR" && cargo build --release)
    install -m 0755 "$SCRIPT_DIR/target/release/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
    say "Built and installed $INSTALL_DIR/$BIN_NAME"
else
    step "Downloading latest release binary"
    # Resolve latest tag
    REL_JSON="$(mktemp)"
    curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" -o "$REL_JSON" || true
    LATEST="$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' "$REL_JSON" | head -n1)"
    rm -f "$REL_JSON"
    if [[ -z "$LATEST" ]]; then
        err "Could not determine latest release. Install from source instead:"
        err "  git clone https://github.com/$REPO && cd gulldeb && ./install.sh"
        exit 1
    fi
    say "Latest release: $LATEST"
    TMP="$(mktemp -d)"
    trap 'rm -rf "$TMP"' EXIT
    URL="https://github.com/$REPO/releases/download/$LATEST/$BIN_NAME"
    if ! curl -fL --progress-bar -o "$TMP/$BIN_NAME" "$URL"; then
        err "Download failed: $URL"
        exit 1
    fi
    # Optional checksum verification
    if curl -fsSL "https://github.com/$REPO/releases/download/$LATEST/$BIN_NAME.sha256" -o "$TMP/$BIN_NAME.sha256" 2>/dev/null; then
        (cd "$TMP" && sha256sum -c "$BIN_NAME.sha256" >/dev/null 2>&1) && say "Checksum OK" || warn "Checksum mismatch or unavailable — continuing"
    fi
    install -m 0755 "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
    say "Installed $INSTALL_DIR/$BIN_NAME"
fi

# ---- Desktop entry ----
step "Installing desktop entry"
if [[ -f "$SCRIPT_DIR/$DESKTOP_FILE" ]]; then
    install -m 0644 "$SCRIPT_DIR/$DESKTOP_FILE" "$APP_DIR/$DESKTOP_FILE"
    # Patch Exec to absolute path so Dolphin/Nautilus can find the binary
    # even when ~/.local/bin is not in the session PATH.
    sed -i "s|^Exec=.*|Exec=$INSTALL_DIR/$BIN_NAME %f|" "$APP_DIR/$DESKTOP_FILE"
else
    cat > "$APP_DIR/$DESKTOP_FILE" <<EOF
[Desktop Entry]
Type=Application
Name=GullDeb
Comment=Convert and install Debian .deb packages on Arch Linux
Exec=$INSTALL_DIR/$BIN_NAME %f
Terminal=true
MimeType=application/vnd.debian.binary-package;
Icon=package-x-generic
Categories=System;PackageManager;
StartupNotify=true
EOF
fi
chmod 0644 "$APP_DIR/$DESKTOP_FILE"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APP_DIR" >/dev/null 2>&1 || true
fi

# ---- MIME association ----
step "Registering MIME association (.deb → GullDeb)"
if command -v xdg-mime >/dev/null 2>&1; then
    xdg-mime default "$DESKTOP_FILE" application/vnd.debian.binary-package
    say "Registered: application/vnd.debian.binary-package → $DESKTOP_FILE"
else
    warn "xdg-mime not found; skipping MIME registration"
fi

# ---- PATH setup ----
step "Ensuring $INSTALL_DIR is on PATH"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) say "$INSTALL_DIR already on PATH" ;;
    *)
        warn "$INSTALL_DIR is not on PATH"
        # Detect shell and append
        SHELL_NAME="$(basename "${SHELL:-}")"
        case "$SHELL_NAME" in
            fish)
                FISH_CFG="$HOME/.config/fish/config.fish"
                mkdir -p "$(dirname "$FISH_CFG")"
                if ! grep -q "$INSTALL_DIR" "$FISH_CFG" 2>/dev/null; then
                    printf '\n# GullDeb\nfish_add_path %s\n' "$INSTALL_DIR" >> "$FISH_CFG"
                    say "Added fish_add_path to $FISH_CFG"
                fi
                ;;
            zsh)
                if ! grep -q "$INSTALL_DIR" "$HOME/.zshrc" 2>/dev/null; then
                    printf '\n# GullDeb\nexport PATH="%s:$PATH"\n' "$INSTALL_DIR" >> "$HOME/.zshrc"
                    say "Added export to ~/.zshrc"
                fi
                ;;
            bash|*)
                if ! grep -q "$INSTALL_DIR" "$HOME/.bashrc" 2>/dev/null; then
                    printf '\n# GullDeb\nexport PATH="%s:$PATH"\n' "$INSTALL_DIR" >> "$HOME/.bashrc"
                    say "Added export to ~/.bashrc"
                fi
                ;;
        esac
        say "Restart your shell or run: export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac

# ---- Verify ----
step "Verification"
if command -v "$BIN_NAME" >/dev/null 2>&1; then
    "$BIN_NAME" --version
    say "✅ GullDeb is ready!"
else
    # Not on PATH in this session yet — invoke directly
    "$INSTALL_DIR/$BIN_NAME" --version
    say "✅ Installed to $INSTALL_DIR/$BIN_NAME"
    warn "Open a new terminal (or source your shell rc) to use 'gulldeb' directly."
fi

echo
echo "Usage:"
echo "  gulldeb /path/to/package.deb            # convert + install"
echo "  gulldeb /path/to/package.deb --no-install  # convert only"
echo
echo "Double-click any .deb file in your file manager to launch GullDeb."
