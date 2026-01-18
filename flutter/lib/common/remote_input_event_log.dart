import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';

class RemoteInputEvent {
  final int tsMs;
  final String type;
  final Map<String, Object?> data;

  RemoteInputEvent({
    required this.tsMs,
    required this.type,
    required this.data,
  });

  String toLine() {
    final jsonData = data.isEmpty ? '' : ' ${jsonEncode(data)}';
    return '$tsMs $type$jsonData';
  }
}

class RemoteInputEventLog {
  static const int _maxEvents = 80;
  static final ValueNotifier<int> revision = ValueNotifier<int>(0);
  static final List<RemoteInputEvent> _events = <RemoteInputEvent>[];

  static bool get isEnabled =>
      isAndroid &&
      kDebugMode &&
      mainGetLocalBoolOptionSync(kOptionEnableAndroidE2eMode);

  static void clear() {
    if (!isEnabled) return;
    _events.clear();
    revision.value++;
  }

  static void add(String type, {Map<String, Object?> data = const {}}) {
    if (!isEnabled) return;
    final tsMs = DateTime.now().millisecondsSinceEpoch;
    _events.add(RemoteInputEvent(tsMs: tsMs, type: type, data: data));
    if (_events.length > _maxEvents) {
      _events.removeRange(0, _events.length - _maxEvents);
    }
    revision.value++;
  }

  static List<RemoteInputEvent> snapshot({int? lastN}) {
    final list = List<RemoteInputEvent>.unmodifiable(_events);
    if (lastN == null || lastN <= 0 || lastN >= list.length) return list;
    return List<RemoteInputEvent>.unmodifiable(
        list.sublist(list.length - lastN));
  }

  static String dumpText({int lastN = 40}) {
    final list = snapshot(lastN: lastN);
    return list.map((e) => e.toLine()).join('\n');
  }
}
