# Borderless Window Implementation

The Flutter UI now uses a frameless (borderless) window style with a custom caption bar.

## Changes Made

### 1. Dependencies (`pubspec.yaml`)
- Added `window_manager: 0.4.3` for cross-platform desktop window management

### 2. Window Initialization (`main.dart`)
- Initialize `window_manager` before app starts
- Configure `WindowOptions` with `titleBarStyle: TitleBarStyle.hidden`
- Set initial window size: 1280×800, minimum: 800×600

### 3. Custom Caption Bar (`app_shell.dart`)
- Added custom 32px draggable caption bar at the top
- Window controls: minimize, maximize/restore, close
- Drag-to-move via `windowManager.startDragging()`
- Close button has red hover effect (`#E81123`)
- Caption bar inherits the sidebar's themed color

### 4. Window Listener
- `AppShell` now implements `WindowListener` mixin
- Properly adds/removes listener in `initState`/`dispose`

## Architecture

```
┌─────────────────────────────────────────┐
│  Custom Caption Bar (32px, draggable)   │ ← Replaces OS title bar
├─────────────┬───────────────────────────┤
│   Sidebar   │   Top Bar                 │
│             ├───────────────────────────┤
│   (240px)   │   Main Content            │
│             │                           │
└─────────────┴───────────────────────────┘
```

## Result

- No OS-provided title bar or window borders
- Consistent appearance across Windows/macOS/Linux
- Full control over caption styling and theming
- Standard window operations (drag, minimize, maximize, close)

## Running

```bash
cd ui
flutter run -d windows --release
```

The window launches frameless with the custom caption bar active.
