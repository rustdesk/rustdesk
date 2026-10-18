import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/mobile/outgoing_connection.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('ending the connection stops the service until the session reconnects',
      () {
    final calls = <String>[];
    final hold = OutgoingConnectionHold();

    hold.apply(true, calls.add);
    hold.apply(true, calls.add);
    hold.apply(false, calls.add);
    hold.apply(true, calls.add);
    hold.apply(false, calls.add);
    hold.apply(false, calls.add);

    expect(calls, [
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
    ]);
  });
}
