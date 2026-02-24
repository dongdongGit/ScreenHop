---
name: ScreenHop Project
description: Comprehensive guide to the ScreenHop codebase — a cross-platform multi-monitor window hopping tool built with Rust
---

# ScreenHop Project Skill

## Overview

**ScreenHop** is a cross-platform desktop utility that moves windows between monitors via middle-click on the title bar. It is built in Rust as a Cargo workspace with 3 crates.

### Core Features
- Middle-click on a window's title bar → hop the window to the next monitor
- Smart relative positioning (preserves proportional location)
- Size adaptation (auto-shrink if target monitor is smaller)
- Interactive tab protection (skip browser/explorer tabs)
- Auto-start on boot
- GitHub Release-based update checking

## Project Structure

```
ScreenHop/
├── Cargo.toml              # Workspace root (resolver = "2")
├── crates/
│   ├── core/               # screenhop-core: shared logic
│   │   └── src/
│   │       ├── lib.rs       # Point, Rect, MonitorInfo types
│   │       ├── config.rs    # AppConfig (TOML-based persistence)
│   │       ├── monitor.rs   # Position calculation, monitor lookup
│   │       └── updater.rs   # GitHub release update checker
│   ├── platform/            # screenhop-platform: platform abstraction
│   │   └── src/
│   │       ├── lib.rs       # Traits + MouseEvent, WindowHandle types
│   │       ├── macos/       # macOS implementations
│   │       │   ├── mod.rs
│   │       │   ├── hook.rs       # CGEventTap-based mouse hook
│   │       │   ├── window.rs     # AXUIElement-based window management
│   │       │   ├── monitor.rs    # NSScreen monitor enumeration
│   │       │   ├── hittest.rs    # AX-based title bar / tab detection
│   │       │   └── autostart.rs  # LaunchAgent-based auto-start
│   │       └── windows/     # Windows implementations
│   │           ├── mod.rs
│   │           ├── hook.rs       # WH_MOUSE_LL low-level mouse hook
│   │           ├── window.rs     # SetWindowPos-based window management
│   │           ├── monitor.rs    # EnumDisplayMonitors enumeration
│   │           ├── hittest.rs    # WM_NCHITTEST-based hit testing
│   │           └── autostart.rs  # Task Scheduler-based auto-start
│   └── app/                 # screenhop: main binary
│       └── src/
│           ├── main.rs      # Entry point, single-instance guard, init
│           ├── engine.rs    # Hook installation + middle-click handler
│           └── tray.rs      # System tray icon + menu (tray-icon + muda)
└── .github/workflows/
    └── build.yml            # CI: cross-platform build + release
```

## Architecture

### Dependency Graph
```
app (screenhop) ──► platform (screenhop-platform) ──► core (screenhop-core)
       │                                                      │
       └──────────────────────────────────────────────────────┘
```

### Core Crate (`screenhop-core`)

| Module | Purpose |
|--------|---------|
| `lib.rs` | Defines `Point { x, y }`, `Rect { x, y, width, height }`, `MonitorInfo { id, bounds, work_area }` |
| `config.rs` | `AppConfig` with fields: `disable_hook`, `auto_start`, `start_minimized`, `auto_check_update`, `title_bar_height` (default 40px). Persists to `~/Library/Application Support/screenhop/config.toml` (macOS) or `%APPDATA%/screenhop/config.toml` (Windows) |
| `monitor.rs` | `calculate_new_position()` — maps window position proportionally from source to target monitor. `find_monitor_for_point()`, `next_monitor_index()`, `is_in_title_bar()` |
| `updater.rs` | `check_for_update()` (async) — queries GitHub Releases API, compares semver. `download_file()` with progress callback |

### Platform Crate (`screenhop-platform`)

**Key Traits** (defined in `lib.rs`):
- `MouseHook` — `start(callback)`, `stop()`, `is_active()`
- `WindowManager` — `get_window_at()`, `get_window_frame()`, `set_window_position()`, `set_window_size()`, `activate_window()`, `is_maximized()`, `restore_window()`, `maximize_window()`
- `HitTester` — `is_title_bar_hit()`, `is_interactive_tab()`
- `MonitorManager` — `get_monitors()`, `get_monitor_for_window()`
- `AutoStart` — `is_enabled()`, `set_enabled()`
- `PermissionChecker` — `check_permissions()`, `request_permissions()`

