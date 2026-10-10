import 'dart:convert';
import 'dart:io';

import 'package:flutter_hbb/mcp/mcp_utils.dart';
import 'package:flutter_hbb/mcp/mcp_dispatcher.dart';
import 'package:flutter_hbb/mcp/mcp_http_server.dart';
import 'package:flutter_test/flutter_test.dart';

// Signature and IHDR of a 1920x1080 PNG; enough for the size reader.
final png1920x1080 = <int>[
  0x89,
  0x50,
  0x4e,
  0x47,
  0x0d,
  0x0a,
  0x1a,
  0x0a,
  0,
  0,
  0,
  13,
  0x49,
  0x48,
  0x44,
  0x52,
  0,
  0,
  0x07,
  0x80,
  0,
  0,
  0x04,
  0x38,
];

class _FakeBackend implements McpBackend {
  final calls = <String>[];

  @override
  Future<List<Map<String, dynamic>>> listPeers() async => [
        {'id': '123456789'}
      ];

  @override
  Future<Map<String, dynamic>> connect(String peerId,
      {String? password}) async {
    calls.add('connect $peerId $password');
    return {'session_id': 'abc', 'peer_id': peerId};
  }

  @override
  Future<List<Map<String, dynamic>>> listSessions() async => [
        {'session_id': 's', 'peer_id': '1', 'mode': 'agent'}
      ];

  @override
  Future<String> requestControl(String sessionId) async {
    if (sessionId == 'denied') {
      throw McpToolException('The user did not grant control.');
    }
    calls.add('request_control $sessionId');
    return 'Granted.';
  }

  @override
  Future<Map<String, dynamic>> getSession(String sessionId,
      {int waitMs = 0}) async {
    calls.add('get_session $sessionId $waitMs');
    return {'session_id': sessionId, 'state': 'ready'};
  }

  @override
  Future<Map<String, dynamic>> authenticate(String sessionId,
      {String? password, String? twoFactorCode}) async {
    calls.add('authenticate $sessionId $password $twoFactorCode');
    return {'session_id': sessionId, 'state': 'ready'};
  }

  @override
  Future<String> releaseControl(String sessionId) async {
    calls.add('release_control $sessionId');
    return 'Released.';
  }

  @override
  Future<McpImage> screenshot(String sessionId, {int display = 0}) async {
    calls.add('screenshot $sessionId $display');
    if (sessionId == 'shot-timeout') {
      throw McpToolException('Timed out waiting for the screenshot.');
    }
    return McpImage(png1920x1080);
  }

  @override
  Future<void> mouse(String sessionId, String action, int x, int y,
          {int display = 0,
          String button = 'left',
          int? toX,
          int? toY,
          List<String> modifiers = const []}) async =>
      calls.add('mouse $sessionId $action $x $y d$display'
          '${toX == null ? '' : ' $button to $toX $toY'}'
          '${modifiers.isEmpty ? '' : ' +${modifiers.join('+')}'}');

  @override
  Future<void> scroll(
          String sessionId, int x, int y, String direction, int amount,
          {int display = 0, List<String> modifiers = const []}) async =>
      calls.add('scroll $sessionId $x $y $direction $amount d$display'
          '${modifiers.isEmpty ? '' : ' +${modifiers.join('+')}'}');

  @override
  Future<void> typeText(String sessionId, String text,
      {int delayMs = 0}) async {
    if (sessionId == 'bad') throw McpToolException('Unknown session_id: bad');
    calls.add('type $sessionId $text');
  }

  @override
  Future<void> pressKey(String sessionId, String key, List<String> modifiers,
          {String action = 'press'}) async =>
      calls.add('key $sessionId $key ${modifiers.join('+')} $action');

  @override
  Future<void> disconnect(String sessionId) async =>
      calls.add('disconnect $sessionId');
}

Future<Map<String, dynamic>> _rpc(
    McpDispatcher d, Map<String, dynamic> msg) async {
  final raw = await d.handleRaw(jsonEncode(msg));
  return jsonDecode(raw!) as Map<String, dynamic>;
}

