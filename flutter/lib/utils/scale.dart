import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/display_windows.dart';
import 'package:uuid/uuid.dart';

/// Clamp custom scale percent to supported bounds.
/// Keep this in sync with the slider's minimum in the desktop toolbar UI.
///
/// This function exists to ensure consistent clamping behavior across the app
/// and to provide a single point of reference for the valid scale range.
int clampCustomScalePercent(int percent) {
  return percent.clamp(kScaleCustomMinPercent, kScaleCustomMaxPercent);
}

/// Parse a string percent and clamp. Defaults to 100 when invalid.
int parseCustomScalePercent(String? s, {int defaultPercent = 100}) {
  final parsed = int.tryParse(s ?? '') ?? defaultPercent;
  return clampCustomScalePercent(parsed);
}

/// Convert a percent value to scale factor after clamping.
double percentToScale(int percent) => clampCustomScalePercent(percent) / 100.0;

/// Per-window view state, keyed by `sessionId.toString()` (each remote window is its own session).
///
/// The peer option behind `bind.sessionGetViewStyle()` / `kCustomScalePercentKey` is shared by every window of the
/// peer. A window's own value MUST win over it: `initWindowView()` (`display_windows.dart`) pins it when the window's
/// session gets its peer info, and every change goes through `setSessionViewStyle()` /
/// `setSessionCustomScalePercent()`, which also update the peer option (the default of a window without a stored
/// state).
final Map<String, String> windowViewStyles = {};
final Map<String, int> windowCustomScalePercents = {};

Future<String?> getSessionViewStyle(UuidValue sessionId) async =>
    windowViewStyles[sessionId.toString()] ??
    await bind.sessionGetViewStyle(sessionId: sessionId);

Future<void> setSessionViewStyle(UuidValue sessionId, String value) async {
  windowViewStyles[sessionId.toString()] = value;
  await bind.sessionSetViewStyle(sessionId: sessionId, value: value);
  reportDisplayWindowSoon();
}

/// Fetch, parse and clamp the custom scale percent for a session.
Future<int> getSessionCustomScalePercent(UuidValue sessionId) async {
  final own = windowCustomScalePercents[sessionId.toString()];
  if (own != null) {
    return clampCustomScalePercent(own);
  }
  final opt = await bind.sessionGetFlutterOption(
      sessionId: sessionId, k: kCustomScalePercentKey);
  return parseCustomScalePercent(opt);
}

Future<void> setSessionCustomScalePercent(UuidValue sessionId, int percent) async {
  percent = clampCustomScalePercent(percent);
  windowCustomScalePercents[sessionId.toString()] = percent;
  await bind.sessionSetFlutterOption(
      sessionId: sessionId, k: kCustomScalePercentKey, v: percent.toString());
  reportDisplayWindowSoon();
}

/// Fetch and compute the custom scale factor for a session.
Future<double> getSessionCustomScale(UuidValue sessionId) async {
  final p = await getSessionCustomScalePercent(sessionId);
  return percentToScale(p);
}
