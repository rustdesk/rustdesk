import 'dart:async';

import 'package:get/get.dart';

import '../common.dart';
import 'model.dart';
import 'platform_model.dart';

class RemoteMicrophoneModel {
  RemoteMicrophoneModel(this.parent);
  final WeakReference<FFI> parent;
  static const preference = 'remote-microphone-auto';
  static RemoteMicrophoneModel? _owner;
  final state = 'unavailable'.obs;
  final error = ''.obs;
  Timer? _timeout;
  Timer? _health;
  bool _disposed = false;
  bool _checkingHealth = false;
  int _generation = 0;
  bool _resumeAfterReconnect = false;
  bool _seenSession = false;

  void send(String action) {
    final ffi = parent.target;
    if (ffi != null) {
      bind.sessionToggleOption(
        sessionId: ffi.sessionId,
        value: 'remote-microphone:$action',
      );
    }
  }

  Future<void> start() async {
    if (_disposed || _owner != null && _owner != this) {
      error.value = 'Another session is using the microphone';
      return;
    }
    if (parent.target?.chatModel.voiceCallStatus.value.name != 'notStarted') {
      error.value = 'Stop the voice call first';
      return;
    }
    final generation = ++_generation;
    _owner = this;
    state.value = 'starting';
    error.value = '';
    try {
      final started =
          await parent.target?.invokeMethod('remote_microphone_start') ?? false;
      if (_disposed || generation != _generation) return;
      if (!started) {
        throw StateError('Grant microphone permission and try again');
      }
      send('start');
      _timeout?.cancel();
      _timeout = Timer(const Duration(seconds: 5), () {
        if (state.value == 'starting') {
          fail('Remote microphone did not respond');
        }
      });
    } catch (failure) {
      fail(failure.toString());
    }
  }

  void stop({bool mute = false}) {
    _resumeAfterReconnect = false;
    if (!isAndroid || state.value == 'unavailable' && _owner != this) return;
    ++_generation;
    _timeout?.cancel();
    _health?.cancel();
    if (_owner == this) {
      parent.target?.invokeMethod('remote_microphone_stop');
      _owner = null;
    }
    send(mute ? 'mute' : 'stop');
    state.value = mute ? 'silenced' : 'stopped';
  }

  void fail(String message) {
    stop();
    error.value = message;
    state.value = 'error';
    showToast(translate(message));
  }

  Future<void> handle(Map<String, dynamic> event, String peerId) async {
    if (_disposed || !isAndroid) return;
    final next = event['state'].toString();
    if (next == 'available') {
      final firstConnection = !_seenSession;
      _seenSession = true;
      state.value = 'stopped';
      error.value = '';
      if (_resumeAfterReconnect ||
          firstConnection &&
              bind.mainGetPeerOptionSync(id: peerId, key: preference) == 'Y') {
        _resumeAfterReconnect = false;
        await start();
      }
    } else if (next == 'active') {
      if (_owner != this || state.value != 'starting') {
        send('stop');
        return;
      }
      _timeout?.cancel();
      state.value = 'active';
      _health?.cancel();
      _health = Timer.periodic(const Duration(seconds: 1), (_) async {
        if (_checkingHealth || _disposed || _owner != this) return;
        _checkingHealth = true;
        try {
          if (!(await parent.target?.invokeMethod('remote_microphone_status') ??
              false)) {
            fail('Android microphone capture stopped');
          }
        } catch (failure) {
          fail(failure.toString());
        } finally {
          _checkingHealth = false;
        }
      });
    } else if (next == 'interrupted' ||
        next == 'error' ||
        next == 'unavailable') {
      _resumeAfterReconnect = next == 'interrupted' &&
          (state.value == 'active' || state.value == 'starting');
      if (_resumeAfterReconnect) {
        ++_generation;
        _timeout?.cancel();
        state.value = 'interrupted';
        // Retain the foreground capture briefly so Android can resume a locked-screen
        // session without starting a microphone service from the background.
        _timeout = Timer(const Duration(seconds: 20), () {
          if (state.value == 'interrupted') {
            _resumeAfterReconnect = false;
            stop();
          }
        });
        return;
      }
      stop();
      state.value = next;
      error.value = event['error']?.toString() ?? '';
      if (next == 'error' && error.value.isNotEmpty) {
        showToast(translate(error.value));
      }
    } else if (next == 'silenced' || next == 'stopped') {
      state.value = next;
    }
  }

  void dispose() {
    stop();
    _disposed = true;
    _resumeAfterReconnect = false;
  }
}
