import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/desktop/widgets/tabbar_widget.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:xterm/xterm.dart';
import 'terminal_connection_manager.dart';

class TerminalPage extends StatefulWidget {
  const TerminalPage({
    Key? key,
    required this.id,
    required this.password,
    required this.tabController,
    required this.isSharedPassword,
    required this.terminalId,
    this.forceRelay,
    this.connToken,
  }) : super(key: key);
  final String id;
  final String? password;
  final DesktopTabController tabController;
  final bool? forceRelay;
  final bool? isSharedPassword;
  final String? connToken;
  final int terminalId;

  @override
  State<TerminalPage> createState() => _TerminalPageState();
}

class _TerminalPageState extends State<TerminalPage>
    with AutomaticKeepAliveClientMixin {
  late FFI _ffi;
  late TerminalModel _terminalModel;

  @override
  void initState() {
    super.initState();

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
      widget.tabController.onSelected?.call(widget.id);
      
      // Check if this is a new connection or additional terminal
      // Note: When a connection exists, the ref count will be > 1 after this terminal is added
      final isExistingConnection = TerminalConnectionManager.hasConnection(widget.id) && 
          TerminalConnectionManager.getTerminalCount(widget.id) > 1;
      
      if (!isExistingConnection) {
        // First terminal - show loading dialog, wait for onReady
        _ffi.dialogManager
            .showLoading(translate('Connecting...'), onCancel: closeConnection);
      } else {
        // Additional terminal - connection already established
        // Open the terminal directly
        _terminalModel.openTerminal();
      }
    });
  }

  @override
  void dispose() {
    // Unregister terminal model from FFI
    _ffi.unregisterTerminalModel(widget.terminalId);
    _terminalModel.dispose();
    // Release connection reference instead of closing directly
    TerminalConnectionManager.releaseConnection(widget.id);
    super.dispose();
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

  @override
  bool get wantKeepAlive => true;
}
