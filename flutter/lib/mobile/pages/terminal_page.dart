import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
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

  // For web only.
  // 'monospace' does not work on web, use Google Fonts, `??` is only for null safety.
  final String _robotoMonoFontFamily = isWeb
      ? (GoogleFonts.robotoMono().fontFamily ?? 'monospace')
      : 'monospace';

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
    return Scaffold(
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: TerminalView(
        _terminalModel.terminal,
        controller: _terminalModel.terminalController,
        autofocus: true,
        textStyle: _getTerminalStyle(),
        backgroundOpacity: 0.7,
        padding: const EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0),
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
    );
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
