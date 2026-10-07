import 'dart:async';

import 'mcp_dispatcher.dart';

/// Serializes a session's input and drains it before releasing held input.
class McpInputState {
  Future<void> _tail = Future.value();
  Future<void>? _release;
  final Set<String> buttons = {};
  final Set<String> keys = {};
  ({int x, int y})? pointer;

  void checkActive() {
    if (_release != null) {
      throw McpToolException('Control was released; this input was cancelled.');
    }
  }

  Future<T> _enqueue<T>(Future<T> Function() body) async {
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

  Future<T> run<T>(Future<T> Function() body) => _enqueue(() {
        checkActive();
        return body();
      });

  // Cancel queued writes immediately, but let the current operation finish
  // before cleanup, including a key/button down still awaiting its FFI call.
  Future<void> release(Future<void> Function() cleanup) =>
      _release ??= _enqueue(cleanup);
}
