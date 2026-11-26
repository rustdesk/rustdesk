import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mcp/mcp_backend.dart';
import 'package:flutter_hbb/mcp/mcp_dispatcher.dart';
import 'package:flutter_hbb/mcp/mcp_remote_window.dart';
import 'package:flutter_hbb/main.dart' show kWindowId;
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:uuid/uuid.dart';
import 'package:flutter_test/flutter_test.dart';

class _Binding extends Fake implements RustdeskImpl {
  final options = <String, String>{};

  @override
  String translate(
          {required String name, required String locale, dynamic hint}) =>
      name;

  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      options[key] ?? '';

  String tab = '';
  String grant = '';

  @override
  String mainGetAppNameSync({dynamic hint}) => 'RustDesk';

  @override
  String mainGetPeerOptionSync(
          {required String id, required String key, dynamic hint}) =>
      '';

  @override
  String mainGetPeerSync({required String id, dynamic hint}) => '{"info":{}}';

  @override
  String sessionMcpConnectionId({required UuidValue sessionId, dynamic hint}) =>
      sessionId.toString() == tab ? 'connection' : '';

  @override
  String mcpSessionId({required String connectionId, dynamic hint}) => tab;

  @override
  bool sessionTrySetAgentControl(
      {required UuidValue sessionId,
      required String connectionId,
      required String grantId,
      required bool initial,
      dynamic hint}) {
    expect(initial, isTrue);
    expect(sessionId.toString(), tab);
    expect(connectionId, 'connection');
    grant = grantId;
    return true;
  }

  @override
  String sessionGetAgentControl({required UuidValue sessionId, dynamic hint}) =>
      grant;
}

class _Session extends Fake implements FFI {
  @override
  final sessionId = UuidValue('00000000-0000-4000-8000-000000000001');
  @override
  final dialogManager = OverlayDialogManager();
}

class _Backend extends RustDeskMcpBackend {
  _Backend(Future<bool?> Function(String) approveConnect)
      : super(approveConnect: approveConnect);

  int listings = 0;
  bool openedMeanwhile = false;

  @override
  Future<List<Map<String, dynamic>>> listSessions() async =>
      ++listings > 1 && openedMeanwhile
          ? [
              {'peer_id': '123'}
            ]
          : [];
}

