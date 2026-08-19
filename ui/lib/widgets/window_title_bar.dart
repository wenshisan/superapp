import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

/// Custom window title bar for borderless window.
///
/// Specs:
/// - Height: 48px
/// - Left: App branding (icon + title)
/// - Right: window control buttons (minimize / maximize / close)
/// - Draggable area for window movement
/// - Double-click to maximize/restore
class WindowTitleBar extends StatefulWidget {
  const WindowTitleBar({
    super.key,
    required this.isDark,
    required this.accentColor,
  });

  /// Whether to render in dark mode. Passed in rather than read from
  /// [Theme.of] because the shell tracks its own theme state.
  final bool isDark;

  /// Accent color for the brand mark, follows the shell's color scheme.
  final Color accentColor;

  @override
  State<WindowTitleBar> createState() => _WindowTitleBarState();
}

class _WindowTitleBarState extends State<WindowTitleBar> with WindowListener {
  bool _isMaximized = false;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    _syncMaximized();
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    super.dispose();
  }

  Future<void> _syncMaximized() async {
    final maximized = await windowManager.isMaximized();
    if (mounted && maximized != _isMaximized) {
      setState(() => _isMaximized = maximized);
    }
  }

  @override
  void onWindowMaximize() => setState(() => _isMaximized = true);

  @override
  void onWindowUnmaximize() => setState(() => _isMaximized = false);

  Color get _backgroundColor => widget.isDark
      ? const Color(0xFF0F0F23)
      : const Color(0xFFF2F2F2);

  Color get _borderColor => widget.isDark
      ? Colors.white.withValues(alpha: 0.05)
      : Colors.black.withValues(alpha: 0.1);

  Color get _foregroundColor => widget.isDark
      ? Colors.white.withValues(alpha: 0.9)
      : Colors.black.withValues(alpha: 0.9);

  @override
  Widget build(BuildContext context) {
    return Container(
      height: 48,
      decoration: BoxDecoration(
        color: _backgroundColor,
        border: Border(
          bottom: BorderSide(color: _borderColor, width: 1),
        ),
      ),
      // Stack: draggable layer + visible content (decoupled gesture arena)
      child: Stack(
        children: [
          // Bottom layer: dragging and double-click maximize
          Positioned.fill(
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onPanStart: (_) => windowManager.startDragging(),
              onDoubleTap: () {
                if (_isMaximized) {
                  windowManager.unmaximize();
                } else {
                  windowManager.maximize();
                }
              },
            ),
          ),
          // Top layer: branding + window control buttons
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              // Left: App branding
              const SizedBox(width: 16),
              Container(
                width: 24,
                height: 24,
                decoration: BoxDecoration(
                  color: widget.accentColor,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: const Icon(
                  Icons.smart_toy,
                  size: 14,
                  color: Colors.white,
                ),
              ),
              const SizedBox(width: 8),
              Text(
                'SuperApp',
                style: TextStyle(
                  fontSize: 13,
                  fontWeight: FontWeight.w500,
                  color: _foregroundColor,
                ),
              ),
              // Fill middle area (draggable)
              const Spacer(),
              // Right: window control buttons
              _WindowControlButton(
                icon: Icons.remove,
                onPressed: () => windowManager.minimize(),
                semanticLabel: 'Minimize',
                isDark: widget.isDark,
              ),
              _WindowControlButton(
                icon: _isMaximized
                    ? Icons.fullscreen_exit_rounded
                    : Icons.crop_square_rounded,
                onPressed: () {
                  if (_isMaximized) {
                    windowManager.unmaximize();
                  } else {
                    windowManager.maximize();
                  }
                },
                semanticLabel: _isMaximized ? 'Restore' : 'Maximize',
                isDark: widget.isDark,
              ),
              _WindowControlButton(
                icon: Icons.close_rounded,
                onPressed: () => windowManager.close(),
                hoverColor: const Color(0xFFE81123),
                semanticLabel: 'Close',
                isDark: widget.isDark,
              ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Window control button: 48×48 clickable area, shows background on hover
class _WindowControlButton extends StatefulWidget {
  const _WindowControlButton({
    required this.icon,
    required this.onPressed,
    required this.semanticLabel,
    required this.isDark,
    this.hoverColor,
  });

  final IconData icon;
  final VoidCallback onPressed;
  final String semanticLabel;
  final bool isDark;
  final Color? hoverColor;

  @override
  State<_WindowControlButton> createState() => _WindowControlButtonState();
}

class _WindowControlButtonState extends State<_WindowControlButton> {
  bool _hovered = false;

  @override
  Widget build(BuildContext context) {
    final Color iconColor = _hovered && widget.hoverColor != null
        ? Colors.white
        : (widget.isDark
            ? Colors.white.withValues(alpha: 0.8)
            : Colors.black.withValues(alpha: 0.8));

    final Color bg = _hovered
        ? (widget.hoverColor ??
            (widget.isDark
                ? Colors.white.withValues(alpha: 0.1)
                : const Color(0xFFE5E7EB)))
        : Colors.transparent;

    return Semantics(
      label: widget.semanticLabel,
      button: true,
      child: MouseRegion(
        onEnter: (_) => setState(() => _hovered = true),
        onExit: (_) => setState(() => _hovered = false),
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onTap: widget.onPressed,
          child: Container(
            width: 48,
            height: 48,
            alignment: Alignment.center,
            color: bg,
            child: Icon(widget.icon, size: 16, color: iconColor),
          ),
        ),
      ),
    );
  }
}
