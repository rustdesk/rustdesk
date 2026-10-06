import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:uuid/uuid.dart';

import 'mcp_dispatcher.dart';
import 'mcp_utils.dart';

const Duration _kSessionWaitTimeout = Duration(seconds: 20);
const Duration _kWindowCallTimeout = Duration(seconds: 3);
const Duration _kControlRequestTimeout = Duration(seconds: 70);
const Duration _kAuthWaitTimeout = Duration(seconds: 20);
// Longest a tool call blocks, kept under common MCP client request timeouts.
// Approvals that take longer keep running and later calls pick them up.
const Duration _kCallBudget = Duration(seconds: 25);
const Duration _kScreenshotTimeout = Duration(seconds: 15);

/// The remote window did not answer, usually because it is still starting.
class McpWindowBusy extends McpToolException {
  McpWindowBusy()
      : super('The RustDesk window did not respond. If it was just opened, '
            'try again in a moment.');
}

class RustDeskMcpBackend implements McpBackend {
  final Future<bool> Function(String peerId) approveConnect;

  RustDeskMcpBackend({required this.approveConnect});

  @override
  Future<List<Map<String, dynamic>>> listPeers() async {
    return gFFI.recentPeersModel.peers
        .map((p) => {
              'id': p.id,
              'alias': p.alias,
              'hostname': p.hostname,
              'username': p.username,
              'platform': p.platform,
              'online': p.online,
            })
        .toList();
  }

  final Map<String, int> _windowOf = {};

  Future<dynamic> _callWindowId(
      int windowId, String method, Object? args, Duration timeout) async {
    // A call made before the new window registers its handler is never
    // answered, so it must not wait forever.
    try {
      return await DesktopMultiWindow.invokeMethod(
              windowId, method, args == null ? null : jsonEncode(args))
          .timeout(timeout);
    } on TimeoutException {
      throw McpWindowBusy();
    }
  }

  Future<dynamic> _callWindow(String method, Map<String, dynamic> args,
      [Duration timeout = _kWindowCallTimeout]) async {
    final sessionId = args['session_id'] as String;
    var windowId = _windowOf[sessionId];
    if (windowId == null) {
      await listSessions();
      windowId = _windowOf[sessionId];
    }
    if (windowId == null) {
      throw McpToolException('Unknown session_id: $sessionId');
    }
    return _callWindowId(windowId, method, args, timeout);
  }

  @override
  Future<List<Map<String, dynamic>>> listSessions() async {
    final windows = rustDeskWinManager.remoteDesktopWindows;
    final answers = await Future.wait(windows.map((id) async {
      try {
        final raw = await _callWindowId(
            id, kWindowEventMcpListSessions, null, _kWindowCallTimeout);
        return raw is String ? jsonDecode(raw) as List : const [];
      } catch (_) {
        return null;
      }
    }));
    if (windows.isNotEmpty && answers.every((a) => a == null)) {
      throw McpWindowBusy();
    }
    // Keep the sessions of a window that did not answer this time, so they
    // are reported as busy instead of unknown.
    _windowOf.removeWhere((_, w) {
      final i = windows.indexOf(w);
      return i < 0 || answers[i] != null;
    });
    final sessions = <Map<String, dynamic>>[];
    for (var i = 0; i < windows.length; i++) {
      for (final s in (answers[i] ?? const []).cast<Map<String, dynamic>>()) {
        _windowOf[s['session_id'] as String] = windows[i];
        sessions.add(_withNextStep(s));
      }
    }
    return sessions;
  }

  Map<String, dynamic> _withNextStep(Map<String, dynamic> s) {
    final hint = s['ready'] == true ? null : mcpNextStep(s['state'] as String);
    return hint == null ? s : {...s, 'next_step': hint};
  }

  Future<Map<String, dynamic>> _find(String sessionId) async {
    for (final s in await listSessions()) {
      if (s['session_id'] == sessionId) return s;
    }
    if (_windowOf.containsKey(sessionId)) throw McpWindowBusy();
    throw McpToolException('Unknown session_id: $sessionId');
  }

