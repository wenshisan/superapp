# SuperApp Flutter UI

The Flutter UI layer for the SuperApp pluggable agent framework.

## Architecture

This UI is built as a plugin-driven shell where the interface itself is implemented as plugins. The design follows the HTML prototypes in `design/pages/` which provide:

- **App shell** with sidebar navigation, top bar, and main content area
- **Dark/light mode** toggle with five color schemes (indigo, ocean, emerald, rose, amber)
- **Workbench view** showing tasks, API usage, news, and activity
- **Responsive layout** using Material 3 design patterns

## Structure

```
ui/
├── lib/
│   ├── main.dart         # App entry point
│   ├── app_shell.dart    # Main shell with sidebar + navigation
│   └── (future plugins)  # trace_viewer, plugin_list, chat, etc.
└── pubspec.yaml
```

## Running

```bash
# Development (Windows)
flutter run -d windows

# macOS
flutter run -d macos

# Linux
flutter run -d linux
```

## Design Source

The UI is based on HTML prototypes located at:
- `design/pages/AI Agent Workbench.html`
- `design/pages/Chat Open.html`

These prototypes use Tailwind CSS with a token-based theming system. The Flutter implementation translates those patterns into Material 3 widgets while preserving:
- Color scheme system (HSL-based primaries with dark mode variants)
- Sidebar with hue-tinted backgrounds
- Card-based layout with consistent border radius and shadows
- Responsive stat cards, progress indicators, and activity lists

## Next Steps

1. **Bridge layer** — connect to the Rust kernel via `flutter_rust_bridge`
2. **Plugin registry UI** — dynamic left nav driven by kernel plugin list
3. **Trace viewer** — real-time event log from `TraceCollector`
4. **Chat interface** — floating ask bubble for plugin generation

See `../ARCHITECTURE.md` for the full roadmap.
