import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/terminal_model.dart';
import 'package:xterm/xterm.dart';
import 'terminal_connection_manager.dart';

/// An embeddable terminal panel that shows one or more persistent terminal
/// tabs over a single shared terminal connection to [peerId].
///
/// It reuses RustDesk's native terminal stack (TerminalModel + the
/// `terminal-persistent` option), so sessions survive disconnect/idle and
/// reattach on reconnect — no tmux/SSH wrapper, no server/proto changes.
class InlineTerminalPanel extends StatefulWidget {
  final String peerId;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;
  // Called when the user closes the last tab — the host closes the terminal UI
  // (we don't silently spawn a replacement session).
  final VoidCallback? onClose;

  const InlineTerminalPanel({
    Key? key,
    required this.peerId,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
    this.onClose,
  }) : super(key: key);

  @override
  State<InlineTerminalPanel> createState() => _InlineTerminalPanelState();
}

class _TerminalTab {
  final int id;
  final TerminalModel model;
  final FocusNode focusNode;
  bool ready = false;
  // True once the remote shell has exited (e.g. the user typed `exit`).
  bool closed = false;
  VoidCallback? listener;

  _TerminalTab({
    required this.id,
    required this.model,
    required this.focusNode,
  });
}

class _InlineTerminalPanelState extends State<InlineTerminalPanel> {
  // Offset terminal ids to avoid colliding with a standalone terminal window
  // that may share the same per-peer connection.
  static const int _baseTerminalId = 900;

  late final FFI _ffi;
  final List<_TerminalTab> _tabs = [];
  int _selectedTabIndex = 0;
  int _nextTabId = 0;
  // True once the shared connection has come up (first terminal opened).
  bool _connReady = false;
  // Real terminal cell height (px), reported by the model on resize; used for
  // vertical padding instead of a hardcoded guess.
  double _cellHeight = 18.0;

  // A calm, readable One Dark-inspired palette for a tidy, "Termius-like" look
  // (intentionally no settings UI). Background matches the panel chrome so the
  // split view stays cohesive.
  static const TerminalTheme _theme = TerminalTheme(
    cursor: Color(0xFF61AFEF),
    selection: Color(0x553B4252),
    foreground: Color(0xFFD7DAE0),
    background: Color(0xFF1E1E1E),
    black: Color(0xFF21252B),
    red: Color(0xFFE06C75),
    green: Color(0xFF98C379),
    yellow: Color(0xFFE5C07B),
    blue: Color(0xFF61AFEF),
    magenta: Color(0xFFC678DD),
    cyan: Color(0xFF56B6C2),
    white: Color(0xFFABB2BF),
    brightBlack: Color(0xFF5C6370),
    brightRed: Color(0xFFE06C75),
    brightGreen: Color(0xFF98C379),
    brightYellow: Color(0xFFE5C07B),
    brightBlue: Color(0xFF61AFEF),
    brightMagenta: Color(0xFFC678DD),
    brightCyan: Color(0xFF56B6C2),
    brightWhite: Color(0xFFFFFFFF),
    searchHitBackground: Color(0xFFFFFF2B),
    searchHitBackgroundCurrent: Color(0xFF31FF26),
    searchHitForeground: Color(0xFF000000),
  );

  // Slightly larger than xterm's default (13) with comfortable line height for
  // legibility on a phone-sized split view.
  static const TerminalStyle _textStyle =
      TerminalStyle(fontSize: 14, height: 1.3);

  // Show the on-screen special-keys bar (Esc/Ctrl/Alt/arrows/…). Honours the
  // same option as the stock mobile terminal (default on).
  late final bool _showExtraKeys;

  // Sticky modifiers (Termius-style): tap Ctrl/Alt to arm it, the next key —
  // from this bar OR the system keyboard — combines with it, then it releases.
  bool _ctrlActive = false;
  bool _altActive = false;

