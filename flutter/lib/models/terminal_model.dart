import 'dart:async';
import 'dart:convert';
import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/main.dart';
import 'package:xterm/xterm.dart';

import 'input_modifier_utils.dart';
import 'model.dart';
import 'platform_model.dart';

class TerminalModel with ChangeNotifier {
  final String id; // peer id
  final FFI parent;
  final int terminalId;
  late final Terminal terminal;
  late final TerminalController terminalController;

  bool _terminalOpened = false;
  bool get terminalOpened => _terminalOpened;

  bool _disposed = false;

  /// Callback to check whether Ctrl modifier lock is currently active.
  /// When active, keyboard input is mapped to control codes (e.g. 'b' → \x02).
  bool Function()? isCtrlLocked;

  /// Callback to clear Ctrl lock after a key is pressed (one-shot mode).
  void Function()? clearCtrlLock;

  /// Callback to check whether Alt modifier lock is currently active.
  bool Function()? isAltLocked;

  /// Callback to clear Alt lock after a key is pressed (one-shot mode).
  void Function()? clearAltLock;

  final _inputBuffer = <String>[];

  /// Exposes buffered input only for lifecycle regression tests.
  @visibleForTesting
  int get debugBufferedInputCount => _inputBuffer.length;

  // Buffer for output data received before terminal view has valid dimensions.
  // This prevents NaN errors when writing to terminal before layout is complete.
  final _pendingOutputChunks = <String>[];
  final _pendingOutputSuppressFlags = <bool>[];
  int _pendingOutputSize = 0;
  static const int _kMaxOutputBufferChars = 8 * 1024;
  // View ready state: true when terminal has valid dimensions, safe to write
  bool _terminalViewReady = false;
  bool _markViewReadyScheduled = false;
  bool _suppressTerminalOutput = false;
  bool _suppressNextTerminalDataOutput = false;

  void Function(int w, int h, int pw, int ph)? onResizeExternal;

  /// Called when the terminal session ends (shell exits).
  /// The listener (typically TerminalPage) can use this to auto-close the tab/page.
  VoidCallback? onClosed;

  Future<void> _handleInput(String data) async {
    // Soft keyboards (notably iOS) emit '\n' when Enter is pressed, while a
    // real keyboard's Enter sends '\r'. Some Android keyboards also emit '\n'.
    // - Peer Windows: '\r' works, '\n' is just a newline.
    // - Peer Linux: canonical-mode shells accept both, but raw-mode apps
    //   (readline, prompt_toolkit, vim, TUI frameworks) expect '\r'.
    // - Peer macOS: same as Linux, raw-mode apps expect '\r'
    //   (https://github.com/rustdesk/rustdesk/issues/14907).
    // So on mobile / web-mobile, normalize the original lone '\n' to '\r'
    // before modifier mappings. This keeps Ctrl+J mapped to LF instead of
    // having the generated control code rewritten to CR afterward.
    // Multi-character keyboard payloads, such as terminal escape sequences,
    // remain unchanged. Paste input follows a separate preprocessing path.
    final ctrlLocked = isCtrlLocked?.call() ?? false;
    final altLocked = isAltLocked?.call() ?? false;
    final modifiersActive = ctrlLocked || altLocked;
    // Use the same predicate for transformation and consumption. Control keys
    // and escape sequences must not silently consume a pending one-shot lock.
    final shouldConsumeModifiers =
        modifiersActive && shouldApplyTerminalInputModifiers(data);
    data = prepareTerminalInputPayload(
      data,
      // IME soft-keyboard paste prompts currently arrive from xterm as normal
      // text input with no paste-origin metadata. Keep them on the keyboard path;
      // clipboard-content heuristics can misclassify ordinary typing.
      source: TerminalInputSource.keyboard,
      isMobileOrWebMobile: isMobile || (isWeb && !isWebDesktop),
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: ctrlLocked,
      altLocked: altLocked,
    );
    if (shouldConsumeModifiers) {
      if (ctrlLocked) clearCtrlLock?.call();
      if (altLocked) clearAltLock?.call();
    }
    return _sendInputPayload(data);
  }

