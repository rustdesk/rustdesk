import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:uuid/uuid.dart';

class SessionInfo {
  final String peerId;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;
  final FFI ffi;

  SessionInfo({
    required this.peerId,
    required this.password,
    required this.isSharedPassword,
    required this.forceRelay,
    required this.ffi,
  });
}

class SessionRegistry extends ChangeNotifier {
  static final SessionRegistry instance = SessionRegistry._();
  SessionRegistry._();

  static const int kMaxSessions = 5;

  final _sessions = <String, SessionInfo>{};
  final _orderedKeys = <String>[];

  int get count => _sessions.length;
  List<String> get peerIds => List.unmodifiable(_orderedKeys);
  SessionInfo? get(String peerId) => _sessions[peerId];
  bool contains(String peerId) => _sessions.containsKey(peerId);

  /// Returns existing FFI if peer already connected, new FFI otherwise.
  /// Returns null if at max capacity.
  FFI? addSession({
    required String peerId,
    required String? password,
    required bool? isSharedPassword,
    required bool? forceRelay,
  }) {
    if (_sessions.containsKey(peerId)) return _sessions[peerId]!.ffi;
    if (_sessions.length >= kMaxSessions) return null;
    final ffi = FFI(const Uuid().v4obj());
    _sessions[peerId] = SessionInfo(
      peerId: peerId,
      password: password,
      isSharedPassword: isSharedPassword,
      forceRelay: forceRelay,
      ffi: ffi,
    );
    _orderedKeys.add(peerId);
    notifyListeners();
    return ffi;
  }

  void removeSession(String peerId) {
    if (!_sessions.containsKey(peerId)) return;
    _sessions.remove(peerId);
    _orderedKeys.remove(peerId);
    notifyListeners();
  }

  Future<void> closeSession(String peerId) async {
    final info = _sessions[peerId];
    if (info == null) return;
    await info.ffi.close();
    removeSession(peerId);
  }
}
