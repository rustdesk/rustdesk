import 'dart:async';

import 'mcp_dispatcher.dart';

class McpOperationQueue {
  Future<void> _tail = Future.value();

  Future<T> run<T>(Future<T> Function() body) async {
    final previous = _tail;
    final done = Completer<void>();
    _tail = done.future;
    try {
      await previous;
      return await body();
    } finally {
      done.complete();
    }
  }
}

/// Serializes a session's input and drains it before releasing held input.
class McpInputState {
  McpInputState(this.grantId);

  final String grantId;
  bool get active => _release == null;
  final _queue = McpOperationQueue();
  Future<void>? _release;
  final Set<String> buttons = {};
  final Set<String> keys = {};
  ({int x, int y})? pointer;

  void checkActive() {
    if (_release != null) {
      throw McpToolException('Control was released; this input was cancelled.');
    }
  }

  Future<T> run<T>(Future<T> Function() body) => _queue.run(() {
        checkActive();
        return body();
      });

  // Cancel queued writes immediately, but let the current operation finish
  // before cleanup, including a key/button down still awaiting its FFI call.
  Future<void> release(Future<void> Function() cleanup) =>
      _release ??= _queue.run(cleanup);
}
