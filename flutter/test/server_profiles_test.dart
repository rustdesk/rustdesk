import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/server_profiles.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('saved servers survive a save and load round trip', () {
    final profiles = [
      ServerProfile(
          'Office',
          ServerConfig(
              idServer: 'id.office', relayServer: 'relay.office', key: 'K1')),
      ServerProfile('Home',
          ServerConfig(idServer: 'id.home', apiServer: 'https://api.home')),
    ];
    final loaded = decodeServerProfiles(encodeServerProfiles(profiles));
    expect(loaded.map((e) => e.name), ['Office', 'Home']);
    expect(loaded[0].config.relayServer, 'relay.office');
    expect(loaded[1].config.apiServer, 'https://api.home');
    expect(decodeServerProfiles('not json'), isEmpty);
  });

  test('current server is matched to its saved entry', () {
    final profiles = [
      ServerProfile('A', ServerConfig(idServer: 'a')),
      ServerProfile(
          'B', ServerConfig(idServer: 'b', relayServer: 'r', key: 'k')),
    ];
    expect(
        indexOfServerProfile(
            profiles, ServerConfig(idServer: 'b', relayServer: 'r/', key: 'k')),
        1);
    expect(indexOfServerProfile(profiles, ServerConfig(idServer: 'c')), -1);
  });
}
