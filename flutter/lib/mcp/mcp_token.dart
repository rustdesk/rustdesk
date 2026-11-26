import 'dart:convert';
import 'dart:math';

Future<String> loadMcpToken({
  required String Function() effective,
  required Future<String> Function() persisted,
  required Future<void> Function(String) save,
  bool regenerate = false,
}) async {
  var token = effective();
  if (regenerate || token.isEmpty) {
    final rng = Random.secure();
    final bytes = List<int>.generate(32, (_) => rng.nextInt(256));
    token = base64Url.encode(bytes).replaceAll('=', '');
    await save(token);
  }
  // The option setter updates its cache even when its write fails.
  if (await persisted() != token || effective() != token) {
    throw StateError('Could not verify the saved MCP token. '
        'The previous token may still work after restarting RustDesk.');
  }
  return token;
}
