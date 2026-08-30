import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// The function called when the user scrolls the [InfiniteScrollView]. [offset]
/// is the current offset of the scroll view, ranging from [double.negativeInfinity]
/// to [double.infinity].
typedef ScrollCallback = void Function(double offset);

/// A [Scrollable] that can be scrolled infinitely in both directions. When
/// scroll happens, the [onScroll] callback is called with the new offset.
class InfiniteScrollView extends StatelessWidget {
  const InfiniteScrollView({
    super.key,
    required this.onScroll,
    required this.child,
  });

  final ScrollCallback onScroll;

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Scrollable(
      viewportBuilder: (context, position) {
        return _InfiniteScrollView(
          position: position,
          onScroll: onScroll,
          child: child,
        );
      },
    );
  }
}

class _InfiniteScrollView extends SingleChildRenderObjectWidget {
  const _InfiniteScrollView({
    // super.key,
    super.child,
    required this.position,
    required this.onScroll,
  });

  final ViewportOffset position;

  final ScrollCallback onScroll;

  @override
  RenderObject createRenderObject(BuildContext context) {
    return _RenderInfiniteScrollView(
      position: position,
      onScroll: onScroll,
    );
  }

  @override
  void updateRenderObject(
    BuildContext context,
    _RenderInfiniteScrollView renderObject,
  ) {
    renderObject
      ..position = position
      ..onScroll = onScroll;
  }
}

class _RenderInfiniteScrollView extends RenderShiftedBox {
  _RenderInfiniteScrollView({
    RenderBox? child,
    required ViewportOffset position,
    required ScrollCallback onScroll,
  })  : _position = position,
        _scrollCallback = onScroll,
        super(child);

  ViewportOffset _position;
  set position(ViewportOffset value) {
    if (_position == value) return;
    if (attached) _position.removeListener(markNeedsLayout);
    _position = value;
    if (attached) _position.addListener(markNeedsLayout);
    markNeedsLayout();
  }

  ScrollCallback _scrollCallback;
  set onScroll(ScrollCallback value) {
    if (_scrollCallback == value) return;
    _scrollCallback = value;
    markNeedsLayout();
  }

  void _onScroll() {
    _scrollCallback(_position.pixels);
  }

  @override
  void attach(covariant PipelineOwner owner) {
    super.attach(owner);
    _position.addListener(_onScroll);
  }

  @override
  void detach() {
    super.detach();
    _position.removeListener(_onScroll);
  }

  @override
  void performLayout() {
    child?.layout(constraints, parentUsesSize: true);
    size = child?.size ?? Size.zero;
    _position.applyViewportDimension(size.height);
    _position.applyContentDimensions(double.negativeInfinity, double.infinity);
  }
}
