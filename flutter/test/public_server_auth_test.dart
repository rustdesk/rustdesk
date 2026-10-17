import 'package:flutter_test/flutter_test.dart';
import 'package:flutter_hbb/common/public_server_auth.dart';

void main() {
  test('only public aliases request the public account', () {
    expect(isPublicServerPeer('123456789@public'), isTrue);
    expect(isPublicServerPeer('123456789/r@public'), isTrue);
    expect(isPublicServerPeer('123 456 789@public'), isTrue);
    expect(isPublicServerPeer('123456789'), isFalse);
    expect(isPublicServerPeer('123456789@private.example?key=public'), isFalse);
  });
}
