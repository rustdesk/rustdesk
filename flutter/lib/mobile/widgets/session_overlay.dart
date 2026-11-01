import 'package:flutter/widgets.dart';

/// Keeps the session content and its full-window dialog host mounted while
/// folding and keyboard insets change the pane geometry.
class SessionOverlay extends StatefulWidget {
  const SessionOverlay({super.key, required this.child, this.overlayKey});

  final Widget child;
  final GlobalKey<OverlayState>? overlayKey;

  @override
  State<SessionOverlay> createState() => _SessionOverlayState();
}

class _SessionOverlayState extends State<SessionOverlay> {
  late final _entry = OverlayEntry(builder: (_) => widget.child);

  @override
  void didUpdateWidget(SessionOverlay oldWidget) {
    super.didUpdateWidget(oldWidget);
    _entry.markNeedsBuild();
  }

  @override
  void dispose() {
    _entry.remove();
    _entry.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      Overlay(key: widget.overlayKey, initialEntries: [_entry]);
}
