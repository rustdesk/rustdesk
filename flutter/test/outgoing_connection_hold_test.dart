import 'package:flutter_hbb/mobile/outgoing_connection.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('an outgoing session holds the connection until it closes', () {
    final hold = OutgoingConnectionHold();

    expect(hold.open(), isTrue);
    expect(hold.open(), isFalse);
    expect(hold.close(), isTrue);
    expect(hold.close(), isFalse);
  });
}
