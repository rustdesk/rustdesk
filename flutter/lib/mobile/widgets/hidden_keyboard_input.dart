import 'package:flutter/widgets.dart';

class HiddenKeyboardInput extends StatelessWidget {
  const HiddenKeyboardInput({
    super.key,
    required this.isIOS,
    required this.child,
  });

  final bool isIOS;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    // iOS floating-cursor bounds need a laid-out line. Offstage keeps keyboard
    // focus without painting the field or constraining its height to zero.
    if (isIOS) return Offstage(child: child);
    return SizedBox(width: 0, height: 0, child: child);
  }
}
