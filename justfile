# appimage-auto development tasks

# Default: show available recipes
default:
    @just --list

# Build debug version
build:
    cargo build

# Build release version
release:
    cargo build --release

# Build and strip release binary
release-stripped: release
    strip target/release/appimage-auto
    @ls -lh target/release/appimage-auto

# Run all tests
test:
    cargo test

# Run tests with output
test-verbose:
    cargo test -- --nocapture

# Run clippy lints
lint:
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Check formatting without changing files
fmt-check:
    cargo fmt -- --check

# Run all checks (format, lint, test)
check: fmt-check lint test

# Run daemon in foreground with debug logging
run:
    cargo run -- -vv daemon

# Run one-shot scan
scan:
    cargo run -- scan

# Show status
status:
    cargo run -- status

# List integrated AppImages
list:
    cargo run -- list

# Show current config
config:
    cargo run -- config show

# Install to ~/.cargo/bin
install: release-stripped
    install -Dm755 target/release/appimage-auto ~/.cargo/bin/appimage-auto
    @echo "Installed to ~/.cargo/bin/appimage-auto"

# Install systemd service
install-service:
    install -Dm644 systemd/appimage-auto.service ~/.local/share/systemd/user/appimage-auto.service
    systemctl --user daemon-reload
    @echo "Service installed. Enable with: systemctl --user enable --now appimage-auto"

# Install XDG autostart entry (alternative to systemd)
autostart-install:
    install -Dm644 autostart/appimage-auto.desktop ~/.config/autostart/appimage-auto.desktop
    @echo "Autostart entry installed. The daemon will start on next login."

# Uninstall XDG autostart entry
autostart-uninstall:
    -rm ~/.config/autostart/appimage-auto.desktop
    @echo "Autostart entry removed."

# Full install (binary + service)
install-all: install install-service
    @echo "Installation complete!"

# Uninstall binary and service
uninstall:
    -systemctl --user disable --now appimage-auto
    -rm ~/.cargo/bin/appimage-auto
    -rm ~/.local/share/systemd/user/appimage-auto.service
    systemctl --user daemon-reload
    @echo "Uninstalled"

# Start the systemd service
start:
    systemctl --user start appimage-auto

# Stop the systemd service
stop:
    systemctl --user stop appimage-auto

# Restart the systemd service
restart:
    systemctl --user restart appimage-auto

# View service logs (follow mode)
logs:
    journalctl --user -u appimage-auto -f

# View recent service logs
logs-recent:
    journalctl --user -u appimage-auto -n 50

# Download test AppImage for development
download-test-appimage:
    curl -L "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
        -o /tmp/test.AppImage
    @echo "Downloaded to /tmp/test.AppImage"

# Test integration with downloaded AppImage
test-integrate: download-test-appimage
    cargo run -- integrate /tmp/test.AppImage
    cargo run -- list

# Clean up test integration
test-cleanup:
    -cargo run -- remove /tmp/test.AppImage
    -rm /tmp/test.AppImage
    cargo run -- list

# Clean build artifacts
clean:
    cargo clean

# Show binary size comparison
size:
    @echo "Debug:"
    @ls -lh target/debug/appimage-auto 2>/dev/null || echo "  (not built)"
    @echo "Release:"
    @ls -lh target/release/appimage-auto 2>/dev/null || echo "  (not built)"

# Generate documentation
docs:
    cargo doc --open

# Watch for changes and run tests
watch:
    cargo watch -x test

# Watch for changes and run clippy
watch-lint:
    cargo watch -x clippy

# Build GUI with gtk4/libadwaita
build-gui:
    cargo build --features gui

# Build GUI in release mode
release-gui:
    cargo build --release --features gui

# Build and strip release GUI binary
release-gui-stripped: release-gui
    strip target/release/appimage-auto-gui
    @ls -lh target/release/appimage-auto-gui

# Run the GUI
run-gui:
    cargo run --bin appimage-auto-gui --features gui

