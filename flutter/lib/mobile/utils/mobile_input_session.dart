import 'dart:async';

class MobileInputKey {
  const MobileInputKey(this.name,
      {this.alt = false,
      this.ctrl = false,
      this.shift = false,
      this.command = false});

  final String name;
  final bool alt;
  final bool ctrl;
  final bool shift;
  final bool command;
}

class MobileInputSession {
  MobileInputSession({
    required bool Function() canSend,
    required Future<void> Function(String) sendText,
    required Future<void> Function(MobileInputKey) sendKey,
  })  : _canSend = canSend,
        _sendText = sendText,
        _sendKey = sendKey;

  final bool Function() _canSend;
  final Future<void> Function(String) _sendText;
  final Future<void> Function(MobileInputKey) _sendKey;
  Future<void> _pending = Future.value();
  int _generation = 0;

  void invalidate() => _generation++;

  Future<void> submit(String text) => dispatch(() async {
        if (text.isNotEmpty) {
          await _sendText(text.replaceAll('\r\n', '\n'));
        }
      });

  Future<void> key(MobileInputKey key) => dispatch(() => _sendKey(key));

  void _check(int generation, {bool isKeyRelease = false}) {
    if (isKeyRelease) return;
    if (generation != _generation || !_canSend()) {
      throw StateError('Connection not allowed');
    }
  }

  Future<void> dispatch(Future<void> Function() send,
      {bool isKeyRelease = false}) async {
    final generation = _generation;
    final previous = _pending;
    final completed = Completer<void>();
    _pending = completed.future;
    try {
      await previous;
      _check(generation, isKeyRelease: isKeyRelease);
      await send();
    } catch (_) {
      // Cancel dependent commands, but retain releases for keys already sent.
      if (generation == _generation) invalidate();
      rethrow;
    } finally {
      completed.complete();
    }
  }
}