  Future<Map<String, dynamic>> _ready(String sessionId) async {
    final s = await _find(sessionId);
    if (s['ready'] != true) {
      final message = s['message'];
      throw McpToolException(
          'The session is not ready (${s['state']}${message == null ? '' : ': $message'}). ${s['next_step'] ?? mcpNotReadyMessage}');
    }
    return s;
  }

  Future<Map<String, dynamic>> _writable(String sessionId) async {
    final s = await _ready(sessionId);
    if (s['mode'] != 'agent') {
      throw McpToolException(mcpHumanControlMessage);
    }
    return s;
  }

  Map<String, dynamic> _display(Map<String, dynamic> session, int display) {
    final displays = (session['displays'] as List?) ?? const [];
    if (display < 0 || display >= displays.length) {
      throw McpToolException(
          'Display $display does not exist; this session has ${displays.length}.');
    }
    return displays[display] as Map<String, dynamic>;
  }

  /// Connections still waiting for approval or for their window, by peer id.
  /// They outlive the call that started them, so a late answer is kept.
  final Map<String, Future<String>> _opening = {};

  /// Pending control requests, by session id.
  final Map<String, Future<dynamic>> _controlRequests = {};

  @override
  Future<Map<String, dynamic>> connect(String peerId,
      {String? password}) async {
    peerId = peerId.replaceAll(' ', '');
    final deadline = DateTime.now().add(_kCallBudget);
    var opening = _opening[peerId];
    if (opening == null) {
      for (final s in await listSessions()) {
        if (s['peer_id'] == peerId) return s;
      }
      opening = _opening[peerId] = _open(peerId);
      opening.whenComplete(() {
        _opening.remove(peerId);
      }).ignore();
    }
    final String sessionId;
    try {
      sessionId = await opening.timeout(deadline.difference(DateTime.now()));
    } on TimeoutException {
      return {
        'peer_id': peerId,
        'ready': false,
        'state': 'opening',
        'next_step': 'The user has not answered the connection request yet, '
            'or the window is still opening. Call connect again with the '
            'same peer_id to keep waiting.',
      };
    }
    final settled = await _waitForSettled(
        sessionId, deadline.difference(DateTime.now()),
        closeOnError: true);
    final state = settled['state'];
    if (password != null &&
        password.isNotEmpty &&
        (state == 'needs_password' || state == 'wrong_password')) {
      return _authenticate(
          sessionId, password, null, deadline.difference(DateTime.now()));
    }
    return settled;
  }

  Future<String> _open(String peerId) async {
    final autoApprove =
        mainGetLocalBoolOptionSync(kOptionMcpAutoApproveControl);
    if (!autoApprove && !await approveConnect(peerId)) {
      throw McpToolException('The user denied the connection to $peerId.');
    }
    // The password is not passed to the new window: its arguments are
    // printed to the debug log. It is submitted once asked for.
    await rustDeskWinManager.newRemoteDesktop(peerId);
    final opened = DateTime.now().add(_kSessionWaitTimeout);
    String? sessionId;
    while (sessionId == null && DateTime.now().isBefore(opened)) {
      try {
        for (final s in await listSessions()) {
          if (s['peer_id'] == peerId) sessionId = s['session_id'] as String;
        }
      } on McpWindowBusy {
        // The window is still starting; poll again.
      }
      if (sessionId == null) {
        await Future.delayed(const Duration(milliseconds: 250));
      }
    }
    if (sessionId == null) {
      throw McpToolException(
          'Timed out waiting for the session window to open.');
    }
    await _callWindow(
        kWindowEventMcpSetControl, {'session_id': sessionId, 'agent': true});
    return sessionId;
  }

