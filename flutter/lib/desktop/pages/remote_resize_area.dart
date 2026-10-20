import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';

import 'remote_window_resize.dart';

class RemoteResizeArea extends StatelessWidget {
  final int windowId;
  final Widget child;
  final double resizeEdgeSize;
  final EdgeInsets childPadding;
  final List<SubWindowResizeEdge>? enableResizeEdges;

  const RemoteResizeArea({
    super.key,
    required this.windowId,
    required this.child,
    required this.resizeEdgeSize,
    required this.childPadding,
    this.enableResizeEdges,
  });

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<bool>(
      valueListenable: RemoteWindowResize.enabled,
      child: child,
      builder: (context, enabled, child) => LayoutBuilder(
        builder: (context, constraints) {
          final width = constraints.maxWidth;
          final height = constraints.maxHeight;
          final edge = resizeEdgeSize;
          Widget region(
              Rect rect, SubWindowResizeEdge side, SubWindowResizeEdge corner) {
            if (enableResizeEdges != null &&
                !enableResizeEdges!.contains(side)) {
              return const SizedBox.shrink();
            }
            final cursor = corner == SubWindowResizeEdge.topLeft ||
                    corner == SubWindowResizeEdge.bottomRight
                ? SystemMouseCursors.resizeUpLeftDownRight
                : SystemMouseCursors.resizeUpRightDownLeft;
            return Positioned.fromRect(
              rect: rect,
              child: MouseRegion(
                cursor: cursor,
                child: Listener(
                  behavior: HitTestBehavior.opaque,
                  onPointerDown: (_) => WindowController.fromWindowId(windowId)
                      .startResizing(corner),
                  child: GestureDetector(
                    behavior: HitTestBehavior.opaque,
                    onDoubleTap: side == SubWindowResizeEdge.top ||
                            side == SubWindowResizeEdge.bottom
                        ? () =>
                            WindowController.fromWindowId(windowId).maximize()
                        : null,
                  ),
                ),
              ),
            );
          }

          return Stack(children: [
            SubWindowDragToResizeArea(
              windowId: windowId,
              resizeEdgeSize: resizeEdgeSize,
              childPadding: childPadding,
              enableResizeEdges: enabled ? const [] : enableResizeEdges,
              child: child!,
            ),
            if (enabled && edge > 0) ...[
              region(Rect.fromLTRB(0, 0, width / 2, edge),
                  SubWindowResizeEdge.top, SubWindowResizeEdge.topLeft),
              region(Rect.fromLTRB(width / 2, 0, width, edge),
                  SubWindowResizeEdge.top, SubWindowResizeEdge.topRight),
              region(Rect.fromLTRB(0, height - edge, width / 2, height),
                  SubWindowResizeEdge.bottom, SubWindowResizeEdge.bottomLeft),
              region(Rect.fromLTRB(width / 2, height - edge, width, height),
                  SubWindowResizeEdge.bottom, SubWindowResizeEdge.bottomRight),
              region(Rect.fromLTRB(0, 0, edge, height / 2),
                  SubWindowResizeEdge.left, SubWindowResizeEdge.topLeft),
              region(Rect.fromLTRB(0, height / 2, edge, height),
                  SubWindowResizeEdge.left, SubWindowResizeEdge.bottomLeft),
              region(Rect.fromLTRB(width - edge, 0, width, height / 2),
                  SubWindowResizeEdge.right, SubWindowResizeEdge.topRight),
              region(Rect.fromLTRB(width - edge, height / 2, width, height),
                  SubWindowResizeEdge.right, SubWindowResizeEdge.bottomRight),
            ],
          ]);
        },
      ),
    );
  }
}
