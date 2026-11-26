import 'package:flutter_hbb/mcp/mcp_token.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('rotation returns the replacement that a restarted server loads',
      () async {
    var disk = 'previous';
    var cache = disk;
    final next = await loadMcpToken(
      regenerate: true,
      effective: () => cache,
      persisted: () async => disk,
      save: (value) async => cache = disk = value,
    );
    expect(next, isNot('previous'));
    expect(
        await loadMcpToken(
          effective: () => disk,
          persisted: () async => disk,
          save: (_) => fail('A restart should reuse the saved token'),
        ),
        next);
  });

  test('a failed write rejects rotation and subsequent use of the cached token',
      () async {
    var cache = 'previous';
    Future<String> load({bool regenerate = false}) => loadMcpToken(
          regenerate: regenerate,
          effective: () => cache,
          persisted: () async => 'previous',
          save: (value) async => cache = value,
        );
    await expectLater(load(regenerate: true), throwsStateError);
    await expectLater(load(), throwsStateError);
  });

  test('rotation rejects a replacement overridden by local settings', () async {
    var disk = 'previous';
    await expectLater(
        loadMcpToken(
          regenerate: true,
          effective: () => 'fixed',
          persisted: () async => disk,
          save: (value) async => disk = value,
        ),
        throwsStateError);
  });
}
