import 'dart:async';
import 'dart:convert';

import 'mcp_utils.dart';

const String kMcpProtocolVersion = '2025-06-18';

const List<String> kMcpSupportedProtocolVersions = [
  '2025-06-18',
  '2025-03-26',
  '2024-11-05',
];

/// Thrown by a [McpBackend] to report a failure to the model as a tool error
/// (`isError: true`) instead of a protocol error.
class McpToolException implements Exception {
  final String message;
  McpToolException(this.message);

  @override
  String toString() => message;
}

class McpImage {
  final List<int> pngBytes;
  McpImage(this.pngBytes);
}

/// Operations an MCP client may perform. The dispatcher is independent of the
/// RustDesk FFI so that it can be unit tested without the native library.
abstract class McpBackend {
  Future<List<Map<String, dynamic>>> listPeers();

  /// Opens a remote desktop session and returns its description. While the
  /// user has not answered or the window is still opening, returns a
  /// description with state `opening` and no `session_id`.
  Future<Map<String, dynamic>> connect(String peerId, {String? password});

  /// Lists every open session with its `mode`: `agent` or `human`.
  Future<List<Map<String, dynamic>>> listSessions();

  Future<String> requestControl(String sessionId);

  Future<String> releaseControl(String sessionId);

  /// Returns one session's status. With [waitMs] > 0, waits until it is ready
  /// or needs credentials, and throws if the connection failed.
  Future<Map<String, dynamic>> getSession(String sessionId, {int waitMs = 0});

  Future<Map<String, dynamic>> authenticate(String sessionId,
      {String? password, String? twoFactorCode});

  Future<McpImage> screenshot(String sessionId, {int display = 0});

  /// [action] is one of [kMcpMouseActions]. Coordinates are pixels from the
  /// top-left of [display]. [button] applies to `down`, `up` and `drag`;
  /// `drag` ends at ([toX], [toY]).
  Future<void> mouse(String sessionId, String action, int x, int y,
      {int display = 0,
      String button = 'left',
      int? toX,
      int? toY,
      List<String> modifiers = const []});

  Future<void> scroll(
      String sessionId, int x, int y, String direction, int amount,
      {int display = 0, List<String> modifiers = const []});

  Future<void> typeText(String sessionId, String text, {int delayMs = 0});

  /// [key] uses RustDesk key names such as `VK_RETURN` or `VK_A`. [action] is
  /// one of [kMcpKeyActions].
  Future<void> pressKey(String sessionId, String key, List<String> modifiers,
      {String action = 'press'});

  Future<void> disconnect(String sessionId);
}

class _Tool {
  final String name;
  final String description;
  final Map<String, dynamic> properties;
  final List<String> required;
  final Map<String, dynamic> annotations;
  final Map<String, dynamic>? outputSchema;

  final Future<Map<String, dynamic>> Function(Map<String, dynamic> args) run;

  _Tool(this.name, this.description, this.properties, this.required,
      this.annotations, this.run,
      {this.outputSchema});

  Map<String, dynamic> toJson() => {
        'name': name,
        'description': description,
        'inputSchema': {
          'type': 'object',
          'properties': properties,
          if (required.isNotEmpty) 'required': required,
        },
        if (outputSchema != null) 'outputSchema': outputSchema,
        'annotations': annotations,
      };
}

const _readOnly = {'readOnlyHint': true};
const _safeWrite = {'readOnlyHint': false, 'destructiveHint': false};
const _input = {'readOnlyHint': false, 'destructiveHint': true};

const _sessionIdProp = {
  'type': 'string',
  'description': 'Session id returned by `connect` or `list_sessions`.',
};

const _displayProp = {
  'type': 'integer',
  'description': 'Remote display index, default 0 (primary). Coordinates are '
      'measured from this display\'s top-left corner.',
};

const _xProp = {
  'type': 'integer',
  'description': 'X coordinate in remote screen pixels.',
};

