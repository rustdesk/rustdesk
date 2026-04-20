class PeerIdentityChangeDetails {
  final String expectedFingerprint;
  final String presentedFingerprint;

  const PeerIdentityChangeDetails({
    required this.expectedFingerprint,
    required this.presentedFingerprint,
  });
}

PeerIdentityChangeDetails? parsePeerIdentityChangedDetails(String text) {
  final match = RegExp(
    r'^Handshake failed: peer identity changed \(expected ([^,]+), got ([^)]+)\)$',
  ).firstMatch(text.trim());
  if (match == null) {
    return null;
  }
  final expectedFingerprint = match.group(1)?.trim() ?? '';
  final presentedFingerprint = match.group(2)?.trim() ?? '';
  if (expectedFingerprint.isEmpty || presentedFingerprint.isEmpty) {
    return null;
  }
  return PeerIdentityChangeDetails(
    expectedFingerprint: expectedFingerprint,
    presentedFingerprint: presentedFingerprint,
  );
}
