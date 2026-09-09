import 'package:flutter_test/flutter_test.dart';

import 'cm_demo.dart' as cm_demo;

void main() {
  test('connection manager demo clients match the current Client API', () {
    expect(cm_demo.testClients, hasLength(4));
    expect(cm_demo.testClients.map((client) => client.name), [
      'UserAAAAAA',
      'UserBBBBB',
      'UserC',
      'UserDDDDDDDDDDDd',
    ]);
    expect(
      cm_demo.testClients.every(
          (client) => client.keyboard && !client.clipboard && !client.audio),
      isTrue,
    );
  });
}