const _yProp = {
  'type': 'integer',
  'description': 'Y coordinate in remote screen pixels.',
};

const _modifiersProp = {
  'type': 'array',
  'items': {
    'type': 'string',
    'enum': ['ctrl', 'shift', 'alt', 'command'],
  },
  'description': 'Modifier keys held during this input, e.g. [`shift`] for a '
      'shift-click. They apply to this call only.',
};

const _screenshotAfterProp = {
  'type': 'integer',
  'description': 'If set, wait this many milliseconds (0 to 5000) after the '
      'input and return a screenshot of `display`. This is a fixed wait: it '
      'does not wait for a page to load or an app to finish.',
};

const _sessionSchema = {
  'type': 'object',
  'properties': {
    'peer_id': {'type': 'string'},
    'session_id': {'type': 'string'},
    'mode': {
      'type': 'string',
      'enum': ['agent', 'human'],
    },
    'ready': {'type': 'boolean'},
    'state': {
      'type': 'string',
      'enum': [
        'opening',
        'connecting',
        'ready',
        'needs_password',
        'wrong_password',
        'needs_2fa',
        'waiting_for_remote_accept',
        'error',
      ],
    },
    'message': {'type': 'string'},
    'next_step': {'type': 'string'},
    'displays': {
      'type': 'array',
      'items': {
        'type': 'object',
        'properties': {
          'index': {'type': 'integer'},
          'x': {'type': 'integer'},
          'y': {'type': 'integer'},
          'width': {'type': 'integer'},
          'height': {'type': 'integer'},
        },
        'required': ['index', 'x', 'y', 'width', 'height'],
      },
    },
  },
  'required': ['peer_id', 'state'],
};

class McpDispatcher {
  final McpBackend backend;
  late final Map<String, _Tool> _tools;

  McpDispatcher(this.backend) {
    _tools = {for (final t in _buildTools()) t.name: t};
  }

