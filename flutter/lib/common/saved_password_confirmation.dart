/// Match the peer configuration ID used by LoginConfigHandler::initialize.
String savedPasswordPeerId(String id) {
  final parts = id.replaceAll(' ', '').split('@');
  var peer = parts.first;
  if (peer.endsWith('/r') || peer.endsWith(r'\r')) {
    peer = peer.substring(0, peer.length - 2);
  }
  return parts.length > 1 ? '$peer@${parts[1].split('?').first}' : peer;
}

class SavedPasswordConfirmation {
  final Set<String> _pending = {};

  Future<bool> confirm({
    required String id,
    required bool enabled,
    required String loginMethod,
    required Future<bool> Function(String) hasSavedPassword,
    required Future<bool?> Function(String) showConfirmation,
  }) async {
    if (!enabled || loginMethod == 'request-approval') return true;
    final peerId = savedPasswordPeerId(id);
    // A second click must not launch another session after the same approval.
    if (!_pending.add(peerId)) return false;
    try {
      if (!await hasSavedPassword(peerId)) return true;
      return await showConfirmation(peerId) == true;
    } finally {
      _pending.remove(peerId);
    }
  }
}
