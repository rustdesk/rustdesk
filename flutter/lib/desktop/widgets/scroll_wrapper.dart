import 'package:flutter/widgets.dart';
import 'package:flutter_improved_scrolling/flutter_improved_scrolling.dart';

class DesktopScrollWrapper extends StatelessWidget {
  final ScrollController scrollController;
  final Widget child;
  const DesktopScrollWrapper(
      {Key? key, required this.scrollController, required this.child})
      : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ImprovedScrolling(
      scrollController: scrollController,
      enableCustomMouseWheelScrolling: false,
      customMouseWheelScrollConfig:
          const CustomMouseWheelScrollConfig(scrollAmountMultiplier: 3.0),
      child: child,
    );
  }
}
