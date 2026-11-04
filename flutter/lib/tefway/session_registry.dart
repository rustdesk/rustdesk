// Sessions started from this Master window.
//
// The core opens each outgoing remote desktop session in its own sub-window and
// does not expose a list of peers to the main window, so the Master shell keeps
// the entries it created. Liveness is checked against the real sub-window list,
// which is why a session disappears from the panel as soon as its window is
// gone. Nothing here invents a connection state: the panel only reports that
// the session window exists and which one is in the foreground.

import 'package:flutter/foundation.dart';

class TefwaySession {
  TefwaySession({
    required this.peerId,
    required this.windowId,
    required this.startedAt,
  });

  final String peerId;
  final int windowId;
  final DateTime startedAt;
}

class TefwaySessionRegistry extends ChangeNotifier {
  TefwaySessionRegistry._();

  static final TefwaySessionRegistry instance = TefwaySessionRegistry._();

  final List<TefwaySession> _sessions = [];

  List<TefwaySession> get sessions => List.unmodifiable(_sessions);

  void add({required String peerId, required int windowId}) {
    _sessions.removeWhere((s) => s.peerId == peerId || s.windowId == windowId);
    _sessions.add(TefwaySession(
      peerId: peerId,
      windowId: windowId,
      startedAt: DateTime.now(),
    ));
    notifyListeners();
  }

  void removeByWindow(int windowId) {
    final before = _sessions.length;
    _sessions.removeWhere((s) => s.windowId == windowId);
    if (_sessions.length != before) notifyListeners();
  }

  /// Drops entries whose session window no longer exists.
  void retainWindows(Set<int> liveWindowIds) {
    final before = _sessions.length;
    _sessions.removeWhere((s) => !liveWindowIds.contains(s.windowId));
    if (_sessions.length != before) notifyListeners();
  }
}
