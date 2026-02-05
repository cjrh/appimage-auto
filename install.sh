#!/usr/bin/env bash
set -euo pipefail

# --- Output helpers ---
red='\033[0;31m'    green='\033[0;32m'
yellow='\033[0;33m' blue='\033[0;34m'
bold='\033[1m'      reset='\033[0m'

info()    { printf "${blue}::${reset} %s\n" "$*"; }
success() { printf "${green}::${reset} %s\n" "$*"; }
warn()    { printf "${yellow}:: WARNING:${reset} %s\n" "$*"; }
error()   { printf "${red}:: ERROR:${reset} %s\n" "$*" >&2; }

# --- Guards ---
if [[ $EUID -eq 0 ]]; then
    error "Do not run this script as root. It installs to user directories."
    exit 1
fi

# --- Resolve archive root ---
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# --- Preflight checks ---
missing=0
for f in bin/appimage-auto systemd/appimage-auto.service config/default.toml assets/icon.png; do
    if [[ ! -f "$SCRIPT_DIR/$f" ]]; then
        error "Missing required file: $f"
        missing=1
    fi
done
[[ $missing -eq 1 ]] && exit 1

HAS_GUI=0
if [[ -f "$SCRIPT_DIR/bin/appimage-auto-gui" ]]; then
    HAS_GUI=1
fi

# --- Stop running daemon (upgrade-safe) ---
info "Stopping running daemon (if any)..."
systemctl --user stop appimage-auto.service 2>/dev/null || true
pkill -x appimage-auto 2>/dev/null || true
sleep 0.5

# --- Install binaries ---
info "Installing binaries..."
install -Dm755 "$SCRIPT_DIR/bin/appimage-auto" "$HOME/.local/bin/appimage-auto"

if [[ $HAS_GUI -eq 1 ]]; then
    install -Dm755 "$SCRIPT_DIR/bin/appimage-auto-gui" "$HOME/.local/bin/appimage-auto-gui"
fi

# --- Install systemd service (fix ExecStart path) ---
info "Installing systemd service..."
mkdir -p "$HOME/.local/share/systemd/user"
sed 's|%h/\.cargo/bin/|%h/.local/bin/|g' \
    "$SCRIPT_DIR/systemd/appimage-auto.service" \
    > "$HOME/.local/share/systemd/user/appimage-auto.service"

# --- Install desktop entry (only if GUI present) ---
if [[ $HAS_GUI -eq 1 ]]; then
    info "Installing desktop entry..."
    mkdir -p "$HOME/.local/share/applications"
    sed "s|^Exec=appimage-auto-gui|Exec=$HOME/.local/bin/appimage-auto-gui|" \
        "$SCRIPT_DIR/desktop/appimage-auto-gui.desktop" \
        > "$HOME/.local/share/applications/appimage-auto-gui.desktop"
fi

# --- Install icon ---
info "Installing icon..."
install -Dm644 "$SCRIPT_DIR/assets/icon.png" \
    "$HOME/.local/share/icons/hicolor/256x256/apps/appimage-auto.png"

# --- Install default config (preserve existing) ---
if [[ ! -f "$HOME/.config/appimage-auto/config.toml" ]]; then
    info "Installing default config..."
    install -Dm644 "$SCRIPT_DIR/config/default.toml" \
        "$HOME/.config/appimage-auto/config.toml"
else
    info "Existing config preserved: ~/.config/appimage-auto/config.toml"
fi

# --- Enable and start systemd service ---
info "Enabling systemd service..."
systemctl --user daemon-reload
systemctl --user enable appimage-auto.service 2>/dev/null || warn "Failed to enable service"
systemctl --user start appimage-auto.service 2>/dev/null || warn "Failed to start service"

# --- Update caches ---
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database &>/dev/null; then
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi

# --- Summary ---
echo ""
success "appimage-auto installed successfully!"
echo ""
printf "  %bInstalled files:%b\n" "$bold" "$reset"
printf "    %s\n" "$HOME/.local/bin/appimage-auto"
[[ $HAS_GUI -eq 1 ]] && printf "    %s\n" "$HOME/.local/bin/appimage-auto-gui"
printf "    %s\n" "$HOME/.local/share/systemd/user/appimage-auto.service"
[[ $HAS_GUI -eq 1 ]] && printf "    %s\n" "$HOME/.local/share/applications/appimage-auto-gui.desktop"
printf "    %s\n" "$HOME/.local/share/icons/hicolor/256x256/apps/appimage-auto.png"
printf "    %s\n" "$HOME/.config/appimage-auto/config.toml"
echo ""
printf "  %bManage the daemon:%b\n" "$bold" "$reset"
printf "    systemctl --user status  appimage-auto\n"
printf "    systemctl --user restart appimage-auto\n"
printf "    journalctl --user -u appimage-auto -f\n"
echo ""

# --- PATH warning ---
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
    warn "\$HOME/.local/bin is not in your \$PATH"
    echo "  Add it to your shell profile:"
    echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
