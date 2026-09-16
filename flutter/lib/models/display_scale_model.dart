import 'dart:async';
import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:uuid/uuid.dart';

String formatDisplayScale(double value) =>
    value.toStringAsFixed(2).replaceFirst(RegExp(r'\.?0+$'), '');

class DisplayScaleState {
  final double percent;
  final double? recommended;
  final List<double> options;
  final (double, double, double)? custom;
  final String token;
  final String identity;
  final (int, int) resolution;

  const DisplayScaleState(
      {required this.percent,
      this.recommended,
      this.custom,
      required this.options,
      required this.token,
      required this.identity,
      required this.resolution});

  double? nearest(double value) {
    final range = custom;
    if (!value.isFinite ||
        range == null ||
        value < range.$1 ||
        value > range.$2) return null;
    final tick = (value / range.$3)
        .round()
        .clamp((range.$1 / range.$3).ceil(), (range.$2 / range.$3).floor());
    return tick * range.$3;
  }

  double? supported(double value) {
    for (final option in options) {
      if ((option - value).abs() < 0.000001) return option;
    }
    final candidate = nearest(value);
    return candidate != null && (candidate - value).abs() < 0.000001
        ? candidate
        : null;
  }

  double? adjacent(double value, int direction) {
    final range = custom;
    if (range == null || !value.isFinite) return null;
    final position = value / range.$3;
    final tick = direction > 0
        ? (position + 0.000001).floor() + 1
        : (position - 0.000001).ceil() - 1;
    final candidate = tick * range.$3;
    return candidate >= range.$1 - 0.000001 && candidate <= range.$2 + 0.000001
        ? candidate
        : null;
  }

  factory DisplayScaleState.fromJson(dynamic data) {
    if (data is! Map ||
        data['percent'] is! num ||
        data['options'] is! List ||
        data['token'] is! String ||
        (data['token'] as String).isEmpty ||
        (data['token'] as String).length > 64 ||
        data['identity'] is! String ||
        (data['identity'] as String).isEmpty ||
        (data['identity'] as String).length > 64 ||
        data['resolution'] is! List ||
        (data['resolution'] as List).length != 2 ||
        (data['resolution'] as List)
            .any((v) => v is! int || v <= 0 || v > 2147483647)) {
      throw const FormatException('Invalid display scaling response');
    }
    final options = data['options'] as List;
    final recommended = data['recommended'];
    if (options.isEmpty ||
        options.length > 64 ||
        options.any((p) => p is! num || !p.isFinite || p < 50 || p > 500) ||
        !options.contains(data['percent']) ||
        (recommended != null &&
            (recommended is! num || !options.contains(recommended)))) {
      throw const FormatException('Invalid display scaling levels');
    }
    (double, double, double)? custom;
    final range = data['custom'];
    if (range != null) {
      if (range is! List ||
          range.length != 3 ||
          range.any((v) => v is! num || !v.isFinite) ||
          range[0] < 50 ||
          range[1] > 500 ||
          range[0] >= range[1] ||
          range[2] < 0.000001 ||
          range[2] > range[1] - range[0]) {
        throw const FormatException('Invalid display scaling range');
      }
      custom = (
        (range[0] as num).toDouble(),
        (range[1] as num).toDouble(),
        (range[2] as num).toDouble()
      );
    }
    return DisplayScaleState(
        percent: (data['percent'] as num).toDouble(),
        recommended: (recommended as num?)?.toDouble(),
        custom: custom,
        options: options.map((p) => (p as num).toDouble()).toSet().toList()
          ..sort(),
        token: data['token'],
        identity: data['identity'],
        resolution: (data['resolution'][0], data['resolution'][1]));
  }
}

class DisplayScaleError implements Exception {
  final String message;
  final String? code;
  const DisplayScaleError(this.message, {this.code});
  @override
  String toString() => message;
}

class DisplaySettingsReopenRequired extends DisplayScaleError {
  final Object cause;

  const DisplaySettingsReopenRequired(this.cause)
      : super(
            'Display settings changed. Reopen the resolution menu and try again.');
}

enum _DisplayScaleRecovery { none, refresh, reopen }

// Responses are scoped to both the local UI session and a unique request.
class DisplayScaleRequests {
  static final _pending = <(String, String), Completer<DisplayScaleState>>{};

  static Future<DisplayScaleState> request(
      String session, Future<void> Function(String requestId) send,
      {Duration timeout = const Duration(seconds: 30)}) async {
    final id = const Uuid().v4();
    final completer = Completer<DisplayScaleState>();
    final key = (session, id);
    _pending[key] = completer;
    // Attach the timeout/error handler before sending; a peer can reply immediately.
    final response = completer.future.timeout(timeout,
        onTimeout: () => throw const DisplayScaleError(
            'Display settings timed out. Refresh and try again.'));
    try {
      unawaited(Future<void>.sync(() => send(id)).catchError((Object error) {
        if (!completer.isCompleted) completer.completeError(error);
      }));
      return await response;
    } finally {
      _pending.remove(key);
    }
  }

  static void handle(String session, String data) {
    dynamic response;
    try {
      response = jsonDecode(data);
    } catch (_) {
      return;
    }
    if (response is! Map || response['request_id'] is! String) return;
    final pending = _pending[(session, response['request_id'] as String)];
    if (pending == null || pending.isCompleted) return;
    try {
      if (response['error'] is String) {
        throw DisplayScaleError(response['error'],
            code: response['code'] is String ? response['code'] : null);
      }
      pending.complete(DisplayScaleState.fromJson(response['state']));
    } catch (error) {
      pending.completeError(error);
    }
  }
}