  @override
  void initState() {
    super.initState();
    _showExtraKeys = !isWebDesktop &&
        mainGetLocalBoolOptionSync(kOptionEnableShowTerminalExtraKeys);
    // Establish the terminal connection exactly like the stock mobile terminal
    // (peer_card -> connect(isTerminal:true) -> TerminalPage): a plain
    // getConnection + registered TerminalModel. No connToken / persistence
    // toggle / event-callback routing here — those broke the connection.
    _ffi = TerminalConnectionManager.getConnection(
      peerId: widget.peerId,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
    );
    // If we reused an already-connected FFI (e.g. a second panel on the same
    // peer), peer_info won't fire again, so the FFI "ready" event that normally
    // drives the first OpenTerminal never comes. Seed _connReady from the live
    // connection so the first tab opens directly instead of hanging on
    // "Connecting…".
    _connReady = _ffi.ffiModel.pi.isSet.value;
    _ensurePersistent();
    _addTab();
  }

  /// Enable RustDesk's native persistent-terminal option so sessions survive
  /// disconnect/idle and reattach on reconnect. This only flips a local config
  /// bool + queues an option message; it does NOT restart the connection.
  void _ensurePersistent() {
    try {
      final on = bind.sessionGetToggleOptionSync(
        sessionId: _ffi.sessionId,
        arg: kOptionTerminalPersistent,
      );
      if (!on) {
        bind.sessionToggleOption(
          sessionId: _ffi.sessionId,
          value: kOptionTerminalPersistent,
        );
      }
    } catch (e) {
      debugPrint('[InlineTerminalPanel] Failed to enable persistence: $e');
    }
  }

  @override
  void dispose() {
    for (final tab in _tabs) {
      _disposeTab(tab);
    }
    _tabs.clear();
    // Release this panel's single reference to the shared connection.
    TerminalConnectionManager.releaseConnection(widget.peerId);
    super.dispose();
  }

  void _disposeTab(_TerminalTab tab) {
    if (tab.listener != null) {
      tab.model.removeListener(tab.listener!);
    }
    _ffi.unregisterTerminalModel(tab.id, tab.model);
    tab.model.dispose();
    tab.focusNode.dispose();
  }

  void _addTab() {
    _addTabWithId(_baseTerminalId + _nextTabId);
    _nextTabId++;
  }