# Install GUI binary and desktop file
install-gui: release-gui-stripped
    install -Dm755 target/release/appimage-auto-gui ~/.cargo/bin/appimage-auto-gui
    install -Dm644 desktop/appimage-auto-gui.desktop ~/.local/share/applications/appimage-auto-gui.desktop
    @echo "Installed GUI to ~/.cargo/bin/appimage-auto-gui"
    @echo "Desktop entry installed to ~/.local/share/applications/"

# Uninstall GUI
uninstall-gui:
    -rm ~/.cargo/bin/appimage-auto-gui
    -rm ~/.local/share/applications/appimage-auto-gui.desktop
    @echo "GUI uninstalled"

# Full install including GUI
install-all-gui: install install-gui
    @echo "Full installation with GUI complete!"

# Build release binaries and package into a distributable zip
bundle: release-stripped release-gui-stripped
    #!/usr/bin/env bash
    set -euo pipefail

    VERSION=$(git describe --tags --exact-match HEAD 2>/dev/null || git rev-parse --short HEAD)
    ZIPNAME="appimage-auto-${VERSION}-x86_64-linux.zip"
    STAGING=$(mktemp -d)
    trap 'rm -rf "$STAGING"' EXIT

    mkdir -p "$STAGING/appimage-auto"/{bin,systemd,desktop,config,assets}

    cp target/release/appimage-auto "$STAGING/appimage-auto/bin/"
    if [[ -f target/release/appimage-auto-gui ]]; then
        cp target/release/appimage-auto-gui "$STAGING/appimage-auto/bin/"
    fi
    cp systemd/appimage-auto.service "$STAGING/appimage-auto/systemd/"
    cp desktop/appimage-auto-gui.desktop "$STAGING/appimage-auto/desktop/"
    cp config/default.toml "$STAGING/appimage-auto/config/"
    cp assets/icon.png "$STAGING/appimage-auto/assets/"
    cp install.sh uninstall.sh "$STAGING/appimage-auto/"

    (cd "$STAGING" && zip -r - appimage-auto/) > "$ZIPNAME"

    echo "Bundled: $ZIPNAME ($(du -h "$ZIPNAME" | cut -f1))"

