import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/mobile/outgoing_connection.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  Future<bool> record(List<String> calls, String method) async {
    calls.add(method);
    return true;
  }

  test('ending the connection stops the service until the session reconnects',
      () async {
    final calls = <String>[];
    final hold = OutgoingConnectionHold();

    await hold.apply(true, (method) => record(calls, method));
    await hold.apply(true, (method) => record(calls, method));
    await hold.apply(false, (method) => record(calls, method));
    await hold.apply(true, (method) => record(calls, method));
    await hold.apply(false, (method) => record(calls, method));
    await hold.apply(false, (method) => record(calls, method));

    expect(calls, [
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
    ]);
  });

  test('a failed service transition can be requested again', () async {
    final calls = <String>[];
    var succeed = false;
    final hold = OutgoingConnectionHold();

    Future<bool> invoke(String method) async {
      calls.add(method);
      return succeed;
    }

    await hold.apply(true, invoke);
    await hold.apply(true, invoke);
    succeed = true;
    await hold.apply(true, invoke);
    await hold.apply(true, invoke);
    succeed = false;
    await hold.apply(false, invoke);
    await hold.apply(false, invoke);

    expect(calls, [
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStartOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
      AndroidChannel.kStopOutgoingForeground,
    ]);
  });
}