Matcher toolError(String message) => throwsA(isA<McpToolException>()
    .having((error) => error.message, 'message', message));

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _Binding binding;

  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
  });

  test('legacy control autoapproval still asks before a new connection',
      () async {
    binding.options[kOptionMcpAutoApproveControl] = 'Y';
    var asked = 0;
    final backend = _Backend((peerId) async {
      asked++;
      expect(peerId, '123');
      return false;
    });
    await expectLater(backend.connect('123'),
        toolError('The user denied the connection to 123.'));
    expect(asked, 1);
    expect(kOptionMcpAutoApproveControl, 'allow-mcp-auto-approve');
  });

  test('connection autoapproval does not require control autoapproval',
      () async {
    binding.options[kOptionMcpAutoApproveConnect] = 'Y';
    final backend = _Backend((_) async {
      fail('New connection autoapproval must skip its prompt.');
    })
      ..openedMeanwhile = true;
    // Stop at the existing-session guard; no native session is opened.
    await expectLater(
        backend.connect('123'),
        toolError('The user opened 123 meanwhile. Call '
            'connect again to get that session.'));
    expect(binding.options[kOptionMcpAutoApproveControl], isNull);
  });

  for (final reuse in [false, true]) {
    test(
        reuse
            ? 'a tab reused by the window stays under human control'
            : 'the launched tab receives initial control', () async {
      binding.options[kOptionOpenNewConnInTabs] = 'Y';
      final backend = RustDeskMcpBackend(approveConnect: (_) async => true);
      final messenger =
          TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
      const windows = MethodChannel('mixin.one/flutter_multi_window');
      const events = MethodChannel('mixin.one/flutter_multi_window_channel');
      messenger.setMockMethodCallHandler(windows, (call) async {
        return call.method == 'createWindow' ? 1 : null;
      });
      final humanTab = Uuid().v4();
      messenger.setMockMethodCallHandler(events, (call) async {
        if (call.method == kWindowEventNewRemoteDesktop) {
          final args = jsonDecode(call.arguments['arguments']);
          binding.tab = reuse ? humanTab : args['session_id'] as String;
        } else if (call.method == kWindowEventMcpListSessions) {
          if (binding.tab.isEmpty) return '[]';
          return jsonEncode([
            {
              'peer_id': '123',
              'session_id': binding.tab,
              'ready': true,
              'state': 'ready'
            }
          ]);
        }
        return null;
      });
      addTearDown(() {
        rustDeskWinManager.clearWindowType(WindowType.RemoteDesktop);
        messenger.setMockMethodCallHandler(windows, null);
        messenger.setMockMethodCallHandler(events, null);
      });
      // The window is registered before its human tab has mounted.
      await rustDeskWinManager.newRemoteDesktop(reuse ? '123' : '456');
      if (reuse) {
        await expectLater(
            backend.connect('123'),
            toolError(
                'The user opened 123 meanwhile. Call connect again to get that session.'));
        expect(binding.grant, isEmpty);
        expect((await backend.connect('123'))['mode'], 'human');
      } else {
        final session = await backend.connect('123');
        expect(session['session_id'], binding.tab);
        expect(session['mode'], 'agent');
        expect(binding.grant, isNotEmpty);
      }
    });
  }

  testWidgets('control cancellation and expiry have different results',
      (tester) async {
    const channel = MethodChannel('mixin.one/flutter_multi_window');
    final messenger =
        TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger;
    messenger.setMockMethodCallHandler(channel, (_) async => null);
    kWindowId = 1;
    addTearDown(() {
      messenger.setMockMethodCallHandler(channel, null);
      kWindowId = null;
    });
    final session = _Session();
    await tester.pumpWidget(MaterialApp(
        navigatorKey: globalKey, home: const Scaffold(body: SizedBox())));
    Future<dynamic> request() => handleMcpWindowCall(
        kWindowEventMcpRequestControl,
        jsonEncode({
          'session_id': session.sessionId.toString(),
          'request_id': 'request',
        }),
        {'123': () => session},
        (_) {},
        (_) {});

    final cancelled = request();
    await tester.pump();
    await tester.tap(find.text('Cancel'));
    await tester.pump();
    expect(await cancelled, isFalse);

    final expired = request();
    await tester.pump();
    await tester.pump(const Duration(seconds: 60));
    expect(await expired, isNull);
    await tester.pump();
    expect(find.text('Agent control request'), findsNothing);
  });

  testWidgets('a repeated connect reports approval expiry as a tool error',
      (tester) async {
    var asked = 0;
    final backend = _Backend((_) {
      asked++;
      return Future<bool?>.delayed(const Duration(seconds: 60), () => null);
    });
    final dispatcher = McpDispatcher(backend);
    Future<Map<String, dynamic>> connect() async {
      final raw = await dispatcher.handleRaw(jsonEncode({
        'jsonrpc': '2.0',
        'id': 1,
        'method': 'tools/call',
        'params': {
          'name': 'connect',
          'arguments': {'peer_id': '123'},
        },
      }));
      return jsonDecode(raw!) as Map<String, dynamic>;
    }

    final first = connect();
    await tester.pump();
    await tester.pump(const Duration(seconds: 25));
    final pending = (await first)['result'];
    expect(pending['isError'], isNot(true));
    expect(jsonDecode(pending['content'][0]['text'])['state'], 'opening');

    final second = connect();
    await tester.pump();
    await tester.pump(const Duration(seconds: 25));
    await second;
    final retry = connect();
    await tester.pump();
    await tester.pump(const Duration(seconds: 10));
    final expired = (await retry)['result'];
    expect(expired['isError'], isTrue);
    expect(expired['content'][0]['text'],
        'Connection approval timed out for 123.');
    expect(asked, 1);
  });
}
