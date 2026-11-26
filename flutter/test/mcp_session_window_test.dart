import 'dart:convert';

import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/generated_bridge.dart';
import 'package:flutter_hbb/mcp/mcp_remote_window.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:uuid/uuid.dart';

class _Session extends Fake implements FFI {
  @override
  final sessionId = Uuid().v4obj();
  @override
  late final ffiModel = _Model(this);
}

class _Model extends FfiModel {
  _Model(FFI ffi) : super(WeakReference(ffi));

  @override
  void handleMsgBox(Map<String, dynamic> evt, UuidValue sessionId, String id) {}
  @override
  void handlePeerInfo(Map<String, dynamic> evt, String id, bool isCache) {}
}

class _Binding extends Fake implements RustdeskImpl {
  bool ready = false;
  bool lastTab = true;

  @override
  String? sessionGetCommonSync(
          {required UuidValue sessionId,
          required String key,
          required String param,
          dynamic hint}) =>
      (key == 'is_desktop_ready' && ready).toString();
  @override
  String sessionGetAgentControl({required UuidValue sessionId, dynamic hint}) =>
      'grant';
  @override
  String mainGetLocalOption({required String key, dynamic hint}) =>
      key == kOptionAllowAskForNoteAtEndOfConnection ? 'Y' : '';
  @override
  String sessionGetAuditServerSync(
          {required UuidValue sessionId, required String typ, dynamic hint}) =>
      'server';
  @override
  String sessionGetAuditGuid({required UuidValue sessionId, dynamic hint}) =>
      'guid';
  @override
  String sessionGetLastAuditNote(
          {required UuidValue sessionId, dynamic hint}) =>
      '';
  @override
  bool willSessionCloseCloseSession(
          {required UuidValue sessionId, dynamic hint}) =>
      lastTab;
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  late _Binding binding;
  late _Session session;
  setUp(() {
    binding = _Binding();
    platformFFI.setBindingForTesting(binding);
    session = _Session();
  });

  test(
      'list_sessions keeps cached displays unavailable until this round logs in',
      () async {
    session.ffiModel.pi.isSet.value = true;
    Future<Map> describe() async => (jsonDecode(await handleMcpWindowCall(
            kWindowEventMcpListSessions,
            null,
            {'peer': () => session},
            (_) {},
            (_) {})) as List)
        .single;

    final event =
        session.ffiModel.startEventListener(session.sessionId, 'peer');
    expect(await describe(), containsPair('ready', false));
    binding.ready = true;
    for (final (type, state) in [
      ('input-2fa', 'needs_2fa'),
      ('wait-remote-accept-nook', 'waiting_for_remote_accept'),
    ]) {
      await event({'name': 'msgbox', 'type': type});
      await event({'name': 'msgbox', 'type': 'elevation-error'});
      final prompt = await describe();
      expect(prompt['state'], state);
      expect(prompt['ready'], isFalse);
      expect(prompt.containsKey('displays'), isFalse);
    }
    await event({'name': 'peer_info'});
    await event(
        {'name': 'msgbox', 'type': 'custom-error', 'title': 'Privacy mode'});
    expect(await describe(), containsPair('ready', true));
  });

  test('closing the final tab of an audited peer leaves it for the user',
      () async {
    final otherPeer = _Session();
    final closed = <String>[];
    Future<dynamic> close() => handleMcpWindowCall(
        kWindowEventMcpClose,
        jsonEncode(
            {'session_id': session.sessionId.toString(), 'grant_id': 'grant'}),
        {'peer': () => session, 'other': () => otherPeer},
        closed.add,
        (_) {});

    expect(await close(), contains('RustDesk window'));
    expect(closed, isEmpty);
    binding.lastTab = false;
    expect(await close(), isTrue);
    expect(closed, ['peer']);
  });
}
