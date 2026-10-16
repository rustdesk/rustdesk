import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/server_profiles.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('server profiles survive an encode/decode round trip', () {
    final raw = encodeServerProfiles({
      'work': ServerConfig(idServer: 'id.work', relayServer: 'relay.work', key: 'k'),
      'default': ServerConfig(),
    });
    final back = decodeServerProfiles(raw);
    expect(back['work']!.relayServer, 'relay.work');
    expect(back['default']!.idServer, '');
  });

  test('garbage input yields no profiles', () {
    expect(decodeServerProfiles('not json'), isEmpty);
  });
}