**Key Types**:
- `MouseEvent { point: Point, button: u32 }`
- `WindowHandle { inner: MacWindowHandle | WinWindowHandle }` (platform-specific inner)

**Factory**: `create_platform()` returns `MacPlatform` or `WinPlatform` based on `cfg(target_os)`

**macOS Technologies**: CGEventTap, AXUIElement (Accessibility), NSScreen, Core Graphics, objc2/cocoa
**Windows Technologies**: WH_MOUSE_LL, SetWindowPos, EnumDisplayMonitors, WM_NCHITTEST, Task Scheduler COM API

### App Crate (`screenhop`)

| Module | Purpose |
|--------|---------|
| `main.rs` | Initializes logger (`env_logger`), single-instance check (`single-instance`), loads `AppConfig`, checks permissions (macOS), installs hook, runs tray event loop |
| `engine.rs` | `install_hook()` — creates platform hook and routes events to `handle_middle_click()`. The handler: get window → check tab → get frame → verify title bar → find monitors → calculate position → move + resize + activate |
| `tray.rs` | Creates system tray with `tray-icon`/`muda`. Menu items: status (disabled), toggle hook, auto-start, check update, quit. macOS uses NSApp run loop; Windows uses MenuEvent recv loop |

## Key Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `objc2` / `objc2-foundation` / `objc2-app-kit` | 0.5 / 0.2 | Modern Objective-C bindings |
| `core-graphics` / `core-foundation` / `cocoa` / `objc` | 0.24 / 0.10 / 0.26 / 0.2 | macOS native APIs |
| `windows` | 0.58 | Windows API bindings |
| `tray-icon` / `muda` | 0.19 / 0.15 | Cross-platform system tray and menu |
| `reqwest` | 0.12 | HTTP client for update checking |
| `tokio` | 1 | Async runtime |
| `serde` / `toml` | 1.0 / 0.8 | Config serialization |
| `semver` | 1.0 | Version comparison |
| `single-instance` | 0.3 | Process single-instance guard |
| `log` / `env_logger` | 0.4 / 0.11 | Logging |

## Building

```bash
# Debug build (current platform only)
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Binary output
# macOS: target/release/screenhop
# Windows: target/release/screenhop.exe
```

## Development Conventions

1. **Language**: Code comments and log messages are in Chinese (中文)
2. **Platform Abstraction**: All platform-specific code goes in `crates/platform/src/{macos,windows}/`. Shared logic stays in `crates/core/`
3. **Conditional Compilation**: Use `#[cfg(target_os = "macos")]` and `#[cfg(target_os = "windows")]` for platform-specific blocks
4. **Error Handling**: Use `anyhow::Result` throughout. Log errors with `log::error!()` but continue gracefully where possible
5. **Configuration**: All user-configurable values go through `AppConfig` and are persisted as TOML
6. **Trait-based Design**: Platform capabilities are expressed as traits in `platform/src/lib.rs`, implemented per platform
7. **App ID**: `com.dongdong.screenhop`
8. **Workspace**: All shared dependency versions are managed in the root `Cargo.toml` `[workspace.dependencies]`

## Adding a New Platform Feature

1. Add the trait method to the appropriate trait in `crates/platform/src/lib.rs`
2. Implement for macOS in `crates/platform/src/macos/`
3. Implement for Windows in `crates/platform/src/windows/`
4. Use in `crates/app/` with `#[cfg(target_os = ...)]` blocks if platform-specific, or through the trait directly

## Adding a New Configuration Option

1. Add the field to `AppConfig` in `crates/core/src/config.rs` with `#[serde(default)]` or a custom default function
2. Update `Default` impl
3. Use in the app crate (`engine.rs` or `tray.rs`)
4. Add a menu item in `tray.rs` if user-facing