  @override
  Future<String> requestControl(String sessionId) async {
    final s = await _find(sessionId);
    if (s['mode'] == 'agent') return 'Already in agent control.';
    dynamic granted;
    if (mainGetLocalBoolOptionSync(kOptionMcpAutoApproveControl)) {
      granted = await _callWindow(
          kWindowEventMcpSetControl, {'session_id': sessionId, 'agent': true});
    } else {
      var request = _controlRequests[sessionId];
      if (request == null) {
        request = _controlRequests[sessionId] = _callWindow(
            kWindowEventMcpRequestControl,
            {'session_id': sessionId},
            _kControlRequestTimeout);
        request.whenComplete(() {
          _controlRequests.remove(sessionId);
        }).ignore();
      }
      try {
        granted = await request.timeout(_kCallBudget);
      } on TimeoutException {
        return 'Not granted yet: the user has not answered the request in the '
            'RustDesk window. Call request_control again to keep waiting.';
      }
    }
    if (granted != true) {
      throw McpToolException('The user did not grant control.');
    }
    return 'Granted. The session is now under agent control.';
  }

  Future<Map<String, dynamic>> _waitForSettled(
      String sessionId, Duration timeout,
      {bool closeOnError = false}) async {
    final deadline = DateTime.now().add(timeout);
    while (true) {
      Map<String, dynamic> s;
      try {
        s = await _find(sessionId);
      } on McpWindowBusy {
        if (!DateTime.now().isBefore(deadline)) rethrow;
        await Future.delayed(const Duration(milliseconds: 500));
        continue;
      }
      final state = s['state'] as String;
      if (state == 'error') {
        final reason = s['message'] ?? 'unknown error';
        if (closeOnError) {
          await _callWindow(kWindowEventMcpClose, {'session_id': sessionId});
          throw McpToolException('Connection failed: $reason.');
        }
        throw McpToolException('Connection failed: $reason. ${s['next_step']}');
      }
      if (s['ready'] == true ||
          kMcpBlockedStates.contains(state) ||
          !DateTime.now().isBefore(deadline)) {
        return s;
      }
      await Future.delayed(const Duration(milliseconds: 500));
    }
  }

  @override
  Future<Map<String, dynamic>> getSession(String sessionId,
      {int waitMs = 0}) async {
    if (waitMs <= 0) return _find(sessionId);
    await _find(sessionId);
    return _waitForSettled(sessionId,
        Duration(milliseconds: waitMs.clamp(0, _kCallBudget.inMilliseconds)));
  }

  @override
  Future<Map<String, dynamic>> authenticate(String sessionId,
      {String? password, String? twoFactorCode}) async {
    final s = await _find(sessionId);
    if (s['mode'] != 'agent') throw McpToolException(mcpHumanControlMessage);
    final state = s['state'];
    final wantsPassword =
        state == 'needs_password' || state == 'wrong_password';
    if (!wantsPassword && state != 'needs_2fa') {
      throw McpToolException(state == 'ready'
          ? 'The session is already authenticated.'
          : 'The session is not waiting for credentials (state: $state).');
    }
    if (wantsPassword && password == null) {
      throw McpToolException('This session needs a password.');
    }
    if (state == 'needs_2fa' && twoFactorCode == null) {
      throw McpToolException('This session needs a two_factor_code.');
    }
    return _authenticate(sessionId, wantsPassword ? password : null,
        wantsPassword ? null : twoFactorCode, _kAuthWaitTimeout);
  }

  Future<Map<String, dynamic>> _authenticate(String sessionId, String? password,
      String? twoFactorCode, Duration wait) async {
    // The secret goes straight to the native side, not through the window
    // call, whose arguments the remote window prints to the debug log.
    await _callWindow(kWindowEventMcpAuthenticate, {'session_id': sessionId});
    final sid = UuidValue(sessionId);
    if (password != null) {
      await bind.sessionLogin(
          sessionId: sid,
          osUsername: '',
          osPassword: '',
          password: password,
          remember: false);
    } else {
      await bind.sessionSend2Fa(
          sessionId: sid, code: twoFactorCode!, trustThisDevice: false);
    }
    final settled = await _waitForSettled(sessionId, wait);
    if (settled['state'] == 'wrong_password') {
      throw McpToolException(
          'The password was rejected. ${settled['next_step']}');
    }
    if (twoFactorCode != null &&
        settled['state'] == 'needs_2fa' &&
        settled['message'] != null) {
      throw McpToolException('${settled['message']} ${settled['next_step']}');
    }
    return settled;
  }

