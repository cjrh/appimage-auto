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
    error "Do not run this script as root. It removes files from user directories."
    exit 1
fi

# --- Stop and disable systemd service ---
info "Stopping and disabling systemd service..."
systemctl --user stop appimage-auto.service 2>/dev/null || true
systemctl --user disable appimage-auto.service 2>/dev/null || true
pkill -x appimage-auto 2>/dev/null || true
pkill -x appimage-auto-gui 2>/dev/null || true
sleep 0.5

# --- Remove installed files ---
info "Removing installed files..."

files=(
    "$HOME/.local/bin/appimage-auto"
    "$HOME/.local/bin/appimage-auto-gui"
    "$HOME/.local/share/systemd/user/appimage-auto.service"
    "$HOME/.local/share/applications/appimage-auto-gui.desktop"
    "$HOME/.local/share/icons/hicolor/256x256/apps/appimage-auto.png"
)

for f in "${files[@]}"; do
    if [[ -f "$f" ]]; then
        rm "$f"
        printf "  Removed %s\n" "$f"
    fi
done

# --- Reload systemd ---
systemctl --user daemon-reload 2>/dev/null || true

# --- Update caches ---
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
fi
if command -v update-desktop-database &>/dev/null; then
    update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true
fi

success "appimage-auto uninstalled."
echo ""

# --- Prompt to remove user data ---
remove_data=n
if [[ -t 0 ]]; then
    printf "%bRemove user configuration and state data? [y/N]%b " "$bold" "$reset"
    read -r remove_data
fi

if [[ "${remove_data,,}" == "y" ]]; then
    info "Removing user data..."
    if [[ -d "$HOME/.config/appimage-auto" ]]; then
        rm -r "$HOME/.config/appimage-auto"
        printf "  Removed %s\n" "$HOME/.config/appimage-auto/"
    fi
    if [[ -d "$HOME/.local/share/appimage-auto" ]]; then
        rm -r "$HOME/.local/share/appimage-auto"
        printf "  Removed %s\n" "$HOME/.local/share/appimage-auto/"
    fi
    success "User data removed."
else
    info "User data preserved:"
    printf "    %s\n" "$HOME/.config/appimage-auto/"
    printf "    %s\n" "$HOME/.local/share/appimage-auto/"
fi

echo ""
info "Desktop entries created by the daemon for integrated AppImages were left in place."
echo "  To remove them manually:"
echo "    rm ~/.local/share/applications/appimage-*.desktop"
