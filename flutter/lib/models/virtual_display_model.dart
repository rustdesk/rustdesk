import 'dart:async';
import 'dart:convert';
import 'package:uuid/uuid.dart';

class VirtualDisplayError implements Exception {
  final String message;
  const VirtualDisplayError(this.message);
  @override
  String toString() => message;
}

class VirtualDisplayRequests {
  static final _pending = <(String, String), Completer<Map<String, dynamic>>>{};

  static Future<void> request(
      String session, Future<void> Function(String requestId) send,
      {required int displayId,
      required int width,
      required int height,
      required int scale,
      Duration timeout = const Duration(seconds: 30)}) async {
    final id = const Uuid().v4();
    final completer = Completer<Map<String, dynamic>>();
    final key = (session, id);
    _pending[key] = completer;
    final response = completer.future.timeout(timeout,
        onTimeout: () => throw const VirtualDisplayError(
            'Display settings timed out. Refresh and try again.'));
    try {
      unawaited(Future<void>.sync(() => send(id)).catchError((Object error) {
        if (!completer.isCompleted) completer.completeError(error);
      }));
      final state = await response;
      if (state['display_id'] is! int ||
          state['width'] is! int ||
          state['height'] is! int ||
          state['scale'] is! int ||
          state['display_id'] != displayId ||
          state['width'] != width ||
          state['height'] != height ||
          state['scale'] != scale) {
        throw const VirtualDisplayError(
            'Failed to resize macOS virtual display');
      }
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
    if (response is! Map<String, dynamic> ||
        response['request_id'] is! String) {
      return;
    }
    final pending = _pending[(session, response['request_id'] as String)];
    if (pending == null || pending.isCompleted) return;
    if (response['error'] is String) {
      pending.completeError(VirtualDisplayError(response['error']));
    } else {
      pending.complete(response);
    }
  }
}