  @override
  Future<String> releaseControl(String sessionId) async {
    final s = await _find(sessionId);
    if (s['mode'] != 'agent') {
      return 'The session is already under human control.';
    }
    await _callWindow(
        kWindowEventMcpSetControl, {'session_id': sessionId, 'agent': false});
    return 'Released. The human is in control; you can only read this session.';
  }

  @override
  Future<McpImage> screenshot(String sessionId, {int display = 0}) async {
    final s = await _ready(sessionId);
    _display(s, display);
    final sid = UuidValue(sessionId);
    final file =
        File('${Directory.systemTemp.path}/rustdesk_mcp_${Uuid().v4()}.png');
    var timedOut = false;
    try {
      await bind.sessionSetFlutterOption(
          sessionId: sid, k: kMcpScreenshotPathOption, v: file.path);
      bind.sessionTakeScreenshot(sessionId: sid, display: display);
      final deadline = DateTime.now().add(_kScreenshotTimeout);
      while (DateTime.now().isBefore(deadline)) {
        if (await file.exists()) {
          final bytes = await file.readAsBytes();
          if (bytes.isNotEmpty) return McpImage(bytes);
        }
        final status = await bind.sessionGetFlutterOption(
                sessionId: sid, k: kMcpScreenshotPathOption) ??
            '';
        if (status.startsWith(kMcpScreenshotErrorPrefix)) {
          throw McpToolException('Screenshot failed: '
              '${status.substring(kMcpScreenshotErrorPrefix.length)}');
        }
        await Future.delayed(const Duration(milliseconds: 100));
      }
      timedOut = true;
      throw McpToolException('Timed out waiting for the screenshot.');
    } finally {
      await bind.sessionSetFlutterOption(
          sessionId: sid,
          k: kMcpScreenshotPathOption,
          v: timedOut ? kMcpScreenshotDropLate : '');
      try {
        await file.delete();
      } catch (_) {}
    }
  }

  Future<void> _sendMouse(UuidValue sid, Map<String, String> msg) =>
      bind.sessionSendMouse(sessionId: sid, msg: jsonEncode(msg));

  ({int x, int y}) _toRemote(
      Map<String, dynamic> session, int display, int x, int y) {
    final d = _display(session, display);
    final width = d['width'] as int, height = d['height'] as int;
    if (x < 0 || y < 0 || x >= width || y >= height) {
      throw McpToolException(
          'Point ($x, $y) is outside display $display (${width}x$height).');
    }
    return (x: (d['x'] as int) + x, y: (d['y'] as int) + y);
  }

  /// Holds [modifiers] as real key presses around [body]. The remote side
  /// only keeps a modifier down for the single mouse-down event that carries
  /// it, so a click or wheel would otherwise lose it before it completes.
  Future<void> _holding(UuidValue sid, List<String> modifiers,
      Future<void> Function() body) async {
    if (modifiers.isEmpty) return body();
    Future<void> key(String m, bool down) => bind.sessionInputKey(
          sessionId: sid,
          name: kMcpModifierKeyNames[m]!,
          down: down,
          press: false,
          alt: down && modifiers.contains('alt'),
          ctrl: down && modifiers.contains('ctrl'),
          shift: down && modifiers.contains('shift'),
          command: down && modifiers.contains('command'),
        );
    try {
      for (final m in modifiers) {
        await key(m, true);
      }
      await body();
      // On a Wayland peer the mouse events can land after a key event sent
      // right behind them, so give them time before releasing.
      await Future.delayed(const Duration(milliseconds: 50));
    } finally {
      for (final m in modifiers.reversed) {
        await key(m, false);
      }
    }
  }

