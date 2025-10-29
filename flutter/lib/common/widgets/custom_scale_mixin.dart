import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/scale.dart';
import 'package:flutter_hbb/common.dart';

/// Mixin providing shared custom scale control logic for both mobile and desktop widgets.
/// Implementations must provide [ffi] and [onScaleChanged] getters.
///
/// Note: Members are intentionally not private (no underscore) to allow access from
/// implementing classes in different libraries. Implementing classes should treat
/// these as protected members.
mixin CustomScaleControlsMixin<T extends StatefulWidget> on State<T> {
  /// FFI instance for session interaction
  FFI get ffi;

  /// Callback invoked when scale value changes
  ValueChanged<int>? get onScaleChanged;

  late int scaleValue;
  late final Debouncer<int> debouncerScale;
  // Normalized slider position in [0, 1]. We map it nonlinearly to percent.
  double scalePos = 0.0;

  // Piecewise mapping constants (from consts.dart)
  static const int minPercent = kScaleCustomMinPercent;
  static const int pivotPercent = kScaleCustomPivotPercent; // 100% should be at 1/3 of track
  static const int maxPercent = kScaleCustomMaxPercent;
  static const double pivotPos = kScaleCustomPivotPos; // first 1/3 → up to 100%
  static const double detentEpsilon = kScaleCustomDetentEpsilon; // snap range around pivot (~0.6%)

  // Clamp helper for local use
  int clampScale(int v) => clampCustomScalePercent(v);

  // Map normalized position [0,1] → percent [5,1000] with 100 at 1/3 width.
  int mapPosToPercent(double p) {
    if (p <= 0.0) return minPercent;
    if (p >= 1.0) return maxPercent;
    if (p <= pivotPos) {
      final q = p / pivotPos; // 0..1
      final v = minPercent + q * (pivotPercent - minPercent);
      return clampScale(v.round());
    } else {
      final q = (p - pivotPos) / (1.0 - pivotPos); // 0..1
      final v = pivotPercent + q * (maxPercent - pivotPercent);
      return clampScale(v.round());
    }
  }

  // Map percent [5,1000] → normalized position [0,1]
  double mapPercentToPos(int percent) {
    final p = clampScale(percent);
    if (p <= pivotPercent) {
      final q = (p - minPercent) / (pivotPercent - minPercent);
      return q * pivotPos;
    } else {
      final q = (p - pivotPercent) / (maxPercent - pivotPercent);
      return pivotPos + q * (1.0 - pivotPos);
    }
  }

  // Snap normalized position to the pivot when close to it
  double snapNormalizedPos(double p) {
    if ((p - pivotPos).abs() <= detentEpsilon) return pivotPos;
    if (p < 0.0) return 0.0;
    if (p > 1.0) return 1.0;
    return p;
  }

  @override
  void initState() {
    super.initState();
    scaleValue = 100;
    debouncerScale = Debouncer<int>(
      kDebounceCustomScaleDuration,
      onChanged: (v) async {
        await applyScale(v);
      },
      initialValue: scaleValue,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      try {
        final v = await getSessionCustomScalePercent(ffi.sessionId);
        if (mounted) {
          setState(() {
            scaleValue = v;
            scalePos = mapPercentToPos(v);
          });
        }
      } catch (e, st) {
        debugPrint('[CustomScale] Failed to get initial value: $e');
        debugPrintStack(stackTrace: st);
      }
    });
  }

  Future<void> applyScale(int v) async {
    v = clampCustomScalePercent(v);
    setState(() {
      scaleValue = v;
    });
    try {
      await bind.sessionSetFlutterOption(
          sessionId: ffi.sessionId,
          k: kCustomScalePercentKey,
          v: v.toString());
      final curStyle = await bind.sessionGetViewStyle(sessionId: ffi.sessionId);
      if (curStyle != kRemoteViewStyleCustom) {
        await bind.sessionSetViewStyle(
            sessionId: ffi.sessionId, value: kRemoteViewStyleCustom);
      }
      await ffi.canvasModel.updateViewStyle();
      if (isMobile) {
        HapticFeedback.selectionClick();
      }
      onScaleChanged?.call(v);
    } catch (e, st) {
      debugPrint('[CustomScale] Apply failed: $e');
      debugPrintStack(stackTrace: st);
    }
  }

  void nudgeScale(int delta) {
    final next = clampScale(scaleValue + delta);
    setState(() {
      scaleValue = next;
      scalePos = mapPercentToPos(next);
    });
    onScaleChanged?.call(next);
    debouncerScale.value = next;
  }

  @override
  void dispose() {
    debouncerScale.cancel();
    super.dispose();
  }

  // Callback for slider changes - implementations can call this from their build method
  void onSliderChanged(double v) {
    final snapped = snapNormalizedPos(v);
    final next = mapPosToPercent(snapped);
    if (next != scaleValue || snapped != scalePos) {
      setState(() {
        scalePos = snapped;
        scaleValue = next;
      });
      onScaleChanged?.call(next);
      debouncerScale.value = next;
    }
  }
}
