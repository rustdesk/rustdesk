import 'dart:io';

import 'package:flutter/material.dart';

import '../window_controller.dart';

/// From window manager
enum SubWindowResizeEdge {
  top,
  left,
  right,
  bottom,
  topLeft,
  bottomLeft,
  topRight,
  bottomRight
}

class DragToResizeEdge extends StatefulWidget {
  final SubWindowResizeEdge resizeEdge;
  final double? width;
  final double? height;
  final Color resizeEdgeColor;
  final MouseCursor resizeCursor;
  final int windowId;
  DragToResizeEdge({
    Key? key,
    this.width,
    this.height,
    required this.resizeEdge,
    required this.resizeEdgeColor,
    required this.resizeCursor,
    required this.windowId,
  });

  @override
  State<DragToResizeEdge> createState() => _DragToResizeEdgeState();
}

class _DragToResizeEdgeState extends State<DragToResizeEdge> {
  MouseCursor cursor = MouseCursor.defer;

  @override
  void initState() {
    cursor = widget.resizeCursor;
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      width: widget.width,
      height: widget.height,
      color: widget.resizeEdgeColor,
      child: Listener(
        onPointerDown: (_) => WindowController.fromWindowId(widget.windowId)
            .startResizing(widget.resizeEdge),
        onPointerUp: (_) => setState(() {
          cursor = widget.resizeCursor;
        }),
        child: MouseRegion(
          cursor: cursor,
          onEnter: (evt) => setState(() {
            cursor = evt.buttons != 0 ? MouseCursor.defer : widget.resizeCursor;
          }),
          child: GestureDetector(
            onDoubleTap: () => (Platform.isWindows &&
                    (widget.resizeEdge == SubWindowResizeEdge.top ||
                        widget.resizeEdge == SubWindowResizeEdge.bottom))
                ? WindowController.fromWindowId(widget.windowId).maximize()
                : null,
          ),
        ),
      ),
    );
  }
}

class SubWindowDragToResizeArea extends StatelessWidget {
  final int windowId;
  final Widget child;
  final double resizeEdgeSize;
  final Color resizeEdgeColor;
  final EdgeInsets resizeEdgeMargin;
  final EdgeInsets childPadding;
  final List<SubWindowResizeEdge>? enableResizeEdges;

  const SubWindowDragToResizeArea({
    Key? key,
    required this.windowId,
    required this.child,
    this.resizeEdgeColor = Colors.transparent,
    this.resizeEdgeSize = 8,
    this.resizeEdgeMargin = EdgeInsets.zero,
    this.enableResizeEdges,
    this.childPadding = EdgeInsets.zero,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    getOffstage(SubWindowResizeEdge resizeEdge) =>
        enableResizeEdges != null && !enableResizeEdges!.contains(resizeEdge);

    return Stack(
      children: <Widget>[
        Container(
          margin: childPadding,
          child: child,
        ),
        Positioned(
          child: Container(
            margin: resizeEdgeMargin,
            child: Column(
              children: [
                Row(
                  children: [
                    Offstage(
                      offstage: getOffstage(SubWindowResizeEdge.topLeft),
                      child: DragToResizeEdge(
                        resizeEdge: SubWindowResizeEdge.topLeft,
                        width: resizeEdgeSize,
                        height: resizeEdgeSize,
                        resizeEdgeColor: resizeEdgeColor,
                        resizeCursor: SystemMouseCursors.resizeUpLeft,
                        windowId: windowId,
                      ),
                    ),
                    Expanded(
                      flex: 1,
                      child: Offstage(
                        offstage: getOffstage(SubWindowResizeEdge.top),
                        child: DragToResizeEdge(
                          resizeEdge: SubWindowResizeEdge.top,
                          height: resizeEdgeSize,
                          resizeEdgeColor: resizeEdgeColor,
                          resizeCursor: SystemMouseCursors.resizeUp,
                          windowId: windowId,
                        ),
                      ),
                    ),
                    Offstage(
                      offstage: getOffstage(SubWindowResizeEdge.topRight),
                      child: DragToResizeEdge(
                        resizeEdge: SubWindowResizeEdge.topRight,
                        width: resizeEdgeSize,
                        height: resizeEdgeSize,
                        resizeEdgeColor: resizeEdgeColor,
                        resizeCursor: SystemMouseCursors.resizeUpRight,
                        windowId: windowId,
                      ),
                    ),
                  ],
                ),
                Expanded(
                  flex: 1,
                  child: Row(
                    children: [
                      Offstage(
                        offstage: getOffstage(SubWindowResizeEdge.left),
                        child: DragToResizeEdge(
                          resizeEdge: SubWindowResizeEdge.left,
                          width: resizeEdgeSize,
                          height: double.infinity,
                          resizeEdgeColor: resizeEdgeColor,
                          resizeCursor: SystemMouseCursors.resizeLeft,
                          windowId: windowId,
                        ),
                      ),
                      Offstage(
                        offstage: getOffstage(SubWindowResizeEdge.left),
                        child: DragToResizeEdge(
                          resizeEdge: SubWindowResizeEdge.left,
                          width: resizeEdgeSize,
                          height: double.infinity,
                          resizeEdgeColor: resizeEdgeColor,
                          resizeCursor: SystemMouseCursors.resizeLeft,
                          windowId: windowId,
                        ),
                      ),
                      Expanded(
                        flex: 1,
                        child: Container(),
                      ),
                      Offstage(
                        offstage: getOffstage(SubWindowResizeEdge.right),
                        child: DragToResizeEdge(
                          resizeEdge: SubWindowResizeEdge.right,
                          width: resizeEdgeSize,
                          height: double.infinity,
                          resizeEdgeColor: resizeEdgeColor,
                          resizeCursor: SystemMouseCursors.resizeRight,
                          windowId: windowId,
                        ),
                      ),
                    ],
                  ),
                ),
                Row(
                  children: [
                    Offstage(
                      offstage: getOffstage(SubWindowResizeEdge.bottomLeft),
                      child: DragToResizeEdge(
                        resizeEdge: SubWindowResizeEdge.bottomLeft,
                        width: resizeEdgeSize,
                        height: resizeEdgeSize,
                        resizeEdgeColor: resizeEdgeColor,
                        resizeCursor: SystemMouseCursors.resizeDownLeft,
                        windowId: windowId,
                      ),
                    ),
                    Expanded(
                      flex: 1,
                      child: Offstage(
                        offstage: getOffstage(SubWindowResizeEdge.bottom),
                        child: DragToResizeEdge(
                          resizeEdge: SubWindowResizeEdge.bottom,
                          height: resizeEdgeSize,
                          resizeEdgeColor: resizeEdgeColor,
                          resizeCursor: SystemMouseCursors.resizeDown,
                          windowId: windowId,
                        ),
                      ),
                    ),
                    Offstage(
                      offstage: getOffstage(SubWindowResizeEdge.bottomRight),
                      child: DragToResizeEdge(
                        resizeEdge: SubWindowResizeEdge.bottomRight,
                        width: resizeEdgeSize,
                        height: resizeEdgeSize,
                        resizeEdgeColor: resizeEdgeColor,
                        resizeCursor: SystemMouseCursors.resizeDownRight,
                        windowId: windowId,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}
