import 'dart:async';

class MobileInputSession {
  MobileInputSession({
    required bool Function() canSend,
    required Future<void> Function(String) sendText,
  })  : _canSend = canSend,
        _sendText = sendText;

  final bool Function() _canSend;
  final Future<void> Function(String) _sendText;
  Future<void> _pending = Future.value();
  int _generation = 0;

  void invalidate() => _generation++;

  Future<void> submit(String text) => dispatch(() async {
        if (text.isNotEmpty) {
          await _sendText(text.replaceAll('\r\n', '\n'));
        }
      });

  Future<void> dispatch(Future<void> Function() send,
      {bool isKeyRelease = false}) async {
    final generation = _generation;
    final previous = _pending;
    final completed = Completer<void>();
    _pending = completed.future;
    try {
      await previous;
      if (!isKeyRelease && (generation != _generation || !_canSend())) {
        throw StateError('Connection not allowed');
      }
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
