import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:uuid/uuid.dart';

import 'mcp_dispatcher.dart';
import 'mcp_input.dart';
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

  bool _stopped = false;

  void _checkRunning() {
    if (_stopped) throw McpToolException('The MCP server has stopped.');
  }

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
    if (method == kWindowEventMcpRequestControl) {
      _checkRunning();
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
        sessions.add(_withNextStep({
          ...s,
          'mode': _inputFor(s['session_id'] as String)?.active == true
              ? 'agent'
              : 'human',
        }));
      }
    }
    _input.removeWhere((id, _) => bind.mcpSessionId(connectionId: id).isEmpty);
    return sessions;
  }

  Map<String, dynamic> _withNextStep(Map<String, dynamic> s) {
    final hint = s['ready'] == true ? null : mcpNextStep(s['state'] as String);
    return hint == null ? s : {...s, 'next_step': hint};
  }

  Future<Map<String, dynamic>> _find(String sessionId) async {
    final sessions = await listSessions();
    _checkRunning();
    for (final s in sessions) {
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
  final Map<String, Future<({String sessionId, McpInputState input})>>
      _opening = {};

  /// Pending control requests, by native connection id.
  final Map<String, ({String id, Future<McpInputState?> result})>
      _controlRequests = {};

  /// The last screenshot request. Screenshots run one at a time because a
  /// peer keeps one pending request per display and drops the older one.
  Future<void> _lastScreenshot = Future.value();

  final Map<String, McpInputState> _input = {};

  String _connectionId(String sessionId) {
    final id = bind.sessionMcpConnectionId(sessionId: UuidValue(sessionId));
    if (id.isEmpty) throw McpToolException('Unknown session_id: $sessionId');
    return id;
  }

  McpInputState? _inputFor(String sessionId) =>
      _input[bind.sessionMcpConnectionId(sessionId: UuidValue(sessionId))];

  void _checkGrant(String sessionId, McpInputState input) {
    _checkRunning();
    input.checkActive();
    if (!identical(_inputFor(sessionId), input) ||
        bind.sessionGetAgentControl(sessionId: UuidValue(sessionId)) !=
            input.grantId) {
      throw McpToolException('Control changed; this operation was cancelled.');
    }
  }

  /// Runs one input call on a connection at a time. Each call awaits between its
  /// events, so two calls would otherwise interleave them.
  Future<T> _serialized<T>(
      String sessionId, Future<T> Function(McpInputState input) body) async {
    _checkRunning();
    final input = _inputFor(sessionId);
    if (input == null) throw McpToolException(mcpHumanControlMessage);
    return input.run(() => body(input));
  }

  /// Releases the buttons and keys the agent left down. Queued behind the
  /// input call in progress, so a press it is sending cannot land after the
  /// release.
  Future<void> releaseHeld(String sessionId) =>
      _releaseConnection(_connectionId(sessionId));

  Future<void> _releaseConnection(String connectionId) async {
    final input = _input[connectionId];
    if (input == null) return;
    await _releaseInput(connectionId, input);
  }

  Future<void> _releaseInput(String connectionId, McpInputState input) =>
      input.release(() async => _clearHeld(connectionId, input));

  void _clearHeld(String connectionId, McpInputState input,
      {bool revoke = true}) {
    bind.mcpReleaseInput(
        connectionId: connectionId,
        grantId: input.grantId,
        buttons: input.buttons.toList(),
        keys: input.keys.toList(),
        revoke: revoke);
    input.buttons.clear();
    input.keys.clear();
  }

  // Notifications only refresh UI from native state. A late notification
  // cannot grant/revoke control, and a frozen window cannot block restart.
  Future<void> _notifyWindow(int id, String method, Object? args) async {
    try {
      await _callWindowId(id, method, args, _kWindowCallTimeout);
    } catch (e) {
      debugPrint('Failed to notify window $id of MCP state: $e');
    }
  }

  Future<void> _refreshControl({String? cancelledRequest}) async {
    await Future.wait(rustDeskWinManager.remoteDesktopWindows.map((window) =>
        _notifyWindow(window, kWindowEventMcpRefreshControl,
            {'cancelled_request': cancelledRequest})));
  }

  Future<void> takeOver(String sessionId, String grantId) async {
    final input = _inputFor(sessionId);
    if (input == null || input.grantId != grantId) return;
    _controlRequests.remove(_connectionId(sessionId));
    await releaseHeld(sessionId);
    await _refreshControl();
  }

  Future<void> stop() async {
    _stopped = true;
    final cancelled = _controlRequests.values.map((r) => r.id).toList();
    _controlRequests.clear();
    // Start every release before awaiting so all session queues are cancelled.
    try {
      await Future.wait(_input.keys.toList().map(_releaseConnection));
    } finally {
      await Future.wait(rustDeskWinManager.remoteDesktopWindows.map((id) =>
          _notifyWindow(
              id, kWindowEventMcpStop, {'cancelled_requests': cancelled})));
    }
  }

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
    final ({String sessionId, McpInputState input}) opened;
    try {
      opened = await opening.timeout(deadline.difference(DateTime.now()));
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
    final sessionId = opened.sessionId;
    final input = opened.input;
    _checkGrant(sessionId, input);
    final settled = await _waitForSettled(
        sessionId, deadline.difference(DateTime.now()),
        input: input, closeOnError: true);
    _checkGrant(sessionId, input);
    final state = settled['state'];
    if (password != null &&
        password.isNotEmpty &&
        (state == 'needs_password' || state == 'wrong_password')) {
      return _authenticate(sessionId, input, password, null,
          deadline.difference(DateTime.now()));
    }
    return settled;
  }

  Future<({String sessionId, McpInputState input})> _open(String peerId) async {
    _checkRunning();
    final autoApprove =
        mainGetLocalBoolOptionSync(kOptionMcpAutoApproveControl);
    if (!autoApprove && !await approveConnect(peerId)) {
      throw McpToolException('The user denied the connection to $peerId.');
    }
    // The human may have opened this peer while the prompt was up. That
    // session stays theirs; the next connect call returns it unchanged.
    final List<Map<String, dynamic>> open;
    try {
      open = await listSessions();
    } on McpWindowBusy {
      throw McpToolException('A RustDesk window is still opening. Call '
          'connect again.');
    }
    if (open.any((s) => s['peer_id'] == peerId)) {
      throw McpToolException('The user opened $peerId meanwhile. Call '
          'connect again to get that session.');
    }
    _checkRunning();
    // The password is not passed to the new window: its arguments are
    // printed to the debug log. It is submitted once asked for.
    await rustDeskWinManager.newRemoteDesktop(peerId);
    final opened = DateTime.now().add(_kSessionWaitTimeout);
    String? sessionId;
    while (sessionId == null && DateTime.now().isBefore(opened)) {
      _checkRunning();
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
    _checkRunning();
    final input = await _requestControl(sessionId, ask: false);
    return (sessionId: sessionId, input: input);
  }

  @override
  Future<String> requestControl(String sessionId) async {
    await _find(sessionId);
    try {
      await _requestControl(sessionId,
          ask: !mainGetLocalBoolOptionSync(kOptionMcpAutoApproveControl));
      return 'Granted. The connection is now under agent control.';
    } on TimeoutException {
      return 'Not granted yet: the user has not answered the request in the '
          'RustDesk window. Call request_control again to keep waiting.';
    }
  }

  Future<McpInputState> _requestControl(String sessionId,
      {required bool ask}) async {
    _checkRunning();
    final connectionId = _connectionId(sessionId);
    var request = _controlRequests[connectionId];
    final current = _input[connectionId];
    if (request == null && current != null && current.active) return current;
    if (request == null) {
      final id = Uuid().v4();
      request =
          (id: id, result: _grantControl(sessionId, connectionId, ask, id));
      _controlRequests[connectionId] = request;
    }
    try {
      final input = await request.result.timeout(_kCallBudget);
      if (_controlRequests[connectionId]?.id == request.id) {
        _controlRequests.remove(connectionId);
      }
      if (input == null) {
        throw McpToolException('The user did not grant control.');
      }
      _checkGrant(sessionId, input);
      return input;
    } on TimeoutException {
      rethrow;
    } catch (_) {
      if (_controlRequests[connectionId]?.id == request.id) {
        _controlRequests.remove(connectionId);
      }
      rethrow;
    }
  }

  Future<McpInputState?> _grantControl(
      String sessionId, String connectionId, bool ask, String grantId) async {
    void checkCurrent() {
      _checkRunning();
      if (_controlRequests[connectionId]?.id != grantId ||
          _connectionId(sessionId) != connectionId) {
        throw McpToolException('The control request was cancelled.');
      }
    }

    // Yield before checking the request so the caller can register it.
    await _releaseConnection(connectionId);
    checkCurrent();
    if (ask) {
      final granted = await _callWindow(
          kWindowEventMcpRequestControl,
          {'session_id': sessionId, 'request_id': grantId},
          _kControlRequestTimeout);
      checkCurrent();
      if (granted != true) return null;
    }
    await _find(sessionId);
    checkCurrent();
    final input = McpInputState(grantId);
    bind.sessionSetAgentControl(
        sessionId: UuidValue(sessionId), grantId: input.grantId);
    _input[connectionId] = input;
    await _refreshControl();
    _checkGrant(sessionId, input);
    return input;
  }

  Future<Map<String, dynamic>> _waitForSettled(
      String sessionId, Duration timeout,
      {McpInputState? input, bool closeOnError = false}) async {
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
      if (input != null) _checkGrant(sessionId, input);
      final state = s['state'] as String;
      if (state == 'error') {
        final reason = s['message'] ?? 'unknown error';
        if (closeOnError && input != null) {
          await _callWindow(kWindowEventMcpClose,
              {'session_id': sessionId, 'grant_id': input.grantId});
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
    final input = _inputFor(sessionId);
    if (input == null) throw McpToolException(mcpHumanControlMessage);
    _checkGrant(sessionId, input);
    final s = await _find(sessionId);
    _checkGrant(sessionId, input);
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
    return _authenticate(sessionId, input, wantsPassword ? password : null,
        wantsPassword ? null : twoFactorCode, _kAuthWaitTimeout);
  }

  Future<Map<String, dynamic>> _authenticate(
      String sessionId,
      McpInputState input,
      String? password,
      String? twoFactorCode,
      Duration wait) async {
    _checkGrant(sessionId, input);
    await _callWindow(kWindowEventMcpAuthenticate,
        {'session_id': sessionId, 'grant_id': input.grantId});
    _checkGrant(sessionId, input);
    // Native validation and credential emission hold the same ownership lock.
    final sent = bind.sessionMcpAuthenticate(
        sessionId: UuidValue(sessionId),
        grantId: input.grantId,
        password: password,
        twoFactorCode: twoFactorCode);
    if (!sent) {
      throw McpToolException('Control changed; authentication was cancelled.');
    }
    final settled = await _waitForSettled(sessionId, wait, input: input);
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
    await _find(sessionId);
    final cancelled = _controlRequests.remove(_connectionId(sessionId));
    await releaseHeld(sessionId);
    await _refreshControl(cancelledRequest: cancelled?.id);
    return 'Released. The human is in control; you can only read this session.';
  }

  @override
  Future<McpImage> screenshot(String sessionId, {int display = 0}) async {
    final previous = _lastScreenshot;
    final done = Completer<void>();
    _lastScreenshot = done.future;
    try {
      await previous;
      return await _screenshot(sessionId, display);
    } finally {
      done.complete();
    }
  }

  Future<McpImage> _screenshot(String sessionId, int display) async {
    final s = await _ready(sessionId);
    _display(s, display);
    final sid = UuidValue(sessionId);
    // The result carries this id back, so a late result of an earlier
    // request is never taken for this one.
    final requestId = Uuid().v4();
    final file =
        File('${Directory.systemTemp.path}/rustdesk_mcp_$requestId.png');
    try {
      await bind.sessionTakeMcpScreenshot(
          sessionId: sid, display: display, requestId: requestId);
      final deadline = DateTime.now().add(_kScreenshotTimeout);
      while (DateTime.now().isBefore(deadline)) {
        final error = await bind.sessionSaveMcpScreenshot(
            sessionId: sid, requestId: requestId, path: file.path);
        if (error != null) {
          if (error.isNotEmpty) {
            throw McpToolException('Screenshot failed: $error');
          }
          return McpImage(await file.readAsBytes());
        }
        await Future.delayed(const Duration(milliseconds: 100));
      }
      throw McpToolException('Timed out waiting for the screenshot.');
    } finally {
      try {
        await file.delete();
      } catch (_) {}
    }
  }

  Future<void> _sendMouse(
          McpInputState input, UuidValue sid, Map<String, String> msg) =>
      bind.sessionSendMcpMouse(
          sessionId: sid, grantId: input.grantId, msg: jsonEncode(msg));

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
  Future<void> _holding(McpInputState input, UuidValue sid,
      List<String> modifiers, Future<void> Function() body) async {
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
        input.checkActive();
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
          List<String> modifiers = const []}) =>
      _serialized(sessionId, (input) async {
        final s = await _writable(sessionId);
        final sid = UuidValue(sessionId);
        final p = _toRemote(s, display, x, y);
        final end = action == 'drag' ? _toRemote(s, display, toX!, toY!) : null;
        return _holding(input, sid, modifiers, () async {
          input.checkActive();
          await _sendMouse(input, sid,
              mcpMouseMessage(x: p.x, y: p.y, modifiers: modifiers));
          input.pointer = p;
          if (action == 'move') return;
          final buttons = switch (action) {
            'right_click' => 'right',
            'middle_click' => 'wheel',
            'down' || 'up' || 'drag' => kMcpMouseButtons[button]!,
            _ => 'left',
          };
          Future<void> press(String type, ({int x, int y}) at) async {
            input.checkActive();
            input.pointer = at;
            if (type == 'down') input.buttons.add(buttons);
            await _sendMouse(
                input,
                sid,
                mcpMouseMessage(
                    type: type,
                    buttons: buttons,
                    x: at.x,
                    y: at.y,
                    modifiers: modifiers));
            if (type == 'up') input.buttons.remove(buttons);
          }

          if (action == 'down' || action == 'up') {
            return press(action, p);
          }
          if (end != null) {
            await press('down', p);
            for (final point in mcpDragPath(p, end)) {
              await Future.delayed(const Duration(milliseconds: 20));
              input.checkActive();
              input.pointer = point;
              await _sendMouse(
                  input,
                  sid,
                  mcpMouseMessage(
                      x: point.x, y: point.y, modifiers: modifiers));
            }
            await press('up', end);
            input.pointer = end;
            return;
          }
          final times = action == 'double_click' ? 2 : 1;
          for (var i = 0; i < times; i++) {
            await press('down', p);
            await press('up', p);
          }
        });
      });

  @override
  Future<void> scroll(
          String sessionId, int x, int y, String direction, int amount,
          {int display = 0, List<String> modifiers = const []}) =>
      _serialized(sessionId, (input) async {
        final s = await _writable(sessionId);
        final sid = UuidValue(sessionId);
        final p = _toRemote(s, display, x, y);
        return _holding(input, sid, modifiers, () async {
          input.checkActive();
          await _sendMouse(input, sid,
              mcpMouseMessage(x: p.x, y: p.y, modifiers: modifiers));
          input.pointer = p;
          final step = mcpWheelStep(direction);
          for (var i = 0; i < amount.clamp(0, 50); i++) {
            input.checkActive();
            await _sendMouse(
                input,
                sid,
                mcpMouseMessage(
                    type: 'wheel', x: step.x, y: step.y, modifiers: modifiers));
          }
        });
      });

  @override
  Future<void> typeText(String sessionId, String text, {int delayMs = 0}) =>
      _serialized(sessionId, (input) async {
        await _writable(sessionId);
        final sid = UuidValue(sessionId);
        final chunks = mcpTextChunks(text, delayed: delayMs > 0).toList();
        for (var i = 0; i < chunks.length; i++) {
          input.checkActive();
          if (chunks[i] == '\n') {
            // Unicode LF is not an Enter key on Windows.
            await _pressKey(input, sessionId, 'VK_RETURN', const [], 'press');
          } else {
            await bind.sessionInputString(sessionId: sid, value: chunks[i]);
          }
          if (delayMs > 0 && i < chunks.length - 1) {
            await Future.delayed(Duration(milliseconds: delayMs));
          }
        }
      });

  @override
  Future<void> pressKey(String sessionId, String key, List<String> modifiers,
          {String action = 'press'}) =>
      _serialized(sessionId, (input) async {
        await _writable(sessionId);
        // A Linux peer turns a modifier key's press into a lone key up.
        if (action == 'press' && kMcpModifierKeys.contains(key)) {
          await _pressKey(input, sessionId, key, modifiers, 'down');
          return _pressKey(input, sessionId, key, modifiers, 'up');
        }
        return _pressKey(input, sessionId, key, modifiers, action);
      });

  Future<void> _pressKey(McpInputState input, String sessionId, String key,
      List<String> modifiers, String action) async {
    input.checkActive();
    if (action == 'down') input.keys.add(key);
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
    if (action == 'up') input.keys.remove(key);
  }

  @override
  Future<void> disconnect(String sessionId) async {
    final input = _inputFor(sessionId);
    if (input == null) throw McpToolException(mcpHumanControlMessage);
    final connectionId = _connectionId(sessionId);
    await _find(sessionId);
    _checkGrant(sessionId, input);
    _controlRequests.remove(connectionId);
    late Future<dynamic> closing;
    await input.release(() async {
      try {
        // Drain/cancel input before closing the last tab, but keep its grant
        // until the window has accepted the close.
        _clearHeld(connectionId, input, revoke: false);
        closing = _callWindow(kWindowEventMcpClose,
            {'session_id': sessionId, 'grant_id': input.grantId});
        // A window error belongs to disconnect, not to the cached input-release
        // result that takeover and shutdown also await. Rethrow it below.
        await closing.catchError((Object _) => null);
      } finally {
        _clearHeld(connectionId, input);
      }
    });
    if (await closing != true) {
      throw McpToolException('Control changed or the session was closed.');
    }
  }
}
