# appimage-auto

A minimal, focused Rust daemon that automatically integrates AppImages into the Linux desktop menu.

## Why?

Existing solutions have issues:

| Tool | Problem |
|------|---------|
| **go-appimaged** | Large surface area with extra features I don't need (Zeroconf, PubSub, P2P/IPFS, Firejail) |
| **AppImageLauncher** | Unmaintained since 2020, crashes on modern distros, uses problematic LD_PRELOAD |
| **Gear Lever** | Manual only, requires drag-and-drop |

You probably want `go-appimaged`. It is a "big" project with a larger team and more features. I
just want something that:
- notices I downloaded an AppImage,
- makes it executable,
- creates a .desktop file with the correct path and icon.
- *bonus* watches for moves and deletions to keep the desktop file updated.

**appimage-auto** does one thing well: watch directories and automatically integrate AppImages into your desktop menu.

## Features

- **Automatic Integration**: Detects new AppImages and creates menu entries
- **Magic Byte Validation**: Verifies ELF + AppImage signatures (not just file extensions)
- **Icon Extraction**: Installs icons to the correct hicolor theme directories
- **Move Tracking**: Updates menu entries when AppImages are moved within watched directories
- **Cleanup**: Removes menu entries when AppImages are deleted
- **Startup Scan**: Integrates existing AppImages when the daemon starts
- **Desktop Notifications**: Optional notifications when apps are integrated or removed
- **Desktop Agnostic**: Uses freedesktop.org standards (works with GNOME, KDE, XFCE, etc.)

## Installation

### From Source

```bash
# Clone and build
git clone https://github.com/youruser/appimage-auto
cd appimage-auto
cargo build --release

# Install binary
install -Dm755 target/release/appimage-auto ~/.cargo/bin/appimage-auto

# Install icon (for notifications)
install -Dm644 assets/icon.png ~/.local/share/icons/hicolor/256x256/apps/appimage-auto.png

# Install and enable systemd service
install -Dm644 systemd/appimage-auto.service ~/.local/share/systemd/user/appimage-auto.service
systemctl --user daemon-reload
systemctl --user enable --now appimage-auto
```

### Verify Installation

```bash
# Check daemon status
systemctl --user status appimage-auto

# View integrated AppImages
appimage-auto list

# Check configuration
appimage-auto status
```

## Usage

### CLI Commands

```bash
# Run daemon in foreground (for testing)
appimage-auto daemon

# One-shot scan (integrate existing, cleanup orphaned, exit)
appimage-auto scan

# Show status and statistics
appimage-auto status

# List all integrated AppImages
appimage-auto list

# Manually integrate a specific AppImage
appimage-auto integrate ~/Downloads/SomeApp.AppImage

# Remove integration for an AppImage
appimage-auto remove ~/Downloads/SomeApp.AppImage

# View current configuration
appimage-auto config show

# Add a watch directory
appimage-auto config add-watch ~/Apps

# Remove a watch directory
appimage-auto config remove-watch ~/Apps

# Verbose output (-v, -vv, -vvv)
appimage-auto -vv daemon
```

### Configuration

Configuration is stored at `~/.config/appimage-auto/config.toml`:

```toml
[watch]
# Directories to monitor for AppImages
directories = [
    "~/Downloads",
    "~/Applications",
    "~/.local/bin",
]

# File patterns (in addition to magic byte detection)
patterns = ["*.AppImage", "*.appimage"]

# Debounce delay in milliseconds
debounce_ms = 1000

[integration]
# Where to install .desktop files
desktop_dir = "~/.local/share/applications"

# Where to install icons
icon_dir = "~/.local/share/icons/hicolor"

# Run update-desktop-database after changes
update_database = true

# Scan for existing AppImages on startup
scan_on_startup = true

[logging]
level = "info"  # trace, debug, info, warn, error

[notifications]
# Enable desktop notifications
enabled = true

# Notify when an AppImage is integrated
on_integrate = true

# Notify when an AppImage is removed
on_unintegrate = true
```

## How It Works

### Detection

1. Monitors configured directories using inotify
2. When a file appears, checks for AppImage magic bytes:
   - ELF header: `0x7F 'E' 'L' 'F'`
   - AppImage signature at offset 8: `'A' 'I' 0x01` (Type 1) or `'A' 'I' 0x02` (Type 2)

### Integration

1. Makes the AppImage executable (`chmod +x`)
2. Extracts metadata using `--appimage-extract`
3. Parses the embedded `.desktop` file
4. Modifies `Exec=` to point to the actual AppImage path
5. Adds tracking identifier (`X-AppImage-Identifier`)
6. Installs icons to `~/.local/share/icons/hicolor/<size>/apps/`
7. Writes `.desktop` file to `~/.local/share/applications/`
8. Runs `update-desktop-database`

### State Tracking

State is persisted to `~/.local/share/appimage-auto/state.json`:

```json
{
  "integrated": {
    "abc123...": {
      "identifier": "abc123...",
      "appimage_path": "/home/user/Downloads/App.AppImage",
      "desktop_path": "/home/user/.local/share/applications/appimage-abc123.desktop",
      "icon_paths": ["/home/user/.local/share/icons/hicolor/256x256/apps/appimage-abc123.png"],
      "name": "App Name",
      "integrated_at": 1234567890,
      "updated_at": 1234567890
    }
  }
}
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     CLI Interface                        │
│         (daemon, scan, status, list, integrate)         │
└─────────────────────────────────────────────────────────┘
                           │
┌─────────────────────────────────────────────────────────┐
│                    Core Daemon                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │   Watcher   │  │  AppImage   │  │     State       │  │
│  │  (inotify)  │  │ Integrator  │  │    Manager      │  │
│  └─────────────┘  └─────────────┘  └─────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

### Modules

| Module | Purpose |
|--------|---------|
| `config.rs` | TOML configuration parsing with serde |
| `watcher.rs` | inotify-based file system monitoring via `notify` crate |
| `appimage.rs` | Magic byte detection, metadata extraction |
| `desktop.rs` | .desktop file parsing and generation |
| `state.rs` | JSON-based state persistence |
| `daemon.rs` | Main event loop coordinating all components |
| `notifications.rs` | Desktop notifications via `notify-rust` |

## Development

### Prerequisites

- Rust 1.70+ (uses 2024 edition features)
- Linux (uses inotify)

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run with verbose logging
cargo run -- -vv daemon
```

### Testing

```bash
# Download a test AppImage
curl -L "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage" \
  -o ~/Downloads/test.AppImage

# Run daemon and watch it integrate
cargo run -- daemon

# In another terminal, check the result
appimage-auto list
cat ~/.local/share/applications/appimage-*.desktop
```

## Systemd Service

The daemon runs as a systemd user service:

```ini
[Unit]
Description=AppImage Auto-Integration Daemon
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.cargo/bin/appimage-auto daemon
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target
```

### Service Management

```bash
# Start/stop/restart
systemctl --user start appimage-auto
systemctl --user stop appimage-auto
systemctl --user restart appimage-auto

# View logs
journalctl --user -u appimage-auto -f

# Disable autostart
systemctl --user disable appimage-auto
```

## Troubleshooting

### AppImage not appearing in menu

1. Check if it's a valid AppImage: `appimage-auto integrate /path/to/app.AppImage -v`
2. Verify the AppImage has an embedded `.desktop` file
3. Check logs: `journalctl --user -u appimage-auto`

### Menu entry not updating after move

The daemon only tracks moves within watched directories. Moving an AppImage to an unwatched location will remove its integration.

### Icons not showing

Some AppImages don't include icons. The integration will still work, but without a custom icon.

## License

MIT
