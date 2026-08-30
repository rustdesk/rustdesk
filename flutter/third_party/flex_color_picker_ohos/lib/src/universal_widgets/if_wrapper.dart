// ignore_for_file: use_super_parameters

import 'package:flutter/material.dart';

/// A potential Widget builder function.
typedef IfWrapBuilder = Widget Function(BuildContext context, Widget child);

/// A builder that if the condition is true, will run its builder and the child
/// will be wrapped by the builder, if false it just returns the child.
///
/// A way to wrap a widget with another widget, but only if
/// the condition is true. It can save you from having to define a large widget,
/// assign it to a Widget, and then wrap it with another widget if some
/// condition is true. With the IfWrapper you can do this directly in the
/// Widget tree.
///
/// Widget widgetA = WidgetX(...);
/// if (condition) widgetA = WidgetY(child: widgetA);
///
/// Becomes:
///
/// IfWrapper(condition: condition,
///   builder: (context, child) {
///     return WidgetY(child: child); },
///   child: child
/// );
///
/// Not library exposed, private to the library.
@immutable
class IfWrapper extends StatelessWidget {
  /// Default const constructor.
  const IfWrapper({
    Key? key,
    required this.condition,
    required this.builder,
    required this.child,
    this.ifFalse,
  }) : super(key: key);

  /// if [condition] evaluates to true, the builder will be run and the child
  /// will be wrapped. If false, only the child is returned.
  final bool condition;

  /// How to display the widget if [condition] is true;
  final IfWrapBuilder builder;

  /// How to display the widget if [condition] is false;
  final IfWrapBuilder? ifFalse;

  /// The widget to be conditionally wrapped. This will be displayed alone
  /// if [condition] is false.
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (condition) {
      return builder(context, child);
    } else if (ifFalse != null) {
      return ifFalse!(context, child);
    } else {
      return child;
    }
  }
}