Map<String, dynamic> _call(String tool, Map<String, dynamic> args) => {
      'jsonrpc': '2.0',
      'id': 1,
      'method': 'tools/call',
      'params': {'name': tool, 'arguments': args},
    };

void main() {
  late _FakeBackend backend;
  late McpDispatcher dispatcher;

  setUp(() {
    backend = _FakeBackend();
    dispatcher = McpDispatcher(backend);
  });

  group('protocol', () {
    test('initialize negotiates version and advertises tools', () async {
      final r = await _rpc(dispatcher, {
        'jsonrpc': '2.0',
        'id': 1,
        'method': 'initialize',
        'params': {'protocolVersion': '2024-11-05'},
      });
      expect(r['result']['protocolVersion'], '2024-11-05');
      expect(r['result']['capabilities']['tools'], isNotNull);
      final fallback = await _rpc(dispatcher, {
        'jsonrpc': '2.0',
        'id': 2,
        'method': 'initialize',
        'params': {'protocolVersion': '1999-01-01'},
      });
      expect(fallback['result']['protocolVersion'], kMcpProtocolVersion);
    });

    test('notifications produce no response', () async {
      final raw = await dispatcher.handleRaw(jsonEncode(
          {'jsonrpc': '2.0', 'method': 'notifications/initialized'}));
      expect(raw, isNull);
    });

    test('unknown method and parse errors', () async {
      final r =
          await _rpc(dispatcher, {'jsonrpc': '2.0', 'id': 7, 'method': 'nope'});
      expect(r['error']['code'], -32601);
      final bad = jsonDecode((await dispatcher.handleRaw('{oops'))!);
      expect(bad['error']['code'], -32700);
    });

    test('batches answer only requests', () async {
      final raw = await dispatcher.handleRaw(jsonEncode([
        {'jsonrpc': '2.0', 'id': 1, 'method': 'ping'},
        {'jsonrpc': '2.0', 'method': 'notifications/initialized'},
      ]));
      final list = jsonDecode(raw!) as List;
      expect(list, hasLength(1));
      expect(list.first['id'], 1);
    });

    test('tools/list exposes every tool with a schema', () async {
      final r = await _rpc(
          dispatcher, {'jsonrpc': '2.0', 'id': 1, 'method': 'tools/list'});
      final names =
          (r['result']['tools'] as List).map((t) => t['name']).toSet();
      expect(names, {
        'list_peers',
        'connect',
        'list_sessions',
        'request_control',
        'release_control',
        'get_session',
        'authenticate',
        'screenshot',
        'mouse',
        'scroll',
        'type_text',
        'press_key',
        'disconnect',
      });
      for (final t in r['result']['tools']) {
        expect(t['inputSchema']['type'], 'object');
        expect(t['annotations']['readOnlyHint'], isA<bool>());
      }
    });
  });

  group('tools', () {
    test('screenshot returns base64 png', () async {
      final r = await _rpc(
          dispatcher, _call('screenshot', {'session_id': 's', 'display': 1}));
      final c = r['result']['content'][0];
      expect(c['type'], 'image');
      expect(c['mimeType'], 'image/png');
      expect(base64Decode(c['data']), png1920x1080);
      expect(r['result']['content'][1]['text'], contains('1920x1080'));
      expect(backend.calls, ['screenshot s 1']);
    });

    test('mouse validates action and coordinates', () async {
      var r = await _rpc(
          dispatcher,
          _call(
              'mouse', {'session_id': 's', 'action': 'fling', 'x': 1, 'y': 2}));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('mouse',
              {'session_id': 's', 'action': 'click', 'x': 'a', 'y': 2}));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('mouse',
              {'session_id': 's', 'action': 'click', 'x': 10.0, 'y': 2}));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['mouse s click 10 2 d0']);
      await _rpc(
          dispatcher,
          _call('mouse', {
            'session_id': 's',
            'action': 'move',
            'x': 1,
            'y': 2,
            'display': 1
          }));
      expect(backend.calls.last, 'mouse s move 1 2 d1');
    });

    test('press_key rejects unknown key names', () async {
      final r = await _rpc(
          dispatcher, _call('press_key', {'session_id': 's', 'key': 'Enter'}));
      expect(r['result']['isError'], true);
      expect(r['result']['content'][0]['text'], contains('VK_RETURN'));
      expect(backend.calls, isEmpty);
    });

    test('authenticate needs a credential and forwards it', () async {
      var r =
          await _rpc(dispatcher, _call('authenticate', {'session_id': 's'}));
      expect(r['result']['isError'], true);
      r = await _rpc(dispatcher,
          _call('authenticate', {'session_id': 's', 'password': 1234}));
      expect(r['result']['isError'], true);
      expect(backend.calls, isEmpty);
      r = await _rpc(dispatcher,
          _call('authenticate', {'session_id': 's', 'password': 'pw'}));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['authenticate s pw null']);
      expect(jsonEncode(r), isNot(contains('pw"')));
    });

    test('get_session passes the wait', () async {
      await _rpc(dispatcher,
          _call('get_session', {'session_id': 's', 'wait_ms': 5000}));
      expect(backend.calls, ['get_session s 5000']);
    });

    test('release_control hands the session back', () async {
      final r =
          await _rpc(dispatcher, _call('release_control', {'session_id': 's'}));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['release_control s']);
    });

    test('press_key rejects unknown modifiers', () async {
      var r = await _rpc(
          dispatcher,
          _call('press_key', {
            'session_id': 's',
            'key': 'VK_C',
            'modifiers': ['super']
          }));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('press_key', {
            'session_id': 's',
            'key': 'VK_C',
            'modifiers': ['ctrl', 'shift']
          }));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['key s VK_C ctrl+shift press']);
    });

    test('request_control reports grant and denial', () async {
      var r =
          await _rpc(dispatcher, _call('request_control', {'session_id': 's'}));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['request_control s']);
      r = await _rpc(
          dispatcher, _call('request_control', {'session_id': 'denied'}));
      expect(r['result']['isError'], true);
    });

    test('list_sessions returns structured content and matching text',
        () async {
      final r = await _rpc(dispatcher, _call('list_sessions', {}));
      final structured = r['result']['structuredContent'];
      expect(structured['sessions'].single['mode'], 'agent');
      expect(jsonDecode(r['result']['content'][0]['text']), structured);
    });

    test('drag needs an end point and forwards the button', () async {
      var r = await _rpc(
          dispatcher,
          _call(
              'mouse', {'session_id': 's', 'action': 'drag', 'x': 1, 'y': 2}));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('mouse', {
            'session_id': 's',
            'action': 'drag',
            'x': 1,
            'y': 2,
            'to_x': 30,
            'to_y': 40,
            'button': 'right',
          }));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['mouse s drag 1 2 d0 right to 30 40']);
    });

    test('scroll takes a direction and a positive amount', () async {
      var r = await _rpc(
          dispatcher,
          _call('scroll', {
            'session_id': 's',
            'x': 1,
            'y': 2,
            'direction': 'left',
            'amount': 0
          }));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('scroll', {
            'session_id': 's',
            'x': 1,
            'y': 2,
            'direction': 'left',
            'amount': 3
          }));
      expect(r['result']['isError'], false);
      expect(backend.calls, ['scroll s 1 2 left 3 d0']);
    });

    test('mouse and scroll take modifiers, held keys must not be modifiers',
        () async {
      var r = await _rpc(
          dispatcher,
          _call('mouse', {
            'session_id': 's',
            'action': 'click',
            'x': 1,
            'y': 2,
            'modifiers': ['shift'],
          }));
      expect(r['result']['isError'], false);
      r = await _rpc(
          dispatcher,
          _call('scroll', {
            'session_id': 's',
            'x': 1,
            'y': 2,
            'direction': 'up',
            'amount': 1,
            'modifiers': ['ctrl'],
          }));
      expect(r['result']['isError'], false);
      expect(backend.calls,
          ['mouse s click 1 2 d0 +shift', 'scroll s 1 2 up 1 d0 +ctrl']);
      r = await _rpc(
          dispatcher,
          _call('mouse', {
            'session_id': 's',
            'action': 'click',
            'x': 1,
            'y': 2,
            'modifiers': ['hyper'],
          }));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('mouse', {
            'session_id': 's',
            'action': 'click',
            'x': 1,
            'y': 2,
            'modifiers': 'shift',
          }));
      expect(r['result']['isError'], true);
      r = await _rpc(
          dispatcher,
          _call('press_key',
              {'session_id': 's', 'key': 'VK_SHIFT', 'action': 'down'}));
      expect(r['result']['isError'], true);
      r = await _rpc(dispatcher,
          _call('press_key', {'session_id': 's', 'key': 'VK_SHIFT'}));
      expect(r['result']['isError'], false);
    });

    test('screenshot_after_ms appends a screenshot to an input', () async {
      final r = await _rpc(
          dispatcher,
          _call('press_key', {
            'session_id': 's',
            'key': 'VK_RETURN',
            'action': 'down',
            'screenshot_after_ms': 0,
          }));
      final content = r['result']['content'] as List;
      expect(content[0]['text'], 'ok');
      expect(content[1]['type'], 'image');
      expect(backend.calls, ['key s VK_RETURN  down', 'screenshot s 0']);
    });

    test('a failed screenshot does not hide that the input was sent', () async {
      final r = await _rpc(
          dispatcher,
          _call('press_key', {
            'session_id': 'shot-timeout',
            'key': 'Meta',
            'screenshot_after_ms': 0,
          }));
      expect(r['result']['isError'], isNot(true));
      final content = r['result']['content'] as List;
      expect(content[0]['text'], 'ok');
      expect(content[1]['text'], contains('screenshot failed'));
    });

    test('text chunks preserve blank lines and normalize CRLF', () {
      expect(mcpTextChunks('\na\r\n\r\nb\n', delayed: false),
          ['\n', 'a', '\n', '\n', 'b', '\n']);
      expect(mcpTextChunks('plain text', delayed: false), ['plain text']);
    });

    test('type_text accepts the limit and rejects longer text', () async {
      final ok = await _rpc(
          dispatcher,
          _call('type_text', {
            'session_id': 's',
            'text': 'a' * kMcpMaxTypeChars,
            'delay_ms': 20
          }));
      expect(ok['result']['isError'], isNot(true));
      final long = await _rpc(
          dispatcher,
          _call('type_text', {
            'session_id': 's',
            'text': 'a' * (kMcpMaxTypeChars + 1),
            'delay_ms': 20
          }));
      expect(long['result']['isError'], true);
      expect(long['result']['content'][0]['text'], contains('split'));
      final slow = await _rpc(
          dispatcher,
          _call('type_text', {
            'session_id': 's',
            'text': 'a' * kMcpMaxTypeChars,
            'delay_ms': 100
          }));
      expect(slow['result']['isError'], true);
      expect(slow['result']['content'][0]['text'], contains('60 seconds'));
    });

    test('backend failures become tool errors', () async {
      final r = await _rpc(
          dispatcher, _call('type_text', {'session_id': 'bad', 'text': 'x'}));
      expect(r['result']['isError'], true);
      expect(r['result']['content'][0]['text'], contains('Unknown session_id'));
    });

    test('missing arguments and unknown tools', () async {
      final r = await _rpc(dispatcher, _call('connect', {}));
      expect(r['result']['isError'], true);
      final u = await _rpc(dispatcher, _call('rm_rf', {}));
      expect(u['error']['code'], -32602);
    });
  });

  group('http hardening', () {
    test('host header must be loopback', () {
      expect(isLoopbackHostHeader('127.0.0.1:21119'), true);
      expect(isLoopbackHostHeader('localhost'), true);
      expect(isLoopbackHostHeader('[::1]:21119'), true);
      expect(isLoopbackHostHeader('evil.com'), false);
      expect(isLoopbackHostHeader('evil.com:21119'), false);
      expect(isLoopbackHostHeader(null), false);
    });

    test('origin must be absent or loopback', () {
      expect(isAllowedOrigin(null), true);
      expect(isAllowedOrigin('http://localhost:3000'), true);
      expect(isAllowedOrigin('https://evil.com'), false);
      expect(isAllowedOrigin('null'), false);
    });
  });

  group('backend helpers', () {
    test('connection state follows the last dialog', () {
      String state(String? type, {String? title, bool ready = false}) =>
          mcpConnectionState(ready: ready, msgType: type, msgTitle: title)
              .state;
      expect(state(null), 'connecting');
      expect(state('input-password'), 'needs_password');
      expect(state('re-input-password'), 'wrong_password');
      expect(state('input-2fa'), 'needs_2fa');
      expect(state('wait-remote-accept-nook'), 'waiting_for_remote_accept');
      expect(state('error'), 'error');
      expect(state('custom-error'), 'error');
      expect(state('info', title: 'Connection Error'), 'error');
      expect(state('input-password', ready: true), 'ready');
      expect(
          mcpConnectionState(ready: false, msgType: 'error', msgText: 'Offline')
              .message,
          'Offline');
      expect(
          mcpConnectionState(
                  ready: false,
                  msgType: 'input-2fa',
                  msgTitle: 'Wrong 2FA Code')
              .message,
          'The 2FA code was rejected.');
    });
  });

  group('http server', () {
    late McpHttpServer server;
    late HttpClient client;
    const token = 'secret-token';

    setUp(() async {
      server = McpHttpServer(dispatcher, token);
      await server.start(port: 0);
      client = HttpClient();
    });

    tearDown(() async {
      client.close(force: true);
      await server.stop();
    });

    Future<(int, String)> post(String body,
        {String? auth = 'Bearer $token',
        String? origin,
        String path = '/mcp'}) async {
      final req = await client
          .postUrl(Uri.parse('http://127.0.0.1:${server.port}$path'));
      if (auth != null) req.headers.set('authorization', auth);
      if (origin != null) req.headers.set('origin', origin);
      req.write(body);
      final res = await req.close();
      return (res.statusCode, await utf8.decoder.bind(res).join());
    }

    final ping = jsonEncode({'jsonrpc': '2.0', 'id': 1, 'method': 'ping'});

    test('serves authenticated requests', () async {
      final (status, body) = await post(ping);
      expect(status, 200);
      expect(jsonDecode(body)['id'], 1);
    });

    test('rejects missing or wrong token', () async {
      expect((await post(ping, auth: null)).$1, 401);
      expect((await post(ping, auth: 'Bearer nope')).$1, 401);
      expect((await post(ping, auth: token)).$1, 401);
    });

    test('rejects foreign origins and unknown paths', () async {
      expect((await post(ping, origin: 'https://evil.com')).$1, 403);
      expect((await post(ping, path: '/other')).$1, 404);
    });

    test('notifications get 202 and GET is not allowed', () async {
      final note =
          jsonEncode({'jsonrpc': '2.0', 'method': 'notifications/initialized'});
      expect((await post(note)).$1, 202);
      final req =
          await client.getUrl(Uri.parse('http://127.0.0.1:${server.port}/mcp'));
      req.headers.set('authorization', 'Bearer $token');
      expect((await req.close()).statusCode, 405);
    });

    test('rejects oversized bodies', () async {
      // The server may drop the connection before the client finishes writing.
      try {
        expect((await post('x' * (2 << 20))).$1, 413);
      } catch (e) {
        expect(e, anyOf(isA<SocketException>(), isA<HttpException>()));
      }
    });
  });
}
