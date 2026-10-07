import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:window_manager/window_manager.dart';

import 'mcp_backend.dart';
import 'mcp_dispatcher.dart';
import 'mcp_http_server.dart';
import 'mcp_input.dart';

class McpServerManager {
  McpServerManager._();
  static final McpServerManager instance = McpServerManager._();

  McpHttpServer? _server;
  RustDeskMcpBackend? _backend;
  final _operations = McpOperationQueue();
  String? lastError;

  bool get running => _server?.running ?? false;
  int get port => _server?.port ?? currentPort;

  int get currentPort {
    final v = int.tryParse(bind.mainGetLocalOption(key: kOptionMcpServerPort));
    return v != null && v > 0 && v < 65536 ? v : kMcpDefaultPort;
  }

  Future<String> token() => _operations.run(() => _token());

  Future<String> _token({bool regenerate = false}) async {
    var t = bind.mainGetLocalOption(key: kOptionMcpServerToken);
    if (t.isEmpty || regenerate) {
      t = generateMcpToken();
      await bind.mainSetLocalOption(key: kOptionMcpServerToken, value: t);
    }
    return t;
  }

  Future<String?> regenerateToken() => _run(() async {
        await _stop();
        await _token(regenerate: true);
        await _sync();
      });

  String endpoint() => 'http://127.0.0.1:$port$kMcpEndpointPath';

  Future<void> sync() async {
    await _run(_sync);
  }

  Future<String?> _run(Future<void> Function() operation) =>
      _operations.run(() async {
        try {
          await operation();
          lastError = null;
        } catch (e) {
          lastError = '$e';
          debugPrint('Failed to update MCP server: $e');
        }
        return lastError;
      });

  Future<void> _sync() async {
    final enabled = mainGetLocalBoolOptionSync(kOptionEnableMcpServer) &&
        !bind.isIncomingOnly();
    if (!enabled) {
      await _stop();
      return;
    }
    if (running && _server?.port == currentPort) return;
    await _stop();
    final backend = RustDeskMcpBackend(approveConnect: _askUser);
    final server = McpHttpServer(McpDispatcher(backend), await _token());
    await server.start(port: currentPort);
    _server = server;
    _backend = backend;
  }

  /// The user took a session back from the agent in its window.
  Future<void> onControlTakenOver(String sessionId) async {
    await _backend?.releaseHeld(sessionId);
  }

  Future<void> _stop() async {
    final s = _server;
    final backend = _backend;
    try {
      await s?.stop();
      _server = null;
    } finally {
      await backend?.stop();
    }
    _backend = null;
  }

  Future<bool> _askUser(String peerId) async {
    final context = globalKey.currentContext;
    if (context == null) return false;
    // A dialog in a hidden or minimized window would never be seen.
    if (await windowManager.isMinimized()) await windowManager.restore();
    await windowManager.show();
    await windowManager.focus();
    final navigator = Navigator.of(context, rootNavigator: true);
    final route = DialogRoute<bool>(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: Text(translate('MCP connection request')),
        content: Text(translate('An MCP client wants to control {}. Allow?')
            .replaceAll('{}', peerId)),
        actions: [
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(false),
              child: Text(translate('Cancel'))),
          TextButton(
              onPressed: () => Navigator.of(ctx).pop(true),
              child: Text(translate('OK'))),
        ],
      ),
    );
    // Remove this dialog only, even if other routes were pushed above it.
    final allowed = await navigator
        .push(route)
        .timeout(const Duration(seconds: 60), onTimeout: () {
      if (route.isActive) navigator.removeRoute(route);
      return false;
    });
    return allowed ?? false;
  }
}
