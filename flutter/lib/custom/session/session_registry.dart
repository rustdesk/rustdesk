import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:uuid/uuid.dart';

const kMaxSessions = 5;

class SessionEntry {
  final FFI ffi;
  final String peerId;
  final String? peerAlias;
  const SessionEntry({required this.ffi, required this.peerId, this.peerAlias});
  String get label => (peerAlias != null && peerAlias!.isNotEmpty) ? peerAlias! : peerId;
}

class SessionRegistry extends ChangeNotifier {
  SessionRegistry._();
  static final instance = SessionRegistry._();

  final List<SessionEntry> _entries = [];
  List<SessionEntry> get entries => List.unmodifiable(_entries);
  int get count => _entries.length;
  bool get isEmpty => _entries.isEmpty;
  bool get isNotEmpty => !isEmpty;
  bool get isFull => _entries.length >= kMaxSessions;

  UuidValue? _activeSessionId;
  UuidValue? get activeSessionId => _activeSessionId;

  void register(FFI ffi, String peerId, {String? peerAlias}) {
    if (isFull) throw StateError('Cannot register more than $kMaxSessions simultaneous sessions');
    _entries.add(SessionEntry(ffi: ffi, peerId: peerId, peerAlias: peerAlias));
    _activeSessionId ??= ffi.sessionId;
    notifyListeners();
  }

  void setActive(UuidValue sessionId) {
    if (_activeSessionId != sessionId) {
      _activeSessionId = sessionId;
      notifyListeners();
    }
  }

  void unregister(UuidValue sessionId) {
    _entries.removeWhere((e) => e.ffi.sessionId == sessionId);
    if (_activeSessionId == sessionId) {
      _activeSessionId = _entries.isNotEmpty ? _entries.last.ffi.sessionId : null;
    }
    notifyListeners();
  }

  SessionEntry? findById(UuidValue sessionId) {
    try {
      return _entries.firstWhere((e) => e.ffi.sessionId == sessionId);
    } catch (_) {
      return null;
    }
  }

  SessionEntry? findByPeerId(String peerId) {
    try {
      return _entries.firstWhere((e) => e.peerId == peerId);
    } catch (_) {
      return null;
    }
  }
}