class DisplayScaleModel extends ChangeNotifier {
  final Future<DisplayScaleState> Function(double percent, String token)
      request;
  DisplayScaleState? current;
  double? percent;
  bool busy = false;
  bool valid = true;
  bool customMode = false;
  bool unavailable = false;
  String draftText = '';

  double? suggestion;
  String? error;
  bool _disposed = false;
  bool _pendingChange = false;
  _DisplayScaleRecovery _recovery = _DisplayScaleRecovery.none;

  DisplayScaleModel(this.request);

  bool get needsRefresh => _recovery == _DisplayScaleRecovery.refresh;
  bool get needsReopen => _recovery == _DisplayScaleRecovery.reopen;
  bool get canApply => valid && _recovery == _DisplayScaleRecovery.none;

  bool get changed =>
      current != null && (_pendingChange || percent != current!.percent);

  void select(double value) {
    if (busy || needsReopen || current?.options.contains(value) != true) return;
    customMode = false;
    valid = true;
    suggestion = null;
    percent = value;
    draftText = formatDisplayScale(value);
    notifyListeners();
  }

  void useCustom() {
    if (busy || needsReopen || current?.custom == null) return;
    customMode = true;
    notifyListeners();
  }

  void edit(String text) {
    if (busy || needsReopen || current == null || !customMode) return;
    draftText = text;
    final input = text.trim().replaceAll(',', '.');
    final value = RegExp(r'^\d+(?:\.\d+)?$').hasMatch(input)
        ? double.tryParse(input)
        : null;
    final accepted = value == null ? null : current!.supported(value);
    valid = accepted != null;
    suggestion =
        accepted == null && value != null ? current!.nearest(value) : null;
    if (accepted != null) percent = accepted;
    notifyListeners();
  }

  void acceptSuggestion() {
    final value = suggestion;
    if (busy || needsReopen || value == null) return;
    percent = value;
    valid = true;
    suggestion = null;
    draftText = formatDisplayScale(percent!);
    notifyListeners();
  }

  void step(int direction) {
    if (busy || needsReopen || !valid || percent == null) return;
    final value = current?.adjacent(percent!, direction);
    if (value == null) return;
    percent = value;
    suggestion = null;
    draftText = formatDisplayScale(percent!);
    notifyListeners();
  }

  void reset() {
    if (busy || needsReopen || current == null) return;
    _pendingChange = false;
    select(current!.percent);
  }

  Future<bool> refresh() => _request(false);
  Future<bool> apply() => _request(true);

  void invalidate(Object failure) {
    if (_disposed || needsReopen) return;
    if (failure is DisplaySettingsReopenRequired) {
      debugPrint('Display resolution confirmation failed: ${failure.cause}');
    }
    unavailable = current == null &&
        failure is DisplayScaleError &&
        failure.code == 'unsupported';
    error = unavailable
        ? null
        : failure is FormatException
            ? failure.message
            : failure.toString();
    _recovery = failure is DisplaySettingsReopenRequired
        ? _DisplayScaleRecovery.reopen
        : current != null
            ? _DisplayScaleRecovery.refresh
            : _DisplayScaleRecovery.none;
    notifyListeners();
  }

  void acceptConfirmedResolution(DisplayScaleState state) {
    if (_disposed || busy || needsReopen) return;
    _acceptState(state, applied: false);
    notifyListeners();
  }

  void _acceptState(DisplayScaleState state, {required bool applied}) {
    final preserveDraft =
        !applied && current != null && (changed || customMode || !valid);
    _pendingChange = !applied && changed;
    _recovery = _DisplayScaleRecovery.none;
    unavailable = false;
    error = null;
    current = state;
    if (preserveDraft) {
      if (customMode && !valid) {
        final input = draftText.trim().replaceAll(',', '.');
        final value = RegExp(r'^\d+(?:\.\d+)?$').hasMatch(input)
            ? double.tryParse(input)
            : null;
        final accepted = value == null ? null : state.supported(value);
        valid = accepted != null;
        suggestion = value == null || valid ? null : state.nearest(value);
        if (valid) percent = accepted;
      } else {
        valid = percent != null && state.supported(percent!) != null;
        suggestion = valid || percent == null ? null : state.nearest(percent!);
      }
      if (valid) {
        customMode = state.custom != null &&
            (customMode || !state.options.contains(percent));
      }
    } else {
      valid = true;
      suggestion = null;
      customMode = customMode && state.custom != null;
      percent = state.percent;
      draftText = formatDisplayScale(state.percent);
    }
  }

  Future<bool> _request(bool apply) async {
    if (_disposed ||
        busy ||
        needsReopen ||
        (apply && (!canApply || !changed))) {
      return false;
    }
    // A mode can cap the native scale to the draft without applying the user's
    // choice. Only an acknowledged apply or Reset clears that pending choice.
    if (!apply) _pendingChange = changed;
    busy = true;
    error = null;
    notifyListeners();
    try {
      final requested = apply ? percent! : 0.0;
      final state = await request(requested, apply ? current!.token : '');
      if (apply && (state.percent - requested).abs() > 0.000001) {
        throw const DisplayScaleError(
            'The system did not apply the requested scale. Refresh and try again.');
      }
      if (_disposed || needsReopen) return false;
      _acceptState(state, applied: apply);
      return true;
    } catch (failure) {
      invalidate(failure);
      return false;
    } finally {
      if (!_disposed) {
        busy = false;
        notifyListeners();
      }
    }
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
