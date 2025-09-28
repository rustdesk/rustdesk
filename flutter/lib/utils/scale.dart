import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import 'package:flutter_hbb/generated_bridge.dart'
    if (dart.library.html) 'package:flutter_hbb/web/bridge.dart';
import 'package:uuid/uuid.dart';

/// Clamp custom scale percent to supported bounds.
/// Keep this in sync with the slider's minimum in the desktop toolbar UI.
int clampCustomScalePercent(int percent) {
  if (percent < kScaleCustomMinPercent) return kScaleCustomMinPercent;
  if (percent > kScaleCustomMaxPercent) return kScaleCustomMaxPercent;
  return percent;
}

/// Parse a string percent and clamp. Defaults to 100 when invalid.
int parseCustomScalePercent(String? s, {int defaultPercent = 100}) {
  final parsed = int.tryParse(s ?? '') ?? defaultPercent;
  return clampCustomScalePercent(parsed);
}

/// Convert a percent value to scale factor after clamping.
double percentToScale(int percent) => clampCustomScalePercent(percent) / 100.0;

/// Fetch, parse and clamp the custom scale percent for a session.
Future<int> getSessionCustomScalePercent(UuidValue sessionId) async {
  final opt = await bind.sessionGetFlutterOption(
      sessionId: sessionId, k: kCustomScalePercentKey);
  return parseCustomScalePercent(opt);
}

/// Fetch and compute the custom scale factor for a session.
Future<double> getSessionCustomScale(UuidValue sessionId) async {
  final p = await getSessionCustomScalePercent(sessionId);
  return percentToScale(p);
}
