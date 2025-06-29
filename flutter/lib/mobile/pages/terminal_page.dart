import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:get/get.dart';
import 'package:xterm/xterm.dart';

import '../../common.dart';
import '../../common/widgets/overlay.dart';
import '../../models/model.dart';
import '../../models/terminal_model.dart';
import '../../desktop/pages/terminal_connection_manager.dart';

class TerminalPage extends StatefulWidget {
  TerminalPage({
    Key? key,
    required this.id,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
  }) : super(key: key);

  final String id;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;

  @override
  State<TerminalPage> createState() => _TerminalPageState(id);
}

class _TerminalPageState extends State<TerminalPage>
    with WidgetsBindingObserver, TickerProviderStateMixin {
  Timer? _timer;
  bool _showBar = !isWebDesktop;
  Orientation? _currentOrientation;

  final _blockableOverlayState = BlockableOverlayState();
  final FocusNode _focusNode = FocusNode();

  late FFI _ffi;
  late TabController _tabController;
  final List<TerminalModel> _terminals = [];
  int _nextTerminalId = 1;

  _TerminalPageState(String id) {
    initSharedStates(id);
  }
  

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 0, vsync: this);

    // Get or create the FFI instance using connection manager
    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.id,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
      connToken: null,
    );

    WidgetsBinding.instance.addPostFrameCallback((_) {
      SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual, overlays: []);
      gFFI.dialogManager
          .showLoading(translate('Connecting...'), onCancel: closeConnection);
    });

    // No need to enable wakelock for terminal connections

    _focusNode.requestFocus();
    _blockableOverlayState.applyFfi(_ffi);

    WidgetsBinding.instance.addObserver(this);
    
    // Add the first terminal after connection is established
    WidgetsBinding.instance.addPostFrameCallback((_) {
      _addNewTerminal();
    });
  }

  void _addNewTerminal() async {
    // Check if remote supports terminal
    if (_terminals.isEmpty) {
      // Wait for connection to establish
      await Future.delayed(const Duration(milliseconds: 500));
    }
    
    final terminalId = _nextTerminalId++;
    final terminal = TerminalModel(_ffi, terminalId);
    
    // Register terminal model with FFI for event routing
    _ffi.registerTerminalModel(terminalId, terminal);
    
    setState(() {
      _terminals.add(terminal);
      _tabController = TabController(
        length: _terminals.length,
        vsync: this,
        initialIndex: _terminals.length - 1,
      );
    });
    
    // Open the terminal after adding it
    WidgetsBinding.instance.addPostFrameCallback((_) {
      terminal.openTerminal();
      gFFI.dialogManager.dismissAll();
    });
  }
  
  void _closeTerminal(int index) {
    if (_terminals.length <= 1) {
      // Don't close the last terminal
      return;
    }
    
    final terminal = _terminals[index];
    
    // Unregister terminal model from FFI
    _ffi.unregisterTerminalModel(terminal.terminalId);
    
    terminal.closeTerminal();
    
    setState(() {
      _terminals.removeAt(index);
      final newIndex = index > 0 ? index - 1 : 0;
      _tabController = TabController(
        length: _terminals.length,
        vsync: this,
        initialIndex: newIndex,
      );
    });
  }

  @override
  Future<void> dispose() async {
    WidgetsBinding.instance.removeObserver(this);
    _tabController.dispose();
    for (var terminal in _terminals) {
      // Unregister each terminal model from FFI
      _ffi.unregisterTerminalModel(terminal.terminalId);
      terminal.dispose();
    }
    super.dispose();
    // Release the connection reference
    TerminalConnectionManager.releaseConnection(widget.id);
    _timer?.cancel();
    await SystemChrome.setEnabledSystemUIMode(SystemUiMode.manual,
        overlays: SystemUiOverlay.values);
    // No need to disable wakelock since we didn't enable it
    removeSharedStates(widget.id);
  }

  Widget emptyOverlay(Color bgColor) => BlockableOverlay(
        state: _blockableOverlayState,
        underlying: Container(
          color: bgColor,
        ),
      );

  Widget _bottomWidget() => (_showBar
      ? BottomAppBar(
          elevation: 10,
          color: MyTheme.accent,
          child: Row(
            mainAxisSize: MainAxisSize.max,
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: <Widget>[
              Row(
                children: <Widget>[
                  IconButton(
                    color: Colors.white,
                    icon: const Icon(Icons.clear),
                    onPressed: () {
                      // closeConnection(sessionId, _ffi.dialogManager);
                    },
                  ),
                  IconButton(
                    color: Colors.white,
                    icon: const Icon(Icons.settings),
                    onPressed: showTerminalOptions,
                  ),
                  IconButton(
                    color: Colors.white,
                    icon: const Icon(Icons.add),
                    onPressed: _addNewTerminal,
                    tooltip: 'New Terminal',
                  ),
                ],
              ),
              IconButton(
                color: Colors.white,
                icon: const Icon(Icons.expand_more),
                onPressed: () {
                  setState(() => _showBar = !_showBar);
                },
              ),
            ],
          ),
        )
      : Offstage());

  void showTerminalOptions() {
    // TODO: 添加终端设置选项
  }

  @override
  Widget build(BuildContext context) {
    return WillPopScope(
      onWillPop: () async {
        // closeConnection(sessionId, _ffi.dialogManager);
        return false;
      },
      child: Scaffold(
        floatingActionButton: !_showBar
            ? FloatingActionButton(
                mini: true,
                child: const Icon(Icons.expand_less, color: Colors.white),
                backgroundColor: MyTheme.accent,
                onPressed: () {
                  setState(() => _showBar = !_showBar);
                },
              )
            : null,
        bottomNavigationBar: Obx(() => Stack(
              alignment: Alignment.bottomCenter,
              children: [
                _bottomWidget(),
              ],
            )),
        body: Obx(
          () => Overlay(
            initialEntries: [
              OverlayEntry(builder: (context) {
                return Container(
                  color: kColorCanvas,
                  child: SafeArea(
                    child: OrientationBuilder(builder: (ctx, orientation) {
                      if (_currentOrientation != orientation) {
                        Timer(const Duration(milliseconds: 200), () {
                          _currentOrientation = orientation;
                        });
                      }
                      return Container(
                        color: MyTheme.canvasColor,
                        child: Column(
                          children: [
                            if (_terminals.length > 1)
                              Container(
                                color: Theme.of(context).primaryColor,
                                child: TabBar(
                                  controller: _tabController,
                                  isScrollable: true,
                                  tabs: _terminals.asMap().entries.map((entry) {
                                    final index = entry.key;
                                    final terminal = entry.value;
                                    return Tab(
                                      child: Row(
                                        mainAxisSize: MainAxisSize.min,
                                        children: [
                                          Text('Terminal ${terminal.terminalId}'),
                                          if (_terminals.length > 1) ...[
                                            const SizedBox(width: 8),
                                            InkWell(
                                              onTap: () => _closeTerminal(index),
                                              child: const Icon(Icons.close, size: 16),
                                            ),
                                          ],
                                        ],
                                      ),
                                    );
                                  }).toList(),
                                ),
                              ),
                            Expanded(
                              child: TabBarView(
                                controller: _tabController,
                                children: _terminals.map((terminal) {
                                  return TerminalView(
                                    terminal.terminal,
                                    controller: terminal.terminalController,
                                    autofocus: true,
                                    backgroundOpacity: 0.7,
                                    onSecondaryTapDown: (details, offset) async {
                                      final selection = terminal.terminalController.selection;
                                      if (selection != null) {
                                        final text = terminal.terminal.buffer.getText(selection);
                                        terminal.terminalController.clearSelection();
                                        await Clipboard.setData(
                                            ClipboardData(text: text));
                                      } else {
                                        final data =
                                            await Clipboard.getData('text/plain');
                                        final text = data?.text;
                                        if (text != null) {
                                          terminal.terminal.paste(text);
                                        }
                                      }
                                    },
                                  );
                                }).toList(),
                              ),
                            ),
                          ],
                        ),
                      );
                    }),
                  ),
                );
              }),
            ],
          ),
        ),
      ),
    );
  }
}