  List<_Tool> _buildTools() {
    Map<String, dynamic> text(String value) => {
          'content': <Map<String, dynamic>>[
            {'type': 'text', 'text': value}
          ],
        };
    // Structured results also go out as text for clients that ignore
    // `structuredContent`.
    Map<String, dynamic> structured(Map<String, dynamic> value) => {
          'content': [
            {'type': 'text', 'text': jsonEncode(value)}
          ],
          'structuredContent': value,
        };
    String sid(Map<String, dynamic> a) => _requireString(a, 'session_id');
    Future<Map<String, dynamic>> input(
        Map<String, dynamic> a, Future<void> Function() send) async {
      final after = a['screenshot_after_ms'];
      final delay =
          after == null ? null : _requireInt(a, 'screenshot_after_ms');
      await send();
      final result = text('ok');
      if (delay != null) {
        await Future.delayed(Duration(milliseconds: delay.clamp(0, 5000)));
        final display = _optionalInt(a, 'display');
        try {
          final image = await backend.screenshot(sid(a), display: display);
          (result['content'] as List).addAll(_imageContent(image, display));
        } on McpToolException catch (e) {
          (result['content'] as List).add({
            'type': 'text',
            'text': 'The input was sent, but the screenshot failed: '
                '${e.message}',
          });
        }
      }
      return result;
    }

    return [
      _Tool(
        'list_peers',
        'List recently connected RustDesk peers (remote computers).',
        {},
        [],
        _readOnly,
        (a) async => structured({'peers': await backend.listPeers()}),
        outputSchema: {
          'type': 'object',
          'properties': {
            'peers': {
              'type': 'array',
              'items': {
                'type': 'object',
                'properties': {
                  'id': {'type': 'string'},
                  'alias': {'type': 'string'},
                  'hostname': {'type': 'string'},
                  'username': {'type': 'string'},
                  'platform': {'type': 'string'},
                  'online': {'type': 'boolean'},
                },
                'required': ['id'],
              },
            },
          },
          'required': ['peers'],
        },
      ),
      _Tool(
        'connect',
        'Open a remote desktop session to a peer. The user is asked to approve '
            'the connection unless they turned approval off. Returns a '
            '`session_id` for the other tools. A new session starts under '
            'agent control. If the peer is already open, that session is '
            'returned unchanged. Waits up to about 25 seconds: returns once '
            'the session is ready, fails with the reason if the connection '
            'fails, or returns early with a `state` such as `needs_password` '
            'and a `next_step`. State `opening` means the user has not '
            'answered yet or the window is still opening; call connect again '
            'with the same peer_id to keep waiting.',
        {
          'peer_id': {
            'type': 'string',
            'description': 'RustDesk ID of the peer.'
          },
          'password': {
            'type': 'string',
            'description': 'Optional password. Omit to use a saved one.',
          },
        },
        ['peer_id'],
        {..._safeWrite, 'idempotentHint': true},
        (a) async => structured(await backend.connect(
            _requireString(a, 'peer_id'),
            password: _optionalString(a, 'password'))),
        outputSchema: _sessionSchema,
      ),
      _Tool(
        'list_sessions',
        'List all open remote desktop sessions with their mode. `agent` means '
            'you have exclusive control and the human is blocked. `human` means '
            'the human is in control: you may only read (screenshot) until '
            'request_control is granted. `state` is `connecting`, `ready`, '
            '`needs_password`, `wrong_password`, `needs_2fa`, '
            '`waiting_for_remote_accept` or `error`, with a '
            '`next_step` when it is not ready. `displays` gives each '
            'display\'s size and position.',
        {},
        [],
        _readOnly,
        (a) async => structured({'sessions': await backend.listSessions()}),
        outputSchema: {
          'type': 'object',
          'properties': {
            'sessions': {'type': 'array', 'items': _sessionSchema},
          },
          'required': ['sessions'],
        },
      ),
      _Tool(
        'get_session',
        'Get the status of one session: mode, state, displays and a '
            '`next_step` when it is not ready. With `wait_ms` it waits until '
            'the session is ready or needs credentials, and fails with the '
            'reason if the connection failed.',
        {
          'session_id': _sessionIdProp,
          'wait_ms': {
            'type': 'integer',
            'description': 'Milliseconds to wait, 0 to 25000. Default 0.',
          },
        },
        ['session_id'],
        _readOnly,
        (a) async => structured(await backend.getSession(sid(a),
            waitMs: _optionalInt(a, 'wait_ms'))),
        outputSchema: _sessionSchema,
      ),
      _Tool(
        'authenticate',
        'Submit a password or two-factor code to a session whose state is '
            '`needs_password`, `wrong_password` or `needs_2fa`. Needs agent '
            'control. Credentials are used once and never saved. Prefer asking '
            'the user to type secrets in the RustDesk window.',
        {
          'session_id': _sessionIdProp,
          'password': {'type': 'string'},
          'two_factor_code': {'type': 'string'},
        },
        ['session_id'],
        _safeWrite,
        (a) async {
          final password = _optionalString(a, 'password');
          final code = _optionalString(a, 'two_factor_code');
          if ((password == null || password.isEmpty) &&
              (code == null || code.isEmpty)) {
            throw McpToolException('Provide password or two_factor_code.');
          }
          return structured(await backend.authenticate(sid(a),
              password: password, twoFactorCode: code));
        },
        outputSchema: _sessionSchema,
      ),
      _Tool(
        'request_control',
        'Ask for exclusive control of a session that is under human control. '
            'The user must approve unless they turned approval off. Waits up '
            'to about 25 seconds for the answer; if the user has not answered '
            'by then, says so, and calling request_control again keeps '
            'waiting for the same request. The human can take control back '
            'at any time, after which writes fail again.',
        {'session_id': _sessionIdProp},
        ['session_id'],
        {..._safeWrite, 'idempotentHint': true},
        (a) async => text(await backend.requestControl(sid(a))),
      ),
      _Tool(
        'release_control',
        'Hand a session back to the human, for example when your task is '
            'done or you need them to act. Afterwards you can only read it. '
            'Call request_control to get control again.',
        {'session_id': _sessionIdProp},
        ['session_id'],
        {..._safeWrite, 'idempotentHint': true},
        (a) async => text(await backend.releaseControl(sid(a))),
      ),
      _Tool(
        'screenshot',
        'Capture the current screen of a remote session as a PNG image, '
            'followed by a text line with its pixel size. Mouse coordinates '
            'use the same pixels. Allowed in either mode. Take one before '
            'clicking and another afterwards to check the result, or pass '
            '`screenshot_after_ms` to an input tool.',
        {
          'session_id': _sessionIdProp,
          'display': {
            'type': 'integer',
            'description': 'Remote display index, default 0 (primary).',
          },
        },
        ['session_id'],
        _readOnly,
        (a) async {
          final display = _optionalInt(a, 'display');
          final image = await backend.screenshot(sid(a), display: display);
          final size = pngSize(image.pngBytes);
          return {
            'content': _imageContent(image, display),
            'structuredContent': {
              'display': display,
              if (size != null) 'width': size.width,
              if (size != null) 'height': size.height,
            },
          };
        },
        outputSchema: {
          'type': 'object',
          'properties': {
            'display': {'type': 'integer'},
            'width': {'type': 'integer'},
            'height': {'type': 'integer'},
          },
          'required': ['display'],
        },
      ),
      _Tool(
        'mouse',
        'Move the pointer, click, press or release a button, or drag on the '
            'remote screen. Needs agent control. Coordinates are pixels as '
            'seen in a screenshot of the same display. `drag` presses '
            '`button` at (x, y), moves to (to_x, to_y) and releases it there. '
            '`down` and `up` press or release `button` at (x, y) for gestures '
            'drag cannot express; always release what you press.',
        {
          'session_id': _sessionIdProp,
          'display': _displayProp,
          'action': {
            'type': 'string',
            'enum': kMcpMouseActions.toList(),
          },
          'x': _xProp,
          'y': _yProp,
          'button': {
            'type': 'string',
            'enum': kMcpMouseButtons.keys.toList(),
            'description': 'Button for `down`, `up` and `drag`. Default left.',
          },
          'to_x': {
            'type': 'integer',
            'description': 'End X of a `drag`, on the same display.',
          },
          'to_y': {
            'type': 'integer',
            'description': 'End Y of a `drag`, on the same display.',
          },
          'modifiers': _modifiersProp,
          'screenshot_after_ms': _screenshotAfterProp,
        },
        ['session_id', 'action', 'x', 'y'],
        _input,
        (a) async {
          final action = _requireString(a, 'action');
          if (!kMcpMouseActions.contains(action)) {
            throw McpToolException('Unknown mouse action: $action');
          }
          final button = a['button'] ?? 'left';
          if (!kMcpMouseButtons.containsKey(button)) {
            throw McpToolException('Unknown mouse button: $button');
          }
          final isDrag = action == 'drag';
          final x = _requireInt(a, 'x'), y = _requireInt(a, 'y');
          final toX = isDrag ? _requireInt(a, 'to_x') : null;
          final toY = isDrag ? _requireInt(a, 'to_y') : null;
          return input(
              a,
              () => backend.mouse(sid(a), action, x, y,
                  display: _optionalInt(a, 'display'),
                  button: button as String,
                  toX: toX,
                  toY: toY,
                  modifiers: _modifiers(a)));
        },
      ),
      _Tool(
        'scroll',
        'Scroll the mouse wheel at a position. Needs agent control.',
        {
          'session_id': _sessionIdProp,
          'display': _displayProp,
          'x': _xProp,
          'y': _yProp,
          'direction': {
            'type': 'string',
            'enum': kMcpScrollDirections.toList(),
          },
          'amount': {
            'type': 'integer',
            'description': 'Wheel notches, 1 to 50.',
          },
          'modifiers': _modifiersProp,
          'screenshot_after_ms': _screenshotAfterProp,
        },
        ['session_id', 'x', 'y', 'direction', 'amount'],
        _input,
        (a) async {
          final direction = _requireString(a, 'direction');
          if (!kMcpScrollDirections.contains(direction)) {
            throw McpToolException('Unknown scroll direction: $direction');
          }
          final amount = _requireInt(a, 'amount');
          if (amount < 1 || amount > 50) {
            throw McpToolException('amount must be between 1 and 50.');
          }
          final x = _requireInt(a, 'x'), y = _requireInt(a, 'y');
          return input(
              a,
              () => backend.scroll(sid(a), x, y, direction, amount,
                  display: _optionalInt(a, 'display'),
                  modifiers: _modifiers(a)));
        },
      ),
      _Tool(
        'type_text',
        'Type text on the remote machine as keyboard input. By default the '
            'text is sent at once, which is fast and fine in browsers. Each '
            'line break (LF, CRLF or CR) is pressed as Enter, which may submit '
            'a single-line field or form. '
            'Some apps (Notepad, for example) drop or garble characters that '
            'arrive that fast; there set `delay_ms` to 20-30 to type one '
            'character at a time (slower, at most $kMcpMaxTypeChars '
            'characters and 60 seconds per call). Check the result with '
            '`screenshot_after_ms`. Needs agent control.',
        {
          'session_id': _sessionIdProp,
          'text': {'type': 'string'},
          'delay_ms': {
            'type': 'integer',
            'description': 'Pause between characters, 0 to 200. Default 0 '
                '(send everything at once).',
          },
          'display': {
            'type': 'integer',
            'description': 'Display captured by `screenshot_after_ms`, '
                'default 0.',
          },
          'screenshot_after_ms': _screenshotAfterProp,
        },
        ['session_id', 'text'],
        _input,
        (a) async {
          final value = _requireString(a, 'text');
          final delay = _optionalInt(a, 'delay_ms').clamp(0, 200);
          if (delay > 0 && value.runes.length > kMcpMaxTypeChars) {
            throw McpToolException('`text` is longer than $kMcpMaxTypeChars '
                'characters; split it into several calls.');
          }
          // Clients give up on a call long before 1000 slow characters finish.
          if (delay * value.runes.length > 60000) {
            throw McpToolException('Typing this with `delay_ms` $delay takes '
                'over 60 seconds; split it into several calls.');
          }
          return input(
              a, () => backend.typeText(sid(a), value, delayMs: delay));
        },
      ),
      _Tool(
        'press_key',
        'Press a key, optionally with modifiers, e.g. key `VK_C` with '
            'modifiers [`ctrl`]. `action` `down` holds the key and `up` '
            'releases it, for keys that must stay held; always release what '
            'you hold. Modifier keys cannot be held this way: pass '
            '`modifiers` to the input that needs them. `Meta` is the Windows '
            'or Command key, the same key as the `command` modifier. `shift` '
            'does not change a letter or digit key into a capital or symbol: '
            'use `type_text` for those characters. Needs agent control.',
        {
          'session_id': _sessionIdProp,
          'key': {
            'type': 'string',
            'description': 'A key name such as VK_RETURN, VK_TAB, VK_ESCAPE, '
                'VK_BACK, VK_DELETE, VK_SPACE, VK_LEFT, VK_UP, VK_F5, VK_A or '
                'VK_0, or a single character.',
          },
          'modifiers': {
            'type': 'array',
            'items': {
              'type': 'string',
              'enum': kMcpKeyModifiers.toList(),
            },
          },
          'action': {
            'type': 'string',
            'enum': kMcpKeyActions.toList(),
            'description': 'Default `press`: down then up.',
          },
          'display': {
            'type': 'integer',
            'description': 'Display captured by `screenshot_after_ms`, '
                'default 0.',
          },
          'screenshot_after_ms': _screenshotAfterProp,
        },
        ['session_id', 'key'],
        _input,
        (a) async {
          final mods = _modifiers(a);
          final key = _requireString(a, 'key');
          if (!isValidMcpKey(key)) {
            throw McpToolException(
                'Unknown key: $key. Use a name like VK_RETURN, VK_TAB, VK_ESCAPE, '
                'VK_BACK, VK_LEFT, VK_F5 or VK_A, or a single character.');
          }
          final action = a['action'] ?? 'press';
          if (!kMcpKeyActions.contains(action)) {
            throw McpToolException('Unknown key action: $action');
          }
          if (action != 'press' && kMcpModifierKeys.contains(key)) {
            throw McpToolException(
                'A modifier key cannot be held across calls: the remote side '
                'resets modifiers on every input. Pass `modifiers` to the '
                'input that needs it instead.');
          }
          return input(
              a,
              () => backend.pressKey(sid(a), key, mods,
                  action: action as String));
        },
      ),
      _Tool(
        'disconnect',
        'Close a remote desktop session. Needs agent control.',
        {'session_id': _sessionIdProp},
        ['session_id'],
        _input,
        (a) async {
          await backend.disconnect(sid(a));
          return text('ok');
        },
      ),
    ];
  }

