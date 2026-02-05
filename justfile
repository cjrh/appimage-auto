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