  /// Create a tab bound to a specific server-side terminal_id. Used for new
  /// tabs and for restoring surviving persistent sessions after a reconnect.
  /// All tabs share ONE authenticated connection (no per-tab re-login).
  void _addTabWithId(int terminalId, {bool selectNew = true}) {
    if (_tabs.any((t) => t.id == terminalId)) return; // already shown
    final model = TerminalModel(_ffi, terminalId);
    // Focusable from birth so the tab can always receive keyboard input.
    final focusNode = FocusNode();

    // Track the real cell height; only rebuild on an actual change (not every
    // resize frame) so the padding stays correct without churn.
    model.onResizeExternal = (w, h, pw, ph) {
      if (ph > 0 && _cellHeight != ph) {
        _cellHeight = ph.toDouble();
        if (mounted) setState(() {});
      }
    };
    // Surface other surviving sessions so we can restore them as tabs.
    model.onPersistentSessions = _restorePersistentSessions;
    // Apply sticky Ctrl/Alt to keystrokes coming from the system keyboard.
    model.inputTransform = _applyModifiers;

    final tab = _TerminalTab(
      id: terminalId,
      model: model,
      focusNode: focusNode,
    );

    tab.listener = () {
      if (!mounted) return;
      if (model.terminalOpened) {
        _connReady = true;
        if (!tab.ready || tab.closed) {
          setState(() {
            tab.ready = true;
            tab.closed = false;
          });
          // Grab the keyboard once the selected tab is actually up.
          if (_tabs.isNotEmpty &&
              _selectedTabIndex < _tabs.length &&
              _tabs[_selectedTabIndex] == tab) {
            _focusSelected();
          }
        }
      } else if (tab.ready && !tab.closed) {
        // The terminal reported closed. Show the "Session closed / Restart"
        // banner but KEEP the tab — a `closed` may be spurious (saturated
        // output channel, transient reconnect), and we must never make a
        // session vanish on its own. The tab goes away only via an explicit ×
        // (with confirm); Restart reattaches/relaunches in place.
        setState(() => tab.closed = true);
      }
    };
    model.addListener(tab.listener!);

    // Registering lets the FFI drive open()/reattach() on connect AND on
    // reconnect (re-sends OpenTerminal(force) → reattaches to the persistent
    // session and replays output).
    _ffi.registerTerminalModel(terminalId, model);
    _tabs.add(tab);
    if (selectNew) _selectedTabIndex = _tabs.length - 1;

    // The FFI "ready" event only opens models registered before it fired. A tab
    // added after the connection is already up must be opened directly (same
    // connection, no re-login). Guard against the tab being removed meanwhile.
    if (_connReady) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted && _tabs.contains(tab)) model.onReady();
      });
    }

    if (mounted) setState(() {});
    if (selectNew) _focusSelected();
  }

  /// On (re)connect the server reports surviving persistent session ids; show
  /// each as a tab so you can switch to whichever you want. Cascades until all
  /// survivors are restored; ids already shown are skipped.
  void _restorePersistentSessions(List<int> ids) {
    // We are inside a successful 'opened' callback → the shared connection is
    // up. Mark it ready so restored tabs are opened directly (they must send
    // OpenTerminal to actually reattach; otherwise input is silently buffered).
    _connReady = true;
    for (final id in ids) {
      if (_tabs.any((t) => t.id == id)) continue;
      final offset = id - _baseTerminalId;
      if (offset >= _nextTabId) _nextTabId = offset + 1; // avoid id collisions
      _addTabWithId(id, selectNew: false);
    }
  }

  Future<void> _closeTab(int index) async {
    if (index < 0 || index >= _tabs.length) return;
    final tab = _tabs[index];
    // Confirm only when killing a session that is actually alive.
    if (tab.ready && !tab.closed) {
      final confirmed = await showDialog<bool>(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text(translate('Close')),
          content: Text('${translate('Close')} "Tab ${index + 1}"?'),
          actions: [
            TextButton(
                onPressed: () => Navigator.pop(ctx, false),
                child: Text(translate('Cancel'))),
            TextButton(
                onPressed: () => Navigator.pop(ctx, true),
                child: Text(translate('OK'))),
          ],
        ),
      );
      if (confirmed != true || !mounted) return;
    }
    // An explicit × means "end this session for good", so always reap it
    // server-side (force covers a closed/hung tab whose model isn't "opened").
    await _removeTab(tab, reap: true);
  }

  /// Dispose a tab and drop it from the bar, optionally reaping its server-side
  /// session first. Removing the last tab calls onClose (the host closes the
  /// terminal UI) rather than spawning a replacement. closeTerminal is awaited
  /// before dispose so its post-RPC notifyListeners() can't hit a disposed
  /// ChangeNotifier.
  Future<void> _removeTab(_TerminalTab tab, {bool reap = false}) async {
    if (reap) {
      await tab.model.closeTerminal(force: true);
      if (!mounted) return;
    }
    // Re-find by identity — _tabs may have changed during the await.
    final i = _tabs.indexOf(tab);
    if (i < 0) return;
    _disposeTab(tab);
    _tabs.removeAt(i);
    if (_selectedTabIndex >= _tabs.length) {
      _selectedTabIndex = _tabs.isEmpty ? 0 : _tabs.length - 1;
    }
    if (mounted) setState(() {});
    if (_tabs.isEmpty) {
      // No silent replacement session — let the host close the terminal UI.
      widget.onClose?.call();
    } else {
      _focusSelected();
    }
  }

  /// Move keyboard focus to the selected tab's terminal (after the next frame,
  /// once its view is laid out). Keeps typing going to the visible terminal.
  void _focusSelected() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted || _tabs.isEmpty) return;
      final i = _selectedTabIndex.clamp(0, _tabs.length - 1);
      _tabs[i].focusNode.requestFocus();
    });
  }

  EdgeInsets _calculatePadding(double heightPx) {
    const defaultPadding = EdgeInsets.symmetric(horizontal: 5.0, vertical: 2.0);
    final cell = _cellHeight > 0 ? _cellHeight : 18.0;
    final rows = (heightPx / cell).floor();
    if (rows <= 0) return defaultPadding;
    final extraSpace = heightPx - rows * cell;
    if (!extraSpace.isFinite || extraSpace < 0) return defaultPadding;
    return EdgeInsets.symmetric(
      horizontal: defaultPadding.horizontal / 2,
      vertical: extraSpace / 2.0,
    );
  }

  @override
  Widget build(BuildContext context) {
    final hasTabs = _tabs.isNotEmpty;
    final selIndex =
        hasTabs ? _selectedTabIndex.clamp(0, _tabs.length - 1) : 0;

    return Container(
      color: const Color(0xFF1E1E1E),
      child: Column(
        children: [
          _buildTabBar(),
          if (hasTabs)
            Expanded(
              // One TerminalView per tab, all kept alive. Switching tabs only
              // changes which is shown — we never swap a terminal underneath a
              // single view (that broke repaint/focus and dropped keystrokes).
              child: IndexedStack(
                index: selIndex,
                sizing: StackFit.expand,
                children: [
                  for (final tab in _tabs)
                    KeyedSubtree(
                      key: ValueKey(tab.id),
                      child: _buildTerminalView(tab),
                    ),
                ],
              ),
            ),
          if (_showExtraKeys && hasTabs) _buildExtraKeys(_tabs[selIndex]),
        ],
      ),
    );
  }

  // A single tab's terminal view (kept alive inside the IndexedStack). Focus is
  // managed explicitly via _focusSelected, so autofocus stays off here (else the
  // offstage tabs would fight over focus).
  Widget _buildTerminalView(_TerminalTab tab) {
    return LayoutBuilder(
      builder: (context, constraints) {
        final view = TerminalView(
          tab.model.terminal,
          controller: tab.model.terminalController,
          focusNode: tab.focusNode,
          autofocus: false,
          theme: _theme,
          textStyle: _textStyle,
          backgroundOpacity: 0.7,
          padding: _calculatePadding(constraints.maxHeight),
          onSecondaryTapDown: (details, offset) async {
            final selection = tab.model.terminalController.selection;
            if (selection != null) {
              final text = tab.model.terminal.buffer.getText(selection);
              tab.model.terminalController.clearSelection();
              await Clipboard.setData(ClipboardData(text: text));
            } else {
              final data = await Clipboard.getData('text/plain');
              final text = data?.text;
              if (text != null) {
                tab.model.terminal.paste(text);
              }
            }
          },
        );
        return RepaintBoundary(
          child: Stack(
            children: [
              view,
              if (!tab.ready && !tab.closed)
                Positioned.fill(child: _connectingView()),
              if (tab.closed)
                Positioned(
                  left: 0,
                  right: 0,
                  bottom: 0,
                  child: _closedBanner(tab),
                ),
            ],
          ),
        );
      },
    );
  }

  // Escape sequences for the bar's non-printable keys.
  static const Map<String, String> _keySequences = {
    'Esc': '\x1B',
    'Tab': '\t',
    '↑': '\x1B[A',
    '↓': '\x1B[B',
    '→': '\x1B[C',
    '←': '\x1B[D',
    'Home': '\x1B[H',
    'End': '\x1B[F',
    'PgUp': '\x1B[5~',
    'PgDn': '\x1B[6~',
  };

  // A Termius-style accessory bar: one scrollable row docked above the system
  // keyboard, with sticky Ctrl/Alt that highlight while armed and combine with
  // the next key (this bar's or the system keyboard's).
  Widget _buildExtraKeys(_TerminalTab tab) {
    return Container(
      decoration: const BoxDecoration(
        color: Color(0xFF161618),
        border: Border(top: BorderSide(color: Color(0xFF333336), width: 1)),
      ),
      padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 6),
      child: SingleChildScrollView(
        scrollDirection: Axis.horizontal,
        physics: const BouncingScrollPhysics(),
        child: Row(
          children: [
            _keyCap(tab, label: 'esc', onTap: () => _sendKey(tab, 'Esc')),
            _keyCap(tab, label: 'ctrl', active: _ctrlActive, onTap: () => _toggleMod(ctrl: true)),
            _keyCap(tab, label: 'alt', active: _altActive, onTap: () => _toggleMod(ctrl: false)),
            _keyCap(tab, label: 'tab', onTap: () => _sendKey(tab, 'Tab')),
            _barSeparator(),
            _keyCap(tab, icon: Icons.west, onTap: () => _sendKey(tab, '←')),
            _keyCap(tab, icon: Icons.north, onTap: () => _sendKey(tab, '↑')),
            _keyCap(tab, icon: Icons.south, onTap: () => _sendKey(tab, '↓')),
            _keyCap(tab, icon: Icons.east, onTap: () => _sendKey(tab, '→')),
            _barSeparator(),
            for (final s in const ['-', '/', '|', '~', '`'])
              _keyCap(tab, label: s, onTap: () => _sendKey(tab, s)),
            _barSeparator(),
            for (final k in const ['Home', 'End', 'PgUp', 'PgDn'])
              _keyCap(tab, label: k, onTap: () => _sendKey(tab, k)),
          ],
        ),
      ),
    );
  }

  Widget _barSeparator() => Container(
        width: 1,
        height: 18,
        margin: const EdgeInsets.symmetric(horizontal: 7),
        color: const Color(0xFF38383B),
      );

  Widget _keyCap(
    _TerminalTab tab, {
    String? label,
    IconData? icon,
    bool active = false,
    required VoidCallback onTap,
  }) {
    final fg = active ? Colors.white : const Color(0xFFCED0D4);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 3),
      child: Material(
        color: active ? const Color(0xFF3B6FE0) : const Color(0xFF2B2B2E),
        borderRadius: BorderRadius.circular(8),
        child: InkWell(
          borderRadius: BorderRadius.circular(8),
          // Never take focus from the terminal — otherwise the soft keyboard
          // closes and the armed modifier can't combine with the next key.
          canRequestFocus: false,
          onTap: () {
            onTap();
            // Keep the terminal focused so the keyboard stays up and the next
            // system-keyboard key reaches it (with the modifier applied).
            if (!tab.focusNode.hasFocus) tab.focusNode.requestFocus();
          },
          child: Container(
            height: 34,
            constraints: const BoxConstraints(minWidth: 42),
            alignment: Alignment.center,
            padding: const EdgeInsets.symmetric(horizontal: 11),
            child: icon != null
                ? Icon(icon, size: 17, color: fg)
                : Text(
                    label!,
                    style: TextStyle(
                      color: fg,
                      fontSize: 13,
                      height: 1.0,
                      fontWeight: active ? FontWeight.w700 : FontWeight.w500,
                    ),
                  ),
          ),
        ),
      ),
    );
  }

  void _toggleMod({required bool ctrl}) {
    setState(() {
      if (ctrl) {
        _ctrlActive = !_ctrlActive;
      } else {
        _altActive = !_altActive;
      }
    });
  }

  void _sendKey(_TerminalTab tab, String label) {
    final raw = _keySequences[label] ?? label;
    tab.model.sendVirtualKey(_applyModifiers(raw));
  }

  /// Apply any armed sticky modifier to [data], then release it (one-shot).
  /// Set as each model's inputTransform, so it also catches the system keyboard.
  String _applyModifiers(String data) {
    if (!_ctrlActive && !_altActive) return data;
    var out = data;
    if (_ctrlActive) out = _ctrlTransform(out);
    if (_altActive) out = '\x1B$out'; // Alt = ESC prefix
    _ctrlActive = false;
    _altActive = false;
    // We may be inside an input event; update the highlight next frame.
    if (mounted) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (mounted) setState(() {});
      });
    }
    return out;
  }

  // Map a single character to its Ctrl- control code (Ctrl+C -> 0x03, etc.).
  // Multi-char input (e.g. arrow sequences) is left unchanged.
  static String _ctrlTransform(String data) {
    if (data.length != 1) return data;
    final c = data.codeUnitAt(0);
    if (c >= 0x61 && c <= 0x7A) return String.fromCharCode(c - 0x60); // a-z
    if (c >= 0x41 && c <= 0x5A) return String.fromCharCode(c - 0x40); // A-Z
    if (c >= 0x5B && c <= 0x5F) return String.fromCharCode(c - 0x40); // [ \ ] ^ _
    if (c == 0x20) return '\x00'; // Ctrl+Space -> NUL
    return data;
  }

  Widget _closedBanner(_TerminalTab tab) {
    return Container(
      color: Colors.black.withOpacity(0.65),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(Icons.cancel, size: 14, color: Colors.grey.shade400),
          const SizedBox(width: 8),
          Text(translate('Session closed'),
              style: TextStyle(color: Colors.grey.shade300, fontSize: 12)),
          const SizedBox(width: 8),
          TextButton(
            onPressed: () => tab.model.openTerminal(force: true),
            child: Text(translate('Restart')),
          ),
        ],
      ),
    );
  }

  Widget _connectingView() {
    return Container(
      color: const Color(0xFF1E1E1E),
      child: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 22,
              height: 22,
              child: CircularProgressIndicator(
                strokeWidth: 2,
                color: Colors.grey.shade500,
              ),
            ),
            const SizedBox(height: 12),
            Text(
              '${translate('Connecting')}...',
              style: TextStyle(color: Colors.grey.shade400, fontSize: 12),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTabBar() {
    return Container(
      height: 36,
      decoration: BoxDecoration(
        color: const Color(0xFF2D2D2D),
        border: Border(
          bottom: BorderSide(color: Colors.grey.shade800, width: 1),
        ),
      ),
      child: Row(
        children: [
          const SizedBox(width: 8),
          Icon(Icons.terminal, size: 16, color: Colors.grey.shade400),
          // Persistent-session indicator: sessions survive reconnect/idle.
          const SizedBox(width: 6),
          Tooltip(
            message: translate('Keep terminal sessions on disconnect'),
            child: Icon(Icons.push_pin, size: 12, color: Colors.green.shade400),
          ),
          const SizedBox(width: 8),
          Expanded(
            child: ListView.builder(
              scrollDirection: Axis.horizontal,
              itemCount: _tabs.length,
              itemBuilder: (context, index) => _buildTab(index),
            ),
          ),
          IconButton(
            icon: Icon(Icons.add, size: 16, color: Colors.grey.shade400),
            padding: EdgeInsets.zero,
            constraints: const BoxConstraints(),
            onPressed: _addTab,
            tooltip: translate('New tab'),
          ),
          const SizedBox(width: 8),
        ],
      ),
    );
  }

  Widget _buildTab(int index) {
    final tab = _tabs[index];
    final isSelected = index == _selectedTabIndex;
    return GestureDetector(
      onTap: () {
        setState(() {
          _selectedTabIndex = index;
          // Don't carry an armed modifier across tabs.
          _ctrlActive = false;
          _altActive = false;
        });
        // Move the keyboard to the newly selected tab.
        _focusSelected();
      },
      child: Container(
        margin: const EdgeInsets.symmetric(horizontal: 2, vertical: 4),
        padding: const EdgeInsets.symmetric(horizontal: 10),
        decoration: BoxDecoration(
          color: isSelected ? const Color(0xFF3D3D3D) : Colors.transparent,
          borderRadius: BorderRadius.circular(4),
          border: isSelected
              ? Border.all(color: Colors.blue.shade400, width: 1)
              : null,
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (tab.closed)
              Icon(Icons.cancel, size: 10, color: Colors.grey.shade500)
            else if (!tab.ready)
              SizedBox(
                width: 8,
                height: 8,
                child: CircularProgressIndicator(
                  strokeWidth: 1,
                  color: Colors.grey.shade500,
                ),
              )
            else
              Icon(Icons.check_circle, size: 10, color: Colors.green.shade400),
            const SizedBox(width: 4),
            Text(
              // Positional label so it stays consistent after close/restore.
              'Tab ${index + 1}',
              style: TextStyle(
                color: isSelected ? Colors.white : Colors.grey.shade400,
                fontSize: 11,
                fontFamily: 'monospace',
              ),
            ),
            const SizedBox(width: 4),
            // Always offer a close affordance — even the last/only tab (a hung
            // session must be closable); closing the last tab closes the
            // terminal UI via onClose.
            GestureDetector(
              onTap: () => _closeTab(index),
              child: Icon(Icons.close, size: 12, color: Colors.grey.shade500),
            ),
          ],
        ),
      ),
    );
  }
}