  /// Sends an already prepared payload without applying keyboard semantics.
  /// Both normal input and paste use this transport path after their source-
  /// specific preprocessing has completed.
  Future<void> _sendInputPayload(String data) async {
    // Clipboard reads and native sends may complete after the terminal page has
    // closed. Never send or re-buffer input once this model is disposed.
    if (_disposed) return;

    if (_terminalOpened) {
      // Send user input to remote terminal
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending terminal input: $e');
      }
    } else {
      debugPrint('[TerminalModel] Terminal not opened yet, buffering input');
      _inputBuffer.add(data);
    }
  }

  TerminalModel(this.parent, [this.terminalId = 0]) : id = parent.id {
    terminal = Terminal(maxLines: 10000);
    terminalController = TerminalController();

    // Setup terminal callbacks
    terminal.onOutput = (data) {
      if (_suppressTerminalOutput) return;
      _handleInput(data);
    };

    terminal.onResize = (w, h, pw, ph) async {
      // Validate all dimensions before using them
      if (w > 0 && h > 0 && pw > 0 && ph > 0) {
        debugPrint(
            '[TerminalModel] Terminal resized to ${w}x$h (pixel: ${pw}x$ph)');

        // This piece of code must be placed before the conditional check in order to initialize properly.
        onResizeExternal?.call(w, h, pw, ph);

        // Mark terminal view as ready and flush any buffered output on first valid resize.
        // Must be after onResizeExternal so the view layer has valid dimensions before flushing.
        if (!_terminalViewReady) {
          _scheduleMarkViewReady();
        }

        if (_terminalOpened) {
          // Notify remote terminal of resize
          try {
            await bind.sessionResizeTerminal(
              sessionId: parent.sessionId,
              terminalId: terminalId,
              rows: h,
              cols: w,
            );
          } catch (e) {
            debugPrint('[TerminalModel] Error resizing terminal: $e');
          }
        }
      } else {
        debugPrint(
            '[TerminalModel] Invalid terminal dimensions: ${w}x$h (pixel: ${pw}x$ph)');
      }
    };
  }

  void onReady() {
    parent.dialogManager.dismissAll();

    // Fire and forget - don't block onReady. If the transport reconnects while
    // this model is still open, re-send OpenTerminal so the remote service marks
    // the persistent session active again and resumes output streaming.
    openTerminal(force: _terminalOpened).catchError((e) {
      debugPrint('[TerminalModel] Error opening terminal: $e');
    });
  }

  Future<void> openTerminal({bool force = false}) async {
    if (_terminalOpened && !force) return;
    // Request the remote side to open a terminal with default shell
    // The remote side will decide which shell to use based on its OS

    // Get terminal dimensions, ensuring they are valid
    int rows = 24;
    int cols = 80;

    if (terminal.viewHeight > 0) {
      rows = terminal.viewHeight;
    }
    if (terminal.viewWidth > 0) {
      cols = terminal.viewWidth;
    }

    debugPrint(
        '[TerminalModel] Opening terminal $terminalId, sessionId: ${parent.sessionId}, size: ${cols}x$rows');
    try {
      await bind
          .sessionOpenTerminal(
        sessionId: parent.sessionId,
        terminalId: terminalId,
        rows: rows,
        cols: cols,
      )
          .timeout(
        const Duration(seconds: 5),
        onTimeout: () {
          throw TimeoutException(
              'sessionOpenTerminal timed out after 5 seconds');
        },
      );
      debugPrint('[TerminalModel] sessionOpenTerminal called successfully');
    } catch (e) {
      debugPrint('[TerminalModel] Error calling sessionOpenTerminal: $e');
      // Optionally show error to user
      if (e is TimeoutException) {
        _writeToTerminal('Failed to open terminal: Connection timeout\r\n');
      }
    }
  }

  Future<void> sendVirtualKey(String data) async {
    return _handleInput(data);
  }

  Future<void> pasteText(String data) async {
    final payload = prepareTerminalInputPayload(
      data,
      source: TerminalInputSource.paste,
      isMobileOrWebMobile: false,
      bracketedPasteMode: terminal.bracketedPasteMode,
      ctrlLocked: false,
      altLocked: false,
    );
    return _sendInputPayload(payload);
  }

  Future<void> closeTerminal() async {
    if (_terminalOpened) {
      try {
        await bind
            .sessionCloseTerminal(
          sessionId: parent.sessionId,
          terminalId: terminalId,
        )
            .timeout(
          const Duration(seconds: 3),
          onTimeout: () {
            throw TimeoutException(
                'sessionCloseTerminal timed out after 3 seconds');
          },
        );
        debugPrint('[TerminalModel] sessionCloseTerminal called successfully');
      } catch (e) {
        debugPrint('[TerminalModel] Error calling sessionCloseTerminal: $e');
        // Continue with cleanup even if close fails
      }
      _terminalOpened = false;
      notifyListeners();
    }
  }

  static int getTerminalIdFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('terminal_id')) {
      final v = evt['terminal_id'];
      if (v is int) {
        // Desktop and mobile send terminal_id as an int
        return v;
      } else if (v is String) {
        // Web sends terminal_id as a string
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse terminal_id as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        // Unexpected type, log and handle gracefully
        debugPrint(
            '[TerminalModel] Unexpected terminal_id type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain terminal_id');
      return 0;
    }
  }

  static bool getSuccessFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('success')) {
      final v = evt['success'];
      if (v is bool) {
        // Desktop and mobile
        return v;
      } else if (v is String) {
        // Web
        return v.toLowerCase() == 'true';
      } else {
        // Unexpected type, log and handle gracefully
        debugPrint(
            '[TerminalModel] Unexpected success type: ${v.runtimeType}, value: $v. Expected bool or String.');
        return false;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain success');
      return false;
    }
  }

  static int getExitCodeFromEvt(Map<String, dynamic> evt) {
    if (evt.containsKey('exit_code')) {
      final v = evt['exit_code'];
      if (v is int) {
        // Desktop and mobile send exit_code as an int
        return v;
      } else if (v is String) {
        // Web sends exit_code as a string
        final parsed = int.tryParse(v);
        if (parsed != null) {
          return parsed;
        } else {
          debugPrint(
              '[TerminalModel] Failed to parse exit_code as integer: $v. Expected a numeric string.');
          return 0;
        }
      } else {
        debugPrint(
            '[TerminalModel] Unexpected exit_code type: ${v.runtimeType}, value: $v. Expected int or String.');
        return 0;
      }
    } else {
      debugPrint('[TerminalModel] Event does not contain exit_code');
      return 0;
    }
  }

  void handleTerminalResponse(Map<String, dynamic> evt) {
    final String? type = evt['type'];
    final int evtTerminalId = getTerminalIdFromEvt(evt);

    // Only handle events for this terminal
    if (evtTerminalId != terminalId) {
      debugPrint(
          '[TerminalModel] Ignoring event for terminal $evtTerminalId (not mine)');
      return;
    }

    switch (type) {
      case 'opened':
        _handleTerminalOpened(evt);
        break;
      case 'data':
        _handleTerminalData(evt);
        break;
      case 'closed':
        _handleTerminalClosed(evt);
        break;
      case 'error':
        _handleTerminalError(evt);
        break;
    }
  }

  void _handleTerminalOpened(Map<String, dynamic> evt) {
    final bool success = getSuccessFromEvt(evt);
    final String message = evt['message']?.toString() ?? '';
    final String? serviceId = evt['service_id']?.toString();

    debugPrint(
        '[TerminalModel] Terminal opened response: success=$success, message=$message, service_id=$serviceId');

    if (success) {
      _terminalOpened = true;

      // On reconnect, the server may replay recent output. That replay can include
      // terminal queries like DSR/DA; xterm answers them through onOutput as
      // "^[[1;1R^[[2;2R^[[>0;0;0c", which must not be sent back to the peer.
      final replayTerminalOutput = evt['replay_terminal_output'];
      _suppressNextTerminalDataOutput = replayTerminalOutput == true ||
          message == 'Reconnected to existing terminal with pending output';

      // Fallback: if terminal view is not yet ready but already has valid
      // dimensions (e.g. layout completed before open response arrived),
      // mark view ready now to avoid output stuck in buffer indefinitely.
      if (!_terminalViewReady &&
          terminal.viewWidth > 0 &&
          terminal.viewHeight > 0) {
        _scheduleMarkViewReady();
      }

      // Process any buffered input
      _processBufferedInputAsync().then((_) {
        notifyListeners();
      }).catchError((e) {
        debugPrint('[TerminalModel] Error processing buffered input: $e');
        notifyListeners();
      });

      final persistentSessions =
          (evt['persistent_sessions'] as List<dynamic>? ?? [])
              .whereType<int>()
              .where((id) => !parent.terminalModels.containsKey(id))
              .toList();
      if (kWindowId != null && persistentSessions.isNotEmpty) {
        DesktopMultiWindow.invokeMethod(
            kWindowId!,
            kWindowEventRestoreTerminalSessions,
            jsonEncode({
              'peer_id': id,
              'persistent_sessions': persistentSessions,
            }));
      }
    } else {
      _writeToTerminal('Failed to open terminal: $message\r\n');
    }
  }

  Future<void> _processBufferedInputAsync() async {
    final buffer = List<String>.from(_inputBuffer);
    _inputBuffer.clear();

    for (final data in buffer) {
      try {
        await bind.sessionSendTerminalInput(
          sessionId: parent.sessionId,
          terminalId: terminalId,
          data: data,
        );
      } catch (e) {
        debugPrint('[TerminalModel] Error sending buffered input: $e');
      }
    }
  }

  void _handleTerminalData(Map<String, dynamic> evt) {
    final data = evt['data'];

    if (data != null) {
      final suppressTerminalOutput = _suppressNextTerminalDataOutput;
      _suppressNextTerminalDataOutput = false;
      try {
        String text = '';
        if (data is String) {
          // Try to decode as base64 first
          try {
            final bytes = base64Decode(data);
            text = utf8.decode(bytes, allowMalformed: true);
          } catch (e) {
            // If base64 decode fails, treat as plain text
            text = data;
          }
        } else if (data is List) {
          // Handle if data comes as byte array
          text = utf8.decode(List<int>.from(data), allowMalformed: true);
        } else {
          debugPrint('[TerminalModel] Unknown data type: ${data.runtimeType}');
          return;
        }

        _writeToTerminal(text, suppressTerminalOutput: suppressTerminalOutput);
      } catch (e) {
        debugPrint('[TerminalModel] Failed to process terminal data: $e');
      }
    }
  }

  /// Write text to terminal, buffering if the view is not yet ready.
  /// All terminal output should go through this method to avoid NaN errors
  /// from writing before the terminal view has valid layout dimensions.
  void _writeToTerminal(
    String text, {
    bool suppressTerminalOutput = false,
  }) {
    if (!_terminalViewReady) {
      // If a single chunk exceeds the cap, keep only its tail.
      // Note: truncation may split a multi-byte ANSI escape sequence,
      // which can cause a brief visual glitch on flush. This is acceptable
      // because it only affects the pre-layout buffering window and the
      // terminal will self-correct on subsequent output.
      if (text.length >= _kMaxOutputBufferChars) {
        final truncated = text.substring(text.length - _kMaxOutputBufferChars);
        _pendingOutputChunks
          ..clear()
          ..add(truncated);
        _pendingOutputSuppressFlags
          ..clear()
          ..add(suppressTerminalOutput);
        _pendingOutputSize = truncated.length;
      } else {
        _pendingOutputChunks.add(text);
        _pendingOutputSuppressFlags.add(suppressTerminalOutput);
        _pendingOutputSize += text.length;
        // Drop oldest chunks if exceeds limit (whole chunks to preserve ANSI sequences)
        while (_pendingOutputSize > _kMaxOutputBufferChars &&
            _pendingOutputChunks.length > 1) {
          final removed = _pendingOutputChunks.removeAt(0);
          _pendingOutputSuppressFlags.removeAt(0);
          _pendingOutputSize -= removed.length;
        }
      }
      return;
    }
    _writeTerminalChunk(text, suppressTerminalOutput: suppressTerminalOutput);
  }

  void _flushOutputBuffer() {
    if (_pendingOutputChunks.isEmpty) return;
    debugPrint(
        '[TerminalModel] Flushing $_pendingOutputSize buffered chars (${_pendingOutputChunks.length} chunks)');
    for (var i = 0; i < _pendingOutputChunks.length; i++) {
      _writeTerminalChunk(
        _pendingOutputChunks[i],
        suppressTerminalOutput: _pendingOutputSuppressFlags[i],
      );
    }
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
  }

  void _writeTerminalChunk(
    String text, {
    required bool suppressTerminalOutput,
  }) {
    if (!suppressTerminalOutput) {
      terminal.write(text);
      return;
    }
    final previous = _suppressTerminalOutput;
    _suppressTerminalOutput = true;
    try {
      terminal.write(text);
    } finally {
      _suppressTerminalOutput = previous;
    }
  }

  /// Mark terminal view as ready and flush buffered output.
  void _scheduleMarkViewReady() {
    if (_disposed || _terminalViewReady || _markViewReadyScheduled) return;
    _markViewReadyScheduled = true;
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _markViewReadyScheduled = false;
      if (_disposed || _terminalViewReady) return;
      if (terminal.viewWidth > 0 && terminal.viewHeight > 0) {
        _markViewReady();
      }
    });
    WidgetsBinding.instance.ensureVisualUpdate();
  }

  void _markViewReady() {
    if (_terminalViewReady) return;
    _terminalViewReady = true;
    _flushOutputBuffer();
  }

  void _handleTerminalClosed(Map<String, dynamic> evt) {
    final int exitCode = getExitCodeFromEvt(evt);
    _writeToTerminal('\r\nTerminal closed with exit code: $exitCode\r\n');
    _terminalOpened = false;
    notifyListeners();
    // Auto-close the tab/page
    onClosed?.call();
  }

  void _handleTerminalError(Map<String, dynamic> evt) {
    final String message = evt['message'] ?? 'Unknown error';
    _writeToTerminal('\r\nTerminal error: $message\r\n');
  }

  @override
  void dispose() {
    if (_disposed) return;
    _disposed = true;
    // Clear buffers to free memory
    _inputBuffer.clear();
    _pendingOutputChunks.clear();
    _pendingOutputSuppressFlags.clear();
    _pendingOutputSize = 0;
    _markViewReadyScheduled = false;
    _suppressNextTerminalDataOutput = false;
    // Terminal cleanup is handled server-side when service closes
    super.dispose();
  }
}
