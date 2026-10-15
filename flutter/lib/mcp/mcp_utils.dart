const String mcpHumanControlMessage =
    'This session is under human control, so the agent may only read it. '
    'Call request_control and wait for the user to approve first.';

const String mcpNotReadyMessage =
    'The session is not ready yet: it is still connecting or waiting for '
    'authentication. Check the RustDesk window, then call list_sessions and '
    'retry once `ready` is true.';

Map<String, String> mcpMouseMessage({
  String type = '',
  String buttons = '',
  required int x,
  required int y,
  List<String> modifiers = const [],
}) =>
    {
      'type': type,
      'buttons': buttons,
      'x': '$x',
      'y': '$y',
      for (final m in modifiers) m: 'true',
    };

/// One wheel notch toward [direction]. Positive y scrolls up and positive x
/// scrolls left, as the remote window sends them.
({int x, int y}) mcpWheelStep(String direction) => switch (direction) {
      'up' => (x: 0, y: 1),
      'down' => (x: 0, y: -1),
      'left' => (x: 1, y: 0),
      _ => (x: -1, y: 0),
    };

List<({int x, int y})> mcpDragPath(({int x, int y}) from, ({int x, int y}) to,
        {int steps = 10}) =>
    [
      for (var i = 1; i <= steps; i++)
        (
          x: from.x + (to.x - from.x) * i ~/ steps,
          y: from.y + (to.y - from.y) * i ~/ steps,
        )
    ];

final Set<String> kMcpKeyNames = {
  for (final c in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'.split('')) 'VK_$c',
  for (var i = 1; i <= 12; i++) 'VK_F$i',
  for (var i = 0; i <= 9; i++) 'VK_NUMPAD$i',
  ...('VK_COMMA VK_SLASH VK_SEMICOLON VK_QUOTE VK_LBRACKET VK_RBRACKET '
          'VK_BACKSLASH VK_MINUS VK_PLUS VK_DIVIDE VK_MULTIPLY VK_SUBTRACT '
          'VK_ADD VK_DECIMAL VK_ENTER VK_CANCEL VK_BACK VK_TAB VK_CLEAR '
          'VK_RETURN VK_SHIFT VK_CONTROL VK_MENU VK_PAUSE VK_CAPITAL VK_KANA '
          'VK_HANGUL VK_JUNJA VK_FINAL VK_HANJA VK_KANJI VK_ESCAPE VK_CONVERT '
          'VK_SPACE VK_PRIOR VK_NEXT VK_END VK_HOME VK_LEFT VK_UP VK_RIGHT '
          'VK_DOWN VK_SELECT VK_PRINT VK_EXECUTE VK_SNAPSHOT VK_SCROLL '
          'VK_INSERT VK_DELETE VK_HELP VK_SLEEP VK_SEPARATOR Meta')
      .split(' '),
};

bool isValidMcpKey(String key) => key.length == 1 || kMcpKeyNames.contains(key);

({int width, int height})? pngSize(List<int> bytes) {
  const signature = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
  if (bytes.length < 24) return null;
  for (var i = 0; i < signature.length; i++) {
    if (bytes[i] != signature[i]) return null;
  }
  int be32(int o) =>
      (bytes[o] << 24) |
      (bytes[o + 1] << 16) |
      (bytes[o + 2] << 8) |
      bytes[o + 3];
  return (width: be32(16), height: be32(20));
}

const Set<String> kMcpBlockedStates = {
  'needs_password',
  'wrong_password',
  'needs_2fa',
  'error',
};

({String state, String? message}) mcpConnectionState({
  required bool ready,
  String? msgType,
  String? msgTitle,
  String? msgText,
}) {
  if (ready) return (state: 'ready', message: null);
  switch (msgType) {
    case 'input-password':
      return (state: 'needs_password', message: null);
    case 're-input-password':
      return (state: 'wrong_password', message: null);
    case 'input-2fa':
      // LOGIN_MSG_2FA_WRONG in src/client.rs.
      return (
        state: 'needs_2fa',
        message:
            msgTitle == 'Wrong 2FA Code' ? 'The 2FA code was rejected.' : null
      );
    case 'wait-remote-accept-nook':
      return (state: 'waiting_for_remote_accept', message: null);
  }
  if (msgType == 'error' ||
      msgTitle == 'Connection Error' ||
      (msgType != null && msgType.contains('error'))) {
    return (state: 'error', message: msgText);
  }
  return (state: 'connecting', message: null);
}

String? mcpNextStep(String state) {
  switch (state) {
    case 'needs_password':
      return 'Call authenticate with the password, or ask the user to type '
          'it in the RustDesk window.';
    case 'wrong_password':
      return 'The password was rejected. Call authenticate with the correct '
          'password, or ask the user to type it in the RustDesk window.';
    case 'needs_2fa':
      return 'Call authenticate with two_factor_code, or ask the user to '
          'enter it in the RustDesk window.';
    case 'error':
      return 'The connection failed. Call disconnect to close the window, '
          'then try again later.';
    case 'waiting_for_remote_accept':
      return 'Waiting for the person at the remote computer to accept the '
          'request. Ask them to accept it, then call get_session with wait_ms.';
    case 'connecting':
      return 'Still connecting. Call get_session with wait_ms to wait longer.';
  }
  return null;
}

/// Text runs and Enter markers, with CRLF counted as one newline.
Iterable<String> mcpTextChunks(String text, {required bool delayed}) sync* {
  final lines = text.split(RegExp(r'\r\n|\r|\n'));
  for (var i = 0; i < lines.length; i++) {
    if (i > 0) yield '\n';
    if (delayed) {
      yield* lines[i].runes.map(String.fromCharCode);
    } else if (lines[i].isNotEmpty) {
      yield lines[i];
    }
  }
}