  List<Map<String, dynamic>> _imageContent(McpImage image, int display) {
    final size = pngSize(image.pngBytes);
    return [
      {
        'type': 'image',
        'data': base64Encode(image.pngBytes),
        'mimeType': 'image/png',
      },
      if (size != null)
        {
          'type': 'text',
          'text': 'Display $display: ${size.width}x${size.height} px. '
              'Mouse coordinates are pixels in this image.',
        },
    ];
  }

  Future<Map<String, dynamic>?> handle(Object? message) async {
    if (message is! Map<String, dynamic>) {
      return _error(null, -32600, 'Invalid Request');
    }
    final id = message['id'];
    final method = message['method'];
    if (method is! String) {
      // A response from the client to a server request; we send none.
      return id != null &&
              (message.containsKey('result') || message.containsKey('error'))
          ? null
          : _error(id, -32600, 'Invalid Request');
    }
    final isNotification = !message.containsKey('id');
    final params = message['params'] is Map<String, dynamic>
        ? message['params'] as Map<String, dynamic>
        : <String, dynamic>{};
    try {
      final result = await _dispatch(method, params);
      return isNotification
          ? null
          : {'jsonrpc': '2.0', 'id': id, 'result': result};
    } on _RpcError catch (e) {
      return isNotification ? null : _error(id, e.code, e.message);
    } catch (e) {
      return isNotification ? null : _error(id, -32603, 'Internal error: $e');
    }
  }

