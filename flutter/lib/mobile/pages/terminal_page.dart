import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:xterm/xterm.dart';
import '../../desktop/pages/terminal_connection_manager.dart';

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
    with AutomaticKeepAliveClientMixin {
  late FFI _ffi;
  late TerminalModel _terminalModel;
  final GlobalKey _keyboardKey = GlobalKey();
  double _keyboardHeight = 0;

  // For web only.
  // 'monospace' does not work on web, use Google Fonts, `??` is only for null safety.
  final String _robotoMonoFontFamily = isWeb
      ? (GoogleFonts.robotoMono().fontFamily ?? 'monospace')
      : 'monospace';

  SessionID get sessionId => _ffi.sessionId;

  @override
  void initState() {
    super.initState();

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

    // Register this terminal model with FFI for event routing
    _ffi.registerTerminalModel(widget.terminalId, _terminalModel);

    // Initialize terminal connection
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _ffi.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);

      _updateKeyboardHeight();
    });
    _ffi.ffiModel.updateEventListener(_ffi.sessionId, widget.id);
  }

  @override
  void dispose() {
    // Unregister terminal model from FFI
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    super.dispose();
    TerminalConnectionManager.releaseConnection(widget.id);
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

  void _updateKeyboardHeight() {
    if (_keyboardKey.currentContext != null) {
      final renderBox = _keyboardKey.currentContext!.findRenderObject() as RenderBox;
      final newHeight = renderBox.size.height;
      if (newHeight != _keyboardHeight) {
        setState(() {
          _keyboardHeight = newHeight;
        });
      }
    }
  }

  Widget buildBody() {
    return Scaffold(
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: Stack(
        children: [
          Positioned.fill(
            child: TerminalView(
              _terminalModel.terminal,
              controller: _terminalModel.terminalController,
              autofocus: true,
              textStyle: _getTerminalStyle(),
              backgroundOpacity: 0.7,
              padding: EdgeInsets.only(left: 5.0, right: 5.0, top: 2.0, bottom: 2.0 + _keyboardHeight),
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
            ),
          ),
          _buildFloatingKeyboard(),
        ],
      ),
    );
  }

  Widget _buildFloatingKeyboard() {
    return AnimatedPositioned(
      duration: const Duration(milliseconds: 200),
      left: 0,
      right: 0,
      bottom: 0,
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
                _buildKeyButton(':'),
                const SizedBox(width: 2),
                _buildKeyButton('?'),
                const SizedBox(width: 2),
                _buildKeyButton('Home'),
                const SizedBox(width: 2),
                _buildKeyButton('↑'),
                const SizedBox(width: 2),
                _buildKeyButton('End'),
              ],
            ),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                _buildKeyButton('Tab'),
                const SizedBox(width: 2),
                _buildKeyButton('Ctrl+C'),
                const SizedBox(width: 2),
                _buildKeyButton('-'),
                const SizedBox(width: 2),
                _buildKeyButton('!'),
                const SizedBox(width: 2),
                _buildKeyButton('←'),
                const SizedBox(width: 2),
                _buildKeyButton('↓'),
                const SizedBox(width: 2),
                _buildKeyButton('→'),
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

      case '/':
        send = '/';
        break;
      case ':':
        send = ':';
        break;
      case '?':
        send = '?';
        break;
      case '-':
        send = '-';
        break;
      case '!':
        send = '!';
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
