import 'package:flutter/widgets.dart';
import 'package:flutter_hbb/consts.dart';
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
      enableCustomMouseWheelScrolling: true,
      customMouseWheelScrollConfig: CustomMouseWheelScrollConfig(
          scrollDuration: kDefaultScrollDuration,
          scrollCurve: Curves.linearToEaseOut,
          mouseWheelTurnsThrottleTimeMs:
              kDefaultMouseWheelThrottleDuration.inMilliseconds,
          scrollAmountMultiplier: kDefaultScrollAmountMultiplier),
      child: child,
    );
  }
}
