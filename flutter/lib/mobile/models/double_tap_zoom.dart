import 'dart:math';

import 'package:flutter/animation.dart';

AnimationController animateDoubleTapZoom(
    TickerProvider vsync, void Function(double) onFrame) {
  final controller = AnimationController(
      vsync: vsync, duration: const Duration(milliseconds: 230));
  controller.addListener(() {
    onFrame(Curves.easeInOutCubic.transform(controller.value));
  });
  return controller..forward();
}

const kOptionZoomStrength = 'double-tap-zoom-percent';

double zoomStrengthFromOption(String value) =>
    (int.tryParse(value) ?? 125).clamp(25, 250) / 100;

double nextDoubleTapZoomScale(double current, double fit,
    {required double nativeScale, required double amount}) {
  final target = nativeScale * max(1.0, amount);
  if ((current - target).abs() <= target * 0.00001) return fit;
  if (amount >= 1) return target;
  final step = (target - fit) * amount;
  if (step > 0 && current < target) return min(current + step, target);
  if (step < 0 && current > target) return max(current + step, target);
  return fit;
}