  Future<String?> handleRaw(String body) async {
    Object? decoded;
    try {
      decoded = jsonDecode(body);
    } on FormatException {
      return jsonEncode(_error(null, -32700, 'Parse error'));
    }
    if (decoded is List) {
      if (decoded.isEmpty) {
        return jsonEncode(_error(null, -32600, 'Invalid Request'));
      }
      final out = <Map<String, dynamic>>[];
      for (final m in decoded) {
        final r = await handle(m);
        if (r != null) out.add(r);
      }
      return out.isEmpty ? null : jsonEncode(out);
    }
    final r = await handle(decoded);
    return r == null ? null : jsonEncode(r);
  }

  Future<Map<String, dynamic>> _dispatch(
      String method, Map<String, dynamic> params) async {
    switch (method) {
      case 'initialize':
        final requested = params['protocolVersion'];
        return {
          'protocolVersion': kMcpSupportedProtocolVersions.contains(requested)
              ? requested
              : kMcpProtocolVersion,
          'capabilities': {
            'tools': {'listChanged': false},
          },
          'serverInfo': {'name': 'rustdesk', 'version': '1.0.0'},
          'instructions':
              'Controls remote computers through RustDesk together with a human. '
                  'Call `connect`, then `screenshot` to see the screen before '
                  'using mouse or keyboard tools. If a write is refused because '
                  'the human is in control, call `request_control`; call '
                  '`release_control` when you are done. Input tools accept '
                  '`screenshot_after_ms` to return a screenshot of the result.',
        };
      case 'ping':
        return {};
      case 'tools/list':
        return {'tools': _tools.values.map((t) => t.toJson()).toList()};
      case 'tools/call':
        final name = params['name'];
        final tool = name is String ? _tools[name] : null;
        if (tool == null) {
          throw _RpcError(-32602, 'Unknown tool: $name');
        }
        final args = params['arguments'] is Map<String, dynamic>
            ? params['arguments'] as Map<String, dynamic>
            : <String, dynamic>{};
        try {
          return {...await tool.run(args), 'isError': false};
        } on McpToolException catch (e) {
          return {
            'content': [
              {'type': 'text', 'text': e.message}
            ],
            'isError': true,
          };
        }
      default:
        if (method.startsWith('notifications/')) return {};
        throw _RpcError(-32601, 'Method not found: $method');
    }
  }