  @override
  Future<void> mouse(String sessionId, String action, int x, int y,
      {int display = 0,
      String button = 'left',
      int? toX,
      int? toY,
      List<String> modifiers = const []}) async {
    final s = await _writable(sessionId);
    final sid = UuidValue(sessionId);
    final p = _toRemote(s, display, x, y);
    final end = action == 'drag' ? _toRemote(s, display, toX!, toY!) : null;
    return _holding(sid, modifiers, () async {
      await _sendMouse(
          sid, mcpMouseMessage(x: p.x, y: p.y, modifiers: modifiers));
      if (action == 'move') return;
      final buttons = switch (action) {
        'right_click' => 'right',
        'middle_click' => 'wheel',
        'down' || 'up' || 'drag' => kMcpMouseButtons[button]!,
        _ => 'left',
      };
      Future<void> press(String type, ({int x, int y}) at) => _sendMouse(
          sid,
          mcpMouseMessage(
              type: type,
              buttons: buttons,
              x: at.x,
              y: at.y,
              modifiers: modifiers));
      if (action == 'down' || action == 'up') return press(action, p);
      if (end != null) {
        await press('down', p);
        for (final point in mcpDragPath(p, end)) {
          await Future.delayed(const Duration(milliseconds: 20));
          await _sendMouse(sid,
              mcpMouseMessage(x: point.x, y: point.y, modifiers: modifiers));
        }
        return press('up', end);
      }
      final times = action == 'double_click' ? 2 : 1;
      for (var i = 0; i < times; i++) {
        await press('down', p);
        await press('up', p);
      }
    });
  }

  @override
  Future<void> scroll(
      String sessionId, int x, int y, String direction, int amount,
      {int display = 0, List<String> modifiers = const []}) async {
    final s = await _writable(sessionId);
    final sid = UuidValue(sessionId);
    final p = _toRemote(s, display, x, y);
    return _holding(sid, modifiers, () async {
      await _sendMouse(
          sid, mcpMouseMessage(x: p.x, y: p.y, modifiers: modifiers));
      final step = mcpWheelStep(direction);
      for (var i = 0; i < amount.clamp(0, 50); i++) {
        await _sendMouse(
            sid,
            mcpMouseMessage(
                type: 'wheel', x: step.x, y: step.y, modifiers: modifiers));
      }
    });
  }

  @override
  Future<void> typeText(String sessionId, String text,
      {int delayMs = 0}) async {
    await _writable(sessionId);
    final sid = UuidValue(sessionId);
    final chunks = mcpTextChunks(text, delayed: delayMs > 0).toList();
    for (var i = 0; i < chunks.length; i++) {
      // The human may take over during a slow delayed run.
      if (delayMs > 0 && i > 0) await _writable(sessionId);
      if (chunks[i] == '\n') {
        // Unicode LF is not an Enter key on Windows.
        await pressKey(sessionId, 'VK_RETURN', const []);
      } else {
        await bind.sessionInputString(sessionId: sid, value: chunks[i]);
      }
      if (delayMs > 0 && i < chunks.length - 1) {
        await Future.delayed(Duration(milliseconds: delayMs));
      }
    }
  }

  @override
  Future<void> pressKey(String sessionId, String key, List<String> modifiers,
      {String action = 'press'}) async {
    await _writable(sessionId);
    // A Linux peer turns a modifier key's press into a lone key up.
    if (action == 'press' && kMcpModifierKeys.contains(key)) {
      await pressKey(sessionId, key, modifiers, action: 'down');
      return pressKey(sessionId, key, modifiers, action: 'up');
    }
    await bind.sessionInputKey(
      sessionId: UuidValue(sessionId),
      name: key,
      down: action == 'down',
      press: action == 'press',
      alt: modifiers.contains('alt'),
      ctrl: modifiers.contains('ctrl'),
      shift: modifiers.contains('shift'),
      command: modifiers.contains('command'),
    );
  }

  @override
  Future<void> disconnect(String sessionId) async {
    final s = await _find(sessionId);
    if (s['mode'] != 'agent') throw McpToolException(mcpHumanControlMessage);
    final closed =
        await _callWindow(kWindowEventMcpClose, {'session_id': sessionId});
    if (closed != true) {
      throw McpToolException('Unknown session_id: $sessionId');
    }
  }
}
