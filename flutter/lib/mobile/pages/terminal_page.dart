import 'dart:async';
import 'dart:math';
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
    with AutomaticKeepAliveClientMixin, WidgetsBindingObserver {
  late FFI _ffi;
  late TerminalModel _terminalModel;
  double? _cellHeight;
  double _sysKeyboardHeight = 0;
  Timer? _keyboardDebounce;

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

  EdgeInsets _calculatePadding(double heightPx) {
    if (_cellHeight == null) {
      return const EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);
    }
    final realHeight = heightPx - _sysKeyboardHeight;
    final rows = (realHeight / _cellHeight!).floor();
    final extraSpace = realHeight - rows * _cellHeight!;
    final topBottom = max(0.0, extraSpace / 2.0);
    return EdgeInsets.only(left: 5.0, right: 5.0, top: topBottom, bottom: topBottom + _sysKeyboardHeight);
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
    return Scaffold(
      resizeToAvoidBottomInset: false, // Disable automatic layout adjustment; manually control UI updates to prevent flickering when the keyboard shows/hides
      backgroundColor: Theme.of(context).scaffoldBackgroundColor,
      body: SafeArea(
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
