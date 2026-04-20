import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/models/security_dialog_helpers.dart';

void main() {
  test('parsePeerIdentityChangedDetails extracts expected and presented fingerprints', () {
    final details = parsePeerIdentityChangedDetails(
      'Handshake failed: peer identity changed (expected abcd1234, got deadbeef)',
    );

    expect(details, isNotNull);
    expect(details!.expectedFingerprint, 'abcd1234');
    expect(details.presentedFingerprint, 'deadbeef');
  });

  test('parsePeerIdentityChangedDetails rejects unrelated or malformed text', () {
    expect(parsePeerIdentityChangedDetails(''), isNull);
    expect(
      parsePeerIdentityChangedDetails('Handshake failed: bootstrap trusted peer identity mismatch'),
      isNull,
    );
    expect(
      parsePeerIdentityChangedDetails(
        'Handshake failed: peer identity changed (expected only-one-value)',
      ),
      isNull,
    );
  });
}