  Map<String, dynamic> _error(Object? id, int code, String message) => {
        'jsonrpc': '2.0',
        'id': id,
        'error': {'code': code, 'message': message},
      };
}

const Set<String> kMcpMouseActions = {
  'move',
  'click',
  'double_click',
  'right_click',
  'middle_click',
  'down',
  'up',
  'drag',
};

const Map<String, String> kMcpMouseButtons = {
  'left': 'left',
  'right': 'right',
  'middle': 'wheel',
};

const Set<String> kMcpScrollDirections = {'up', 'down', 'left', 'right'};

const Set<String> kMcpKeyActions = {'press', 'down', 'up'};

const Map<String, String> kMcpModifierKeyNames = {
  'ctrl': 'VK_CONTROL',
  'shift': 'VK_SHIFT',
  'alt': 'VK_MENU',
  'command': 'Meta',
};

final Set<String> kMcpKeyModifiers = kMcpModifierKeyNames.keys.toSet();

const int kMcpMaxTypeChars = 1000;

const Set<String> kMcpModifierKeys = {
  'VK_SHIFT',
  'VK_CONTROL',
  'VK_MENU',
  'Meta'
};

List<String> _modifiers(Map<String, dynamic> a) {
  final raw = a['modifiers'];
  if (raw == null) return const [];
  if (raw is! List) {
    throw McpToolException('`modifiers` must be an array of strings.');
  }
  for (final m in raw) {
    if (!kMcpKeyModifiers.contains(m)) {
      throw McpToolException('Unknown modifier: $m');
    }
  }
  return raw.cast<String>();
}

class _RpcError implements Exception {
  final int code;
  final String message;
  _RpcError(this.code, this.message);
}

int _optionalInt(Map<String, dynamic> args, String key) =>
    args[key] == null ? 0 : _requireInt(args, key);

String? _optionalString(Map<String, dynamic> args, String key) {
  final v = args[key];
  if (v == null || v is String) return v as String?;
  throw McpToolException('Missing or invalid argument: $key');
}

String _requireString(Map<String, dynamic> args, String key) {
  final v = args[key];
  if (v is String && v.isNotEmpty) return v;
  throw McpToolException('Missing or invalid argument: $key');
}

int _requireInt(Map<String, dynamic> args, String key) {
  final v = args[key];
  if (v is int) return v;
  if (v is double && v == v.roundToDouble()) return v.toInt();
  throw McpToolException('Missing or invalid argument: $key');
}