# End-to-end test of bundle → install → verify → uninstall → verify
bundle-test: bundle
    #!/usr/bin/env bash
    set -euo pipefail

    red='\033[0;31m' green='\033[0;32m' bold='\033[1m' reset='\033[0m'
    pass() { printf "${green}  PASS${reset} %s\n" "$*"; }
    fail() { printf "${red}  FAIL${reset} %s\n" "$*"; FAILURES=$((FAILURES + 1)); }
    section() { printf "\n${bold}--- %s ---${reset}\n" "$*"; }
    check_exists()     { [[ -e "$1" ]] && pass "$1 exists"         || fail "$1 missing"; }
    check_not_exists() { [[ ! -e "$1" ]] && pass "$1 removed"     || fail "$1 still exists"; }
    check_executable() { [[ -x "$1" ]] && pass "$1 is executable" || fail "$1 not executable"; }

    FAILURES=0
    ZIPNAME=$(ls appimage-auto-*-x86_64-linux.zip 2>/dev/null | head -1)
    if [[ -z "$ZIPNAME" ]]; then
        echo "No bundle zip found" >&2; exit 1
    fi

    EXTRACT_DIR=$(mktemp -d)
    trap 'rm -rf "$EXTRACT_DIR"; rm -f "$ZIPNAME"' EXIT

    # ── Step 1: Verify zip contents ──────────────────────────────
    section "Zip contents"
    for entry in \
        appimage-auto/bin/appimage-auto \
        appimage-auto/systemd/appimage-auto.service \
        appimage-auto/config/default.toml \
        appimage-auto/assets/icon.png \
        appimage-auto/install.sh \
        appimage-auto/uninstall.sh; do
        unzip -l "$ZIPNAME" | grep -q "$entry" && pass "$entry in zip" || fail "$entry missing from zip"
    done

    # ── Step 2: Extract and check permissions ────────────────────
    section "Extract"
    unzip -q "$ZIPNAME" -d "$EXTRACT_DIR"
    check_executable "$EXTRACT_DIR/appimage-auto/bin/appimage-auto"
    check_executable "$EXTRACT_DIR/appimage-auto/install.sh"
    check_executable "$EXTRACT_DIR/appimage-auto/uninstall.sh"
    if [[ -f "$EXTRACT_DIR/appimage-auto/bin/appimage-auto-gui" ]]; then
        check_executable "$EXTRACT_DIR/appimage-auto/bin/appimage-auto-gui"
    fi

    # ── Step 3: Install ──────────────────────────────────────────
    section "Install"
    bash "$EXTRACT_DIR/appimage-auto/install.sh" 2>&1
    check_executable "$HOME/.local/bin/appimage-auto"
    check_exists "$HOME/.local/share/systemd/user/appimage-auto.service"
    check_exists "$HOME/.local/share/icons/hicolor/256x256/apps/appimage-auto.png"
    check_exists "$HOME/.config/appimage-auto/config.toml"

    # Verify sed rewrites
    if grep -q 'ExecStart=%h/.local/bin/appimage-auto' \
        "$HOME/.local/share/systemd/user/appimage-auto.service"; then
        pass "ExecStart rewritten to .local/bin"
    else
        fail "ExecStart not rewritten"
    fi

    if [[ -f "$HOME/.local/bin/appimage-auto-gui" ]]; then
        check_executable "$HOME/.local/bin/appimage-auto-gui"
        check_exists "$HOME/.local/share/applications/appimage-auto-gui.desktop"
        if grep -q "Exec=$HOME/.local/bin/appimage-auto-gui" \
            "$HOME/.local/share/applications/appimage-auto-gui.desktop"; then
            pass "Desktop Exec has absolute path"
        else
            fail "Desktop Exec not rewritten"
        fi
    fi

    # Binary runs
    if "$HOME/.local/bin/appimage-auto" --help >/dev/null 2>&1; then
        pass "appimage-auto --help"
    else
        fail "appimage-auto --help"
    fi

    # ── Step 4: Daemon starts ────────────────────────────────────
    section "Daemon"
    sleep 2
    if systemctl --user is-active appimage-auto.service >/dev/null 2>&1; then
        pass "Service is active"
    else
        fail "Service is not active"
        journalctl --user -u appimage-auto -n 5 --no-pager 2>/dev/null || true
    fi

    # ── Step 5: Uninstall (keep data) ────────────────────────────
    section "Uninstall (keep data)"
    bash "$EXTRACT_DIR/appimage-auto/uninstall.sh" --keep-data 2>&1
    check_not_exists "$HOME/.local/bin/appimage-auto"
    check_not_exists "$HOME/.local/bin/appimage-auto-gui"
    check_not_exists "$HOME/.local/share/systemd/user/appimage-auto.service"
    check_not_exists "$HOME/.local/share/applications/appimage-auto-gui.desktop"
    check_not_exists "$HOME/.local/share/icons/hicolor/256x256/apps/appimage-auto.png"
    check_exists "$HOME/.config/appimage-auto"
    if ! systemctl --user is-active appimage-auto.service >/dev/null 2>&1; then
        pass "Service is not running"
    else
        fail "Service still running"
    fi

    # ── Step 6: Re-install, then uninstall (remove data) ────────
    section "Uninstall (remove data)"
    bash "$EXTRACT_DIR/appimage-auto/install.sh" 2>&1
    sleep 1
    bash "$EXTRACT_DIR/appimage-auto/uninstall.sh" --remove-data 2>&1
    check_not_exists "$HOME/.local/bin/appimage-auto"
    check_not_exists "$HOME/.local/share/systemd/user/appimage-auto.service"
    check_not_exists "$HOME/.config/appimage-auto"
    check_not_exists "$HOME/.local/share/appimage-auto"

    # ── Summary ──────────────────────────────────────────────────
    echo ""
    if [[ $FAILURES -eq 0 ]]; then
        printf "${green}${bold}All checks passed.${reset}\n"
    else
        printf "${red}${bold}%d check(s) failed.${reset}\n" "$FAILURES"
        exit 1
    fi
