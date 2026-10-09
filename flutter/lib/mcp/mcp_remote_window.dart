import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart' show kWindowId;
import 'package:flutter_hbb/models/model.dart';
import 'package:get/get.dart';

import 'mcp_utils.dart';

const Duration _kControlRequestTimeout = Duration(seconds: 60);

/// Handles the MCP calls the main window sends to a remote window. [pages]
/// maps each open peer id to a getter for its FFI, which throws until the
/// page has been built.
Future<dynamic> handleMcpWindowCall(
    String method,
    dynamic arguments,
    Map<String, FFI Function()> pages,
    void Function(String peerId) closeTab,
    void Function(String peerId) selectTab) async {
  final sessions = <String, FFI>{};
  for (final e in pages.entries) {
    try {
      sessions[e.key] = e.value();
    } catch (_) {}
  }
  switch (method) {
    case kWindowEventMcpStop:
      final args = jsonDecode(arguments as String);
      for (final ffi in sessions.values) {
        for (final id in args['cancelled_requests'] as List) {
          ffi.dialogManager.dismissByTag('mcp-agent-control-$id');
        }
        ffi.ffiModel.refreshAgentControl();
      }
      return true;
    case kWindowEventMcpListSessions:
      return jsonEncode(
          [for (final e in sessions.entries) _describe(e.key, e.value)]);
    case kWindowEventMcpRefreshControl:
      final args = jsonDecode(arguments as String);
      for (final ffi in sessions.values) {
        final cancelled = args['cancelled_request'];
        if (cancelled is String) {
          ffi.dialogManager.dismissByTag('mcp-agent-control-$cancelled');
        }
        ffi.ffiModel.refreshAgentControl();
      }
      return true;
    case kWindowEventMcpAuthenticate:
      final args = jsonDecode(arguments as String);
      final ffi = _bySessionId(sessions, args['session_id']);
      if (ffi == null || ffi.ffiModel.agentControlGrant != args['grant_id']) {
        return false;
      }
      ffi.dialogManager.dismissAll();
      ffi.ffiModel.mcpLastMsgBox = null;
      return true;
    case kWindowEventMcpClose:
      final args = jsonDecode(arguments as String);
      final entry = sessions.entries
          .where((e) => e.value.sessionId.toString() == args['session_id'])
          .firstOrNull;
      if (entry == null) return false;
      if (args['grant_id'] != null &&
          entry.value.ffiModel.agentControlGrant != args['grant_id']) {
        return false;
      }
      closeTab(entry.key);
      return true;
    case kWindowEventMcpRequestControl:
      final args = jsonDecode(arguments as String);
      final entry = sessions.entries
          .where((e) => e.value.sessionId.toString() == args['session_id'])
          .firstOrNull;
      if (entry == null) return false;
      selectTab(entry.key);
      return _askAgentControl(
          entry.key, entry.value, args['request_id'] as String);
  }
  return null;
}

Map<String, dynamic> _describe(String peerId, FFI ffi) {
  final pi = ffi.ffiModel.pi;
  final ready = pi.isSet.value;
  final box = ffi.ffiModel.mcpLastMsgBox;
  final status = mcpConnectionState(
      ready: ready,
      msgType: box?['type'],
      msgTitle: box?['title'],
      msgText: box?['text']);
  return {
    'peer_id': peerId,
    'session_id': ffi.sessionId.toString(),
    'mode': ffi.ffiModel.agentControl ? 'agent' : 'human',
    'ready': ready,
    'state': status.state,
    if (status.message != null) 'message': status.message,
    if (ready)
      'displays': [
        for (var i = 0; i < pi.displays.length; i++)
          {
            'index': i,
            'x': pi.displays[i].x.round(),
            'y': pi.displays[i].y.round(),
            'width': pi.displays[i].width,
            'height': pi.displays[i].height,
          }
      ],
  };
}

FFI? _bySessionId(Map<String, FFI> sessions, Object? sessionId) =>
    sessions.values
        .where((f) => f.sessionId.toString() == sessionId)
        .firstOrNull;

Future<bool> _askAgentControl(String peerId, FFI ffi, String requestId) async {
  final window = WindowController.fromWindowId(kWindowId!);
  final tag = 'mcp-agent-control-$requestId';
  // Register before yielding so cancellation can find the dialog even while
  // showing or focusing the window is still in progress.
  final answer = ffi.dialogManager
      .show<bool>(
          (setState, close, context) => CustomAlertDialog(
                title: Row(children: [
                  const Icon(Icons.smart_toy_outlined, size: 28),
                  Flexible(
                      child: Text(translate('Agent control request'))
                          .paddingOnly(left: 10)),
                ]),
                content: Text(translate(
                        'An AI agent asks to take exclusive control of {}. Your input is blocked until you take over again.')
                    .replaceAll('{}', peerId)),
                actions: [
                  dialogButton('Cancel',
                      icon: const Icon(Icons.close_rounded),
                      onPressed: close,
                      isOutline: true),
                  dialogButton('OK',
                      icon: const Icon(Icons.done_rounded),
                      onPressed: () => close(true)),
                ],
                // No onSubmit: a stray Enter typed in another tab must not
                // grant control.
                onCancel: close,
              ),
          tag: tag)
      .timeout(_kControlRequestTimeout, onTimeout: () {
    ffi.dialogManager.dismissByTag(tag);
    return null;
  });
  try {
    final results = await Future.wait<dynamic>([
      answer,
      window.show().then((_) => window.focus()),
    ], eagerError: true);
    return results.first == true;
  } finally {
    ffi.dialogManager.dismissByTag(tag);
  }
}

Future<void> _takeOver(FFI ffi) async {
  try {
    // Keep human input gated until the agent's last release has been sent.
    await DesktopMultiWindow.invokeMethod(
        kMainWindowId, kWindowEventMcpControlTakenOver, {
      'session_id': ffi.sessionId.toString(),
      'grant_id': ffi.ffiModel.agentControlGrant,
    });
    ffi.ffiModel.refreshAgentControl();
  } catch (e) {
    debugPrint('Failed to release MCP input: $e');
  }
}

/// Shown over a remote session while an agent has exclusive control.
class AgentControlBanner extends StatelessWidget {
  final FFI ffi;
  const AgentControlBanner({Key? key, required this.ffi}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: ffi.ffiModel,
      builder: (context, _) {
        if (!ffi.ffiModel.agentControl) return const SizedBox.shrink();
        return Positioned(
          top: 10,
          left: 0,
          right: 0,
          child: Center(
            child: Material(
              elevation: 4,
              color: Colors.deepPurple,
              borderRadius: BorderRadius.circular(20),
              child: Padding(
                padding: const EdgeInsets.fromLTRB(14, 6, 6, 6),
                child: Row(mainAxisSize: MainAxisSize.min, children: [
                  const Icon(Icons.smart_toy_outlined,
                      color: Colors.white, size: 18),
                  Text(translate('AI agent is in control'),
                          style: const TextStyle(color: Colors.white))
                      .paddingOnly(left: 8, right: 12),
                  TextButton(
                    style: TextButton.styleFrom(
                      backgroundColor: Colors.white,
                      visualDensity: VisualDensity.compact,
                      shape: RoundedRectangleBorder(
                          borderRadius: BorderRadius.circular(14)),
                    ),
                    onPressed: () => _takeOver(ffi),
                    child: Text(translate('Take over')),
                  ),
                ]),
              ),
            ),
          ),
        );
      },
    );
  }
}
