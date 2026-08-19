# Borderless Window Implementation

The Flutter UI uses a frameless (borderless) window with a custom title bar.

## Changes Made

### 1. Dependencies (`pubspec.yaml`)
- Added `window_manager: 0.4.3` for cross-platform desktop window management

### 2. Window Initialization (`main.dart`)
- Initialize `window_manager` before app starts
- Configure `WindowOptions` with `titleBarStyle: TitleBarStyle.hidden`
- Set initial window size: 1280×800, minimum: 800×600

### 3. Custom Title Bar (`widgets/window_title_bar.dart`)
- **Height**: 48px (follows Figma design spec)
- **Background**: Themed (dark: `#0F0F23`, light: `#F2F2F2`)
- **Border**: 1px bottom border with 10% opacity
- **Left**: App icon (24×24) + "SuperApp" label
- **Right**: Window controls (minimize, maximize/restore, close)
- **Draggable**: Pan gesture triggers `windowManager.startDragging()`
- **Double-click**: Toggles maximize/restore
- **Close button**: Red hover effect (`#E81123`)
- **Theme sync**: Accepts `isDark` and `accentColor` from parent to stay in sync with shell theme

### 4. App Shell Integration (`app_shell.dart`)
- Removed inline caption bar implementation
- Added `WindowTitleBar` widget at the top of the column
- Passes `_themeMode` and `_getPrimaryColor()` to title bar
- Title bar accent color changes with theme selection (indigo/purple/pink)

## Architecture

```
┌─────────────────────────────────────────┐
│  WindowTitleBar (48px, draggable)       │ ← Custom, replaces OS chrome
├─────────────┬───────────────────────────┤
│   Sidebar   │   Top Bar                 │
│             ├───────────────────────────┤
│   (240px)   │   Main Content            │
│             │                           │
└─────────────┴───────────────────────────┘
```

## Window Controls

- **Minimize**: Minimizes to taskbar
- **Maximize/Restore**: Toggles full-screen, icon changes based on state
- **Close**: Closes application window
- All buttons: 48×48 touch target, hover effects, semantic labels

## Theme Behavior

The title bar tracks the shell's theme state:
- **Dark mode**: Near-black background (`#0F0F23`), white foreground
- **Light mode**: Light gray background (`#F2F2F2`), dark foreground
- **Accent color**: Brand mark matches selected theme (indigo/purple/pink)

## Running

```bash
cd ui
flutter run -d windows --release
```

The window launches frameless with the 48px title bar at the top.
