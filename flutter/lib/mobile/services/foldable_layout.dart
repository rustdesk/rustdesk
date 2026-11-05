import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// View-local geometry from UIKit. No device dimensions or hinge angles are
/// inferred: the OS decides when a division region is active.
class FoldableLayout {
  const FoldableLayout({
    this.supported = false,
    this.size = Size.zero,
    this.divisions = const [],
  });

  final bool supported;
  final Size size;
  final List<Rect> divisions;

  factory FoldableLayout.fromEvent(dynamic event) {
    final data = event as Map;
    return FoldableLayout(
      supported: data['supported'] == true,
      size: Size((data['width'] as num).toDouble(),
          (data['height'] as num).toDouble()),
      divisions: (data['divisions'] as List).map((dynamic item) {
        final rect = item as List;
        return Rect.fromLTWH(
          (rect[0] as num).toDouble(),
          (rect[1] as num).toDouble(),
          (rect[2] as num).toDouble(),
          (rect[3] as num).toDouble(),
        );
      }).toList(),
    );
  }

  static const _channel = EventChannel('rustdesk/foldable_layout');

  static Stream<FoldableLayout> get changes =>
      _channel.receiveBroadcastStream().map(FoldableLayout.fromEvent);
}

/// A route chooses its widget tree once. Hinge updates then change geometry,
/// never move the live session into a different Overlay.
class FoldableSessionLayout {
  FoldableLayout? _current;

  bool get ready => _current != null;
  FoldableLayout get current => _current ?? const FoldableLayout();

  void update(FoldableLayout layout) {
    _current = FoldableLayout(
      supported: _current?.supported ?? layout.supported,
      size: layout.size,
      divisions: layout.divisions,
    );
  }
}

class FoldableSessionGeometry {
  const FoldableSessionGeometry(this.canvas, this.controls,
      {this.divided = false});

  final Rect canvas;
  final Rect? controls;
  final bool divided;

  factory FoldableSessionGeometry.resolve({
    required Size size,
    required EdgeInsets padding,
    EdgeInsets viewInsets = EdgeInsets.zero,
    required FoldableLayout layout,
    required bool showControls,
  }) {
    final safe = Rect.fromLTRB(
        padding.left,
        padding.top,
        size.width - padding.right,
        size.height -
            (viewInsets.bottom > padding.bottom
                ? viewInsets.bottom
                : padding.bottom));
    // A resize may reach Flutter before the corresponding native fold update.
    // Never apply an old division to a differently sized view.
    if (layout.size == size) {
      for (final division in layout.divisions) {
        final fold = division.intersect(safe);
        if (fold.isEmpty) continue;
        if (viewInsets.bottom > 0 &&
            fold.width >= safe.width * .8 &&
            fold.top - safe.top >= 160) {
          final top = Rect.fromLTRB(safe.left, safe.top, safe.right, fold.top);
          return FoldableSessionGeometry(
            showControls
                ? Rect.fromLTRB(top.left, top.top, top.right - 48, top.bottom)
                : top,
            showControls
                ? Rect.fromLTRB(top.right - 48, top.top, top.right, top.bottom)
                : null,
          );
        }
        if (fold.width >= fold.height &&
            fold.width >= safe.width * .8 &&
            fold.top - safe.top >= 160 &&
            safe.bottom - fold.bottom >= 160) {
          return FoldableSessionGeometry(
            Rect.fromLTRB(safe.left, safe.top, safe.right, fold.top),
            showControls
                ? Rect.fromLTRB(safe.left, fold.bottom, safe.right, safe.bottom)
                : null,
            divided: true,
          );
        }
        if (fold.height > fold.width &&
            fold.height >= safe.height * .8 &&
            fold.left - safe.left >= 160 &&
            safe.right - fold.right >= 160) {
          return FoldableSessionGeometry(
            Rect.fromLTRB(safe.left, safe.top, fold.left, safe.bottom),
            showControls
                ? Rect.fromLTRB(fold.right, safe.top, safe.right, safe.bottom)
                : null,
            divided: true,
          );
        }
      }
    }
    if (showControls && safe.width >= 280 && safe.height >= 160) {
      // Preserve desktop width on portrait and square windows. Wide windows
      // benefit from retaining their height instead.
      if (safe.width <= safe.height * 1.6) {
        return FoldableSessionGeometry(
          Rect.fromLTRB(safe.left, safe.top, safe.right, safe.bottom - 48),
          Rect.fromLTRB(safe.left, safe.bottom - 48, safe.right, safe.bottom),
        );
      }
      return FoldableSessionGeometry(
        Rect.fromLTRB(safe.left, safe.top, safe.right - 48, safe.bottom),
        Rect.fromLTRB(safe.right - 48, safe.top, safe.right, safe.bottom),
      );
    }
    return FoldableSessionGeometry(safe, null);
  }
}
