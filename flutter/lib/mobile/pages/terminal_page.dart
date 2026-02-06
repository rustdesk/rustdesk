import 'dart:async';
import 'dart:math';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:xterm/xterm.dart';
import '../../desktop/pages/terminal_connection_manager.dart';
import '../../consts.dart';

class TerminalPage extends StatefulWidget {
  const TerminalPage({
    Key? key,
    required this.id,
    required this.password,
    required this.isSharedPassword,
    this.forceRelay,
    this.connToken,
  }) : super(key: key);
  final String id;
  final String? password;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;
  final terminalId = 0;

  @override
  State<TerminalPage> createState() => _TerminalPageState();
}

class _TerminalPageState extends State<TerminalPage>
    with AutomaticKeepAliveClientMixin, WidgetsBindingObserver {
  late FFI _ffi;
  late TerminalModel _terminalModel;
  double? _cellHeight;
  double _sysKeyboardHeight = 0;
  Timer? _keyboardDebounce;
  final GlobalKey _keyboardKey = GlobalKey();
  double _keyboardHeight = 0;
  late bool _showTerminalExtraKeys;
  // For iOS edge swipe gesture
  double _swipeStartX = 0;
  double _swipeCurrentX = 0;

  // For web only.
  // 'monospace' does not work on web, use Google Fonts, `??` is only for null safety.
  final String _robotoMonoFontFamily = isWeb
      ? (GoogleFonts.robotoMono().fontFamily ?? 'monospace')
      : 'monospace';

  SessionID get sessionId => _ffi.sessionId;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);

    debugPrint(
        '[TerminalPage] Initializing terminal ${widget.terminalId} for peer ${widget.id}');

    // Use shared FFI instance from connection manager
    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
      connToken: widget.connToken,
    );

    // Create terminal model with specific terminal ID
    _terminalModel = TerminalModel(_ffi, widget.terminalId);
    debugPrint(
        '[TerminalPage] Terminal model created for terminal ${widget.terminalId}');

    _terminalModel.onResizeExternal = (w, h, pw, ph) {
      _cellHeight = ph * 1.0;
    };

    // Register this terminal model with FFI for event routing
    _ffi.registerTerminalModel(widget.terminalId, _terminalModel);

    _showTerminalExtraKeys = mainGetLocalBoolOptionSync(kOptionEnableShowTerminalExtraKeys);
    // Initialize terminal connection
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);

      if (_showTerminalExtraKeys) {
        _updateKeyboardHeight();
      }
    });
    _ffi.ffiModel.updateEventListener(_ffi.sessionId, widget.id);
  }

  @override
  void dispose() {
    // Unregister terminal model from FFI
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    _keyboardDebounce?.cancel();
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
    TerminalConnectionManager.releaseConnection(widget.id);
  }

  @override
  void didChangeMetrics() {
    super.didChangeMetrics();

    _keyboardDebounce?.cancel();
    _keyboardDebounce = Timer(const Duration(milliseconds: 20), () {
      final bottomInset = MediaQuery.of(context).viewInsets.bottom;
      setState(() {
        _sysKeyboardHeight = bottomInset;
      });
    });
  }

  void _updateKeyboardHeight() {
    if (_keyboardKey.currentContext != null) {
      final renderBox = _keyboardKey.currentContext!.findRenderObject() as RenderBox;
      _keyboardHeight = renderBox.size.height;
    }
  }

  EdgeInsets _calculatePadding(double heightPx) {
    if (_cellHeight == null) {
      return const EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);
    }
    final realHeight = heightPx - _sysKeyboardHeight - _keyboardHeight;
    final rows = (realHeight / _cellHeight!).floor();
    final extraSpace = realHeight - rows * _cellHeight!;
    final topBottom = max(0.0, extraSpace / 2.0);
    return EdgeInsets.only(left: 5.0, right: 5.0, top: topBottom, bottom: topBottom + _sysKeyboardHeight + _keyboardHeight);
  }

  @override
  Widget build(BuildContext context) {
    super.build(context);
    return WillPopScope(
      onWillPop: () async {
        clientClose(sessionId, _ffi);
        return false; // Prevent default back behavior
      },
      child: buildBody(),
    );
  }

  Widget buildBody() {
    final scaffold = Scaffold(
      resizeToAvoidBottomInset: false, // Disable automatic layout adjustment; manually control UI updates to prevent flickering when the keyboard shows/hides
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: Stack(
        children: [
          Positioned.fill(
            child: SafeArea(
              top: true,
              child: LayoutBuilder(
                builder: (context, constraints) {
                  final heightPx = constraints.maxHeight;
                  return TerminalView(
                    _terminalModel.terminal,
                    controller: _terminalModel.terminalController,
                    autofocus: true,
                    textStyle: _getTerminalStyle(),
                    backgroundOpacity: 0.7,
                    // The following comment is from xterm.dart source code:
                    // Workaround to detect delete key for platforms and IMEs that do not
                    // emit a hardware delete event. Preferred on mobile platforms. [false] by
                    // default.
                    //
                    // Android works fine without this workaround.
                    deleteDetection: isIOS,
                    padding: _calculatePadding(heightPx),
                    onSecondaryTapDown: (details, offset) async {
                      final selection = _terminalModel.terminalController.selection;
                      if (selection != null) {
                        final text = _terminalModel.terminal.buffer.getText(selection);
                        _terminalModel.terminalController.clearSelection();
                        await Clipboard.setData(ClipboardData(text: text));
                      } else {
                        final data = await Clipboard.getData('text/plain');
                        final text = data?.text;
                        if (text != null) {
                          _terminalModel.terminal.paste(text);
                        }
                      }
                    },
                  );
                },
              ),
            ),
          ),
          if (_showTerminalExtraKeys) _buildFloatingKeyboard(),
          // iOS-style circular close button in top-right corner
          if (isIOS) _buildCloseButton(),
        ],
      ),
    );

    // Add iOS edge swipe gesture to exit (similar to Android back button)
    if (isIOS) {
      return LayoutBuilder(
        builder: (context, constraints) {
          final screenWidth = constraints.maxWidth;
          // Base thresholds on screen width but clamp to reasonable logical pixel ranges
          // Edge detection region: ~10% of width, clamped between 20 and 80 logical pixels
          final edgeThreshold = (screenWidth * 0.1).clamp(20.0, 80.0);
          // Required horizontal movement: ~25% of width, clamped between 80 and 300 logical pixels
          final swipeThreshold = (screenWidth * 0.25).clamp(80.0, 300.0);

          return RawGestureDetector(
            behavior: HitTestBehavior.translucent,
            gestures: <Type, GestureRecognizerFactory>{
              HorizontalDragGestureRecognizer: GestureRecognizerFactoryWithHandlers<HorizontalDragGestureRecognizer>(
                () => HorizontalDragGestureRecognizer(
                  debugOwner: this,
                  // Only respond to touch input, exclude mouse/trackpad
                  supportedDevices: kTouchBasedDeviceKinds,
                ),
                (HorizontalDragGestureRecognizer instance) {
                  instance
                    // Capture initial touch-down position (before touch slop)
                    ..onDown = (details) {
                      _swipeStartX = details.localPosition.dx;
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onUpdate = (details) {
                      _swipeCurrentX = details.localPosition.dx;
                    }
                    ..onEnd = (details) {
                      // Check if swipe started from left edge and moved right
                      if (_swipeStartX < edgeThreshold && (_swipeCurrentX - _swipeStartX) > swipeThreshold) {
                        clientClose(sessionId, _ffi);
                      }
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    }
                    ..onCancel = () {
                      _swipeStartX = 0;
                      _swipeCurrentX = 0;
                    };
                },
              ),
            },
            child: scaffold,
          );
        },
      );
    }

    return scaffold;
  }

  Widget _buildCloseButton() {
    return Positioned(
      top: 0,
      right: 0,
      child: SafeArea(
        minimum: const EdgeInsets.only(
          top: 16, // iOS standard margin
          right: 16, // iOS standard margin
        ),
        child: Semantics(
          button: true,
          label: translate('Close'),
          child: Container(
            width: 44, // iOS standard tap target size
            height: 44,
            decoration: BoxDecoration(
              color: Colors.black.withOpacity(0.5), // Half transparency
              shape: BoxShape.circle,
            ),
            child: Material(
              color: Colors.transparent,
              shape: const CircleBorder(),
              clipBehavior: Clip.antiAlias,
              child: InkWell(
                customBorder: const CircleBorder(),
                onTap: () {
                  clientClose(sessionId, _ffi);
                },
                child: Tooltip(
                  message: translate('Close'),
                  child: const Icon(
                    Icons.chevron_left, // iOS-style back arrow
                    color: Colors.white,
                    size: 28,
                  ),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildFloatingKeyboard() {
    return AnimatedPositioned(
      duration: const Duration(milliseconds: 200),
      left: 0,
      right: 0,
      bottom: _sysKeyboardHeight,
      child: Container(
        key: _keyboardKey,
        color: Theme.of(context).scaffoldBackgroundColor,
        padding: EdgeInsets.zero,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                _buildKeyButton('Esc'),
                const SizedBox(width: 2),
                _buildKeyButton('/'),
                const SizedBox(width: 2),
                _buildKeyButton('|'),
                const SizedBox(width: 2),
                _buildKeyButton('Home'),
                const SizedBox(width: 2),
                _buildKeyButton('↑'),
                const SizedBox(width: 2),
                _buildKeyButton('End'),
                const SizedBox(width: 2),
                _buildKeyButton('PgUp'),
              ],
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                _buildKeyButton('Tab'),
                const SizedBox(width: 2),
                _buildKeyButton('Ctrl+C'),
                const SizedBox(width: 2),
                _buildKeyButton('~'),
                const SizedBox(width: 2),
                _buildKeyButton('←'),
                const SizedBox(width: 2),
                _buildKeyButton('↓'),
                const SizedBox(width: 2),
                _buildKeyButton('→'),
                const SizedBox(width: 2),
                _buildKeyButton('PgDn'),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildKeyButton(String label) {
    return ElevatedButton(
      onPressed: () {
        _sendKeyToTerminal(label);
      },
      child: Text(label),
      style: ElevatedButton.styleFrom(
        minimumSize: const Size(48, 32),
        padding: EdgeInsets.zero,
        textStyle: const TextStyle(fontSize: 12),
        backgroundColor: Theme.of(context).colorScheme.surfaceVariant,
        foregroundColor: Theme.of(context).colorScheme.onSurfaceVariant,
      ),
    );
  }

  void _sendKeyToTerminal(String key) {
    String? send;

    switch (key) {
      case 'Esc':
        send = '\x1B';
        break;
      case 'Tab':
        send = '\t';
        break;
      case 'Ctrl+C':
        send = '\x03';
        break;

      case '↑':
        send = '\x1B[A';
        break;
      case '↓':
        send = '\x1B[B';
        break;
      case '→':
        send = '\x1B[C';
        break;
      case '←':
        send = '\x1B[D';
        break;

      case 'Home':
        send = '\x1B[H';
        break;
      case 'End':
        send = '\x1B[F';
        break;
      case 'PgUp':
        send = '\x1B[5~';
        break;
      case 'PgDn':
        send = '\x1B[6~';
        break;

      default:
        send = key;
        break;
    }

    if (send != null) {
      _terminalModel.sendVirtualKey(send);
    }
  }

  // https://github.com/TerminalStudio/xterm.dart/issues/42#issuecomment-877495472
  // https://github.com/TerminalStudio/xterm.dart/issues/198#issuecomment-2526548458
  TerminalStyle _getTerminalStyle() {
    return isWeb
        ? TerminalStyle(
            fontFamily: _robotoMonoFontFamily,
            fontSize: 14,
          )
        : const TerminalStyle();
  }

  @override
  bool get wantKeepAlive => true;
}
