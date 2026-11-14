import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';

/// On Android, lets the up and down arrows (the D-pad of a TV remote) move the
/// focus out of a single-line text field to the widget above or below it.
///
/// A focused text field otherwise keeps these keys, so a TV remote cannot
/// reach the next field or the dialog buttons. Multi-line text fields keep
/// the keys, because there they move the cursor between lines.
///
/// Wrap the app's navigator with this, so it covers every page and dialog.
class DpadTextFieldTraversal extends StatelessWidget {
  const DpadTextFieldTraversal({Key? key, required this.child})
      : super(key: key);

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Shortcuts(
      shortcuts: const <ShortcutActivator, Intent>{
        SingleActivator(LogicalKeyboardKey.arrowUp):
            _LeaveTextFieldIntent(TraversalDirection.up),
        SingleActivator(LogicalKeyboardKey.arrowDown):
            _LeaveTextFieldIntent(TraversalDirection.down),
      },
      child: Actions(
        actions: <Type, Action<Intent>>{
          _LeaveTextFieldIntent: _LeaveTextFieldAction(),
        },
        child: child,
      ),
    );
  }
}

class _LeaveTextFieldIntent extends Intent {
  const _LeaveTextFieldIntent(this.direction);

  final TraversalDirection direction;
}

class _LeaveTextFieldAction extends Action<_LeaveTextFieldIntent> {
  EditableText? _focusedField() =>
      primaryFocus?.context?.findAncestorWidgetOfExactType<EditableText>();

  // When disabled, the key goes on to the default handlers: text editing in a
  // multi-line field, or normal focus traversal outside text fields.
  @override
  bool isEnabled(_LeaveTextFieldIntent intent) =>
      _focusedField()?.maxLines == 1;

  @override
  Object? invoke(_LeaveTextFieldIntent intent) {
    primaryFocus?.focusInDirection(intent.direction);
    return null;
  }
}
