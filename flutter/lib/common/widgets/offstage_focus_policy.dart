import 'package:flutter/rendering.dart';
import 'package:flutter/widgets.dart';

/// Reading order traversal that skips focus nodes nobody can see: those inside
/// a hidden [Offstage], and those on kept-alive pages or list items that are
/// scrolled out of their viewport (e.g. settings pages in a [PageView]).
///
/// Such subtrees are not painted and have no semantics, but their focus nodes
/// stay in the tab order. Tab then lands on controls nobody can see and screen
/// readers announce nothing, so keyboard users appear to be stuck.
class SkipOffstageTraversalPolicy extends ReadingOrderTraversalPolicy {
  SkipOffstageTraversalPolicy({super.requestFocusCallback});

  @override
  Iterable<FocusNode> sortDescendants(
      Iterable<FocusNode> descendants, FocusNode currentNode) {
    // Keep the current node so traversal can always move away from it.
    return super.sortDescendants(
        descendants.where((node) => node == currentNode || !_isHidden(node)),
        currentNode);
  }

  static bool _isHidden(FocusNode node) {
    RenderObject? renderObject = node.context?.findRenderObject();
    while (renderObject != null) {
      if (renderObject is RenderOffstage && renderObject.offstage) {
        return true;
      }
      final parentData = renderObject.parentData;
      if (parentData is SliverMultiBoxAdaptorParentData &&
          parentData.keptAlive) {
        return true;
      }
      renderObject = renderObject.parent;
    }
    return false;
  }
}
