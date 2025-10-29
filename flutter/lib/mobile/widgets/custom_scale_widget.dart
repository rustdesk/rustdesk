import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:debounce_throttle/debounce_throttle.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/scale.dart';
import 'package:flutter_hbb/common.dart';

class MobileCustomScaleControls extends StatefulWidget {
  final FFI ffi;
  final ValueChanged<int>? onChanged;
  const MobileCustomScaleControls({Key? key, required this.ffi, this.onChanged}) : super(key: key);

  @override
  State<MobileCustomScaleControls> createState() => _MobileCustomScaleControlsState();
}

class _MobileCustomScaleControlsState extends State<MobileCustomScaleControls> {
  late int _value;
  late final Debouncer<int> _debouncerScale;
  // Normalized slider position in [0, 1]. We map it nonlinearly to percent.
  double _pos = 0.0;

  // Piecewise mapping constants (from consts.dart)
  static const int _minPercent = kScaleCustomMinPercent;
  static const int _pivotPercent = kScaleCustomPivotPercent; // 100% should be at 1/3 of track
  static const int _maxPercent = kScaleCustomMaxPercent;
  static const double _pivotPos = kScaleCustomPivotPos; // first 1/3 → up to 100%
  static const double _detentEpsilon = kScaleCustomDetentEpsilon; // snap range around pivot (~0.6%)

  // Clamp helper for local use
  int _clamp(int v) => clampCustomScalePercent(v);

  // Map normalized position [0,1] → percent [5,1000] with 100 at 1/3 width.
  int _mapPosToPercent(double p) {
    if (p <= 0.0) return _minPercent;
    if (p >= 1.0) return _maxPercent;
    if (p <= _pivotPos) {
      final q = p / _pivotPos; // 0..1
      final v = _minPercent + q * (_pivotPercent - _minPercent);
      return _clamp(v.round());
    } else {
      final q = (p - _pivotPos) / (1.0 - _pivotPos); // 0..1
      final v = _pivotPercent + q * (_maxPercent - _pivotPercent);
      return _clamp(v.round());
    }
  }

  // Map percent [5,1000] → normalized position [0,1]
  double _mapPercentToPos(int percent) {
    final p = _clamp(percent);
    if (p <= _pivotPercent) {
      final q = (p - _minPercent) / (_pivotPercent - _minPercent);
      return q * _pivotPos;
    } else {
      final q = (p - _pivotPercent) / (_maxPercent - _pivotPercent);
      return _pivotPos + q * (1.0 - _pivotPos);
    }
  }

  // Snap normalized position to the pivot when close to it
  double _snapNormalizedPos(double p) {
    if ((p - _pivotPos).abs() <= _detentEpsilon) return _pivotPos;
    if (p < 0.0) return 0.0;
    if (p > 1.0) return 1.0;
    return p;
  }

  @override
  void initState() {
    super.initState();
    _value = 100;
    _debouncerScale = Debouncer<int>(
      kDebounceCustomScaleDuration,
      onChanged: (v) async {
        await _apply(v);
      },
      initialValue: _value,
    );
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      try {
        final v = await getSessionCustomScalePercent(widget.ffi.sessionId);
        if (mounted) {
          setState(() {
            _value = v;
            _pos = _mapPercentToPos(v);
          });
        }
      } catch (e, st) {
        debugPrint('[CustomScale] Failed to get initial value: $e');
        debugPrintStack(stackTrace: st);
      }
    });
  }

  Future<void> _apply(int v) async {
    v = clampCustomScalePercent(v);
    setState(() {
      _value = v;
    });
    try {
      await bind.sessionSetFlutterOption(
          sessionId: widget.ffi.sessionId,
          k: kCustomScalePercentKey,
          v: v.toString());
      final curStyle = await bind.sessionGetViewStyle(sessionId: widget.ffi.sessionId);
      if (curStyle != kRemoteViewStyleCustom) {
        await bind.sessionSetViewStyle(
            sessionId: widget.ffi.sessionId, value: kRemoteViewStyleCustom);
      }
      await widget.ffi.canvasModel.updateViewStyle();
      if (isMobile) {
        HapticFeedback.selectionClick();
      }
      widget.onChanged?.call(v);
    } catch (e, st) {
      debugPrint('[CustomScale] Apply failed: $e');
      debugPrintStack(stackTrace: st);
    }
  }

  void _nudge(int delta) {
    final next = _clamp(_value + delta);
    setState(() {
      _value = next;
      _pos = _mapPercentToPos(next);
    });
    widget.onChanged?.call(next);
    _debouncerScale.value = next;
  }

  @override
  void dispose() {
    _debouncerScale.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Smaller button size for mobile
    const smallBtnConstraints = BoxConstraints(minWidth: 32, minHeight: 32);

    final sliderControl = Slider(
      value: _pos,
      min: 0.0,
      max: 1.0,
      divisions: (_maxPercent - _minPercent).round(),
      label: '$_value%',
      onChanged: (v) {
        final snapped = _snapNormalizedPos(v);
        final next = _mapPosToPercent(snapped);
        if (next != _value || snapped != _pos) {
          setState(() {
            _pos = snapped;
            _value = next;
          });
          widget.onChanged?.call(next);
          _debouncerScale.value = next;
        }
      },
    );

    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8.0, vertical: 8.0),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            '${translate("Scale custom")}: $_value%',
            style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
          ),
          const SizedBox(height: 8),
          Row(
            children: [
              IconButton(
                iconSize: 20,
                padding: const EdgeInsets.all(4),
                constraints: smallBtnConstraints,
                icon: const Icon(Icons.remove),
                tooltip: translate('Decrease'),
                onPressed: () => _nudge(-1),
              ),
              Expanded(child: sliderControl),
              IconButton(
                iconSize: 20,
                padding: EdgeInsets.all(4),
                constraints: smallBtnConstraints,
                icon: const Icon(Icons.add),
                tooltip: translate('Increase'),
                onPressed: () => _nudge(1),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
