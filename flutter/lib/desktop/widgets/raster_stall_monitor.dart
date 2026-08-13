import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/scheduler.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';
import '../../models/state_model.dart';

/// Records a hung raster thread (frames continuously scheduled but no frame
/// timings delivered for 30s) in `texture-render-health`; a hang cannot be
/// rescued in-process, so the next launch defaults texture rendering off.
class RasterStallMonitor {
  static bool _started = false;
  static bool _reported = false;
  static DateTime? _lastTimings;
  static DateTime _lastQuiet = DateTime.now();

  static void start() {
    if (_started || isWeb) return;
    _started = true;
    SchedulerBinding.instance.addTimingsCallback((_) {
      _lastTimings = DateTime.now();
    });
    Timer.periodic(const Duration(seconds: 2), (_) {
      if (_reported) return;
      final now = DateTime.now();
      final lifecycle = SchedulerBinding.instance.lifecycleState;
      // Minimized/inactive (incl. screen lock) or idle (nothing scheduled):
      // no timings is legitimate, keep moving the quiet anchor forward.
      if (stateGlobal.isMinimized ||
          (lifecycle != null && lifecycle != AppLifecycleState.resumed) ||
          !SchedulerBinding.instance.hasScheduledFrame) {
        _lastQuiet = now;
        return;
      }
      var ref = _lastQuiet;
      final lastTimings = _lastTimings;
      if (lastTimings != null && lastTimings.isAfter(ref)) {
        ref = lastTimings;
      }
      if (now.difference(ref) > const Duration(seconds: 30)) {
        _reported = true;
        bind.mainSetLocalOption(
            key: kOptionTextureRenderHealth, value: 'failed-raster-stall');
        debugPrint(
            'raster thread stall detected, texture rendering disabled for next launch');
      }
    });
  }
}
