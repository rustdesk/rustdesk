import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/widgets/autocomplete.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/peer_model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:url_launcher/url_launcher.dart';

import 'home_page.dart';

final _androidTunnelController = AndroidTunnelController();
const _tunnelNotesOptionKey = 'rd_tunnel_notes_v1';

class _TunnelForward {
  final int localPort;
  final String remoteHost;
  final int remotePort;
  final String note;

  const _TunnelForward({
    required this.localPort,
    required this.remoteHost,
    required this.remotePort,
    this.note = '',
  });

  factory _TunnelForward.fromJson(
    List<dynamic> json, {
    String note = '',
  }) {
    return _TunnelForward(
      localPort: json[0] as int,
      remoteHost: json[1] as String,
      remotePort: json[2] as int,
      note: note,
    );
  }

  bool sameEndpoint(_TunnelForward other) =>
      localPort == other.localPort &&
      remoteHost == other.remoteHost &&
      remotePort == other.remotePort;
}

class AndroidTunnelController extends ChangeNotifier {
  FFI? _ffi;
  Socket? _probeSocket;
  bool running = false;
  String peerId = '';
  List<_TunnelForward> forwards = <_TunnelForward>[];
  bool? secure;
  bool? direct;
  bool? mux;
  bool authPending = false;
  String streamType = '';
  String peerVersion = '';
  String errorText = '';
  bool _closingAfterFailure = false;

  String localUrl(_TunnelForward forward) =>
      'http://127.0.0.1:${forward.localPort}';

  bool get failed => errorText.isNotEmpty;
  bool get connected => !failed && mux != null;

  String get connectionLabel {
    if (failed) return translate('Connection failed');
    if (connected) return translate('Connected');
    if (authPending) return translate('Password Required');
    return translate('Connecting...');
  }

  String get encryptionLabel {
    if (!connected) return translate('Pending');
    if (secure != true) return translate('Insecure');
    return mux == true ? 'E2EE' : translate('Legacy / raw');
  }

  String get tunnelModeLabel =>
      connected ? (mux == true ? 'MUX' : translate('Legacy')) : translate('Pending');

  String get transportLabel {
    if (!connected || direct == null) return translate('Pending');
    final path = direct == true ? translate('Direct') : translate('Relay');
    return streamType.isEmpty ? path : '$path ($streamType)';
  }

  bool get hasSecurityWarning =>
      mux == false || (mux != null && secure != true);

  String get securityWarning {
    if (secure != true && mux != null) {
      return translate('The RustDesk session is not end-to-end encrypted.');
    }
    if (mux == false) {
      return translate(
        'Legacy forwarding is active. TCP payload leaves the RustDesk encrypted session after login.',
      );
    }
    return '';
  }

  void _syncStatus() {
    final model = _ffi?.ffiModel;
    if (model == null) return;
    secure = model.secure;
    direct = model.direct;
    mux = model.portForwardMux;
    authPending = model.portForwardAuthPending;
    streamType = model.cachedPeerData.streamType;
    peerVersion = model.portForwardPeerVersion;

    final modelError = model.portForwardError;
    if (modelError.isNotEmpty || model.portForwardClosed) {
      errorText =
          modelError.isNotEmpty ? modelError : translate('Connection closed');
      running = false;
      authPending = false;
      _closeProbeSocket();
      notifyListeners();
      unawaited(_closeFailedSession());
      return;
    }

    if (mux != null) {
      errorText = '';
      authPending = false;
      _closeProbeSocket();
    }
    notifyListeners();
  }

  void _resetStatus() {
    secure = null;
    direct = null;
    mux = null;
    authPending = false;
    streamType = '';
    peerVersion = '';
    errorText = '';
  }

  Future<void> start({
    required String peerId,
    required List<_TunnelForward> forwards,
  }) async {
    // A failed/cancelled authentication can leave a closed or stale FFI
    // object around after the error dialog. Always retire it before retrying.
    if (_ffi != null) {
      await _closeSession();
    }
    running = false;
    if (forwards.isEmpty) {
      throw StateError('At least one port forward is required');
    }

    final ffi = FFI(null, forceUniqueSession: true);
    _ffi = ffi;
    _resetStatus();
    ffi.ffiModel.addListener(_syncStatus);
    this.peerId = peerId;
    this.forwards = List<_TunnelForward>.unmodifiable(forwards);

    try {
      // Match the normal connection page: let RustDesk resolve any remembered
      // PeerConfig/address-book password itself. If none is available, the
      // existing input-password / re-input-password / 2FA dialogs are used.
      ffi.start(
        peerId,
        isPortForward: true,
      );

      // Reconcile the mobile list with RustDesk's native per-peer
      // PeerConfig.port_forwards. Existing unchanged mappings are already
      // started by the port-forward session; changed/new mappings are updated
      // dynamically through the same API used by the desktop tunnel page.
      final desiredByPort = <int, _TunnelForward>{
        for (final forward in forwards) forward.localPort: forward,
      };
      try {
        final peer = bind.mainGetPeerSync(id: peerId);
        final config = jsonDecode(peer) as Map<String, dynamic>;
        final existingJson =
            (config['port_forwards'] as List<dynamic>? ?? const <dynamic>[]);
        final existing = <_TunnelForward>[];

        for (final item in existingJson) {
          if (item is List &&
              item.length >= 3 &&
              item[0] is int &&
              item[1] is String &&
              item[2] is int) {
            existing.add(_TunnelForward.fromJson(item));
          }
        }

        for (final saved in existing) {
          final desired = desiredByPort[saved.localPort];
          if (desired == null || !saved.sameEndpoint(desired)) {
            await bind.sessionRemovePortForward(
              sessionId: ffi.sessionId,
              localPort: saved.localPort,
            );
          }
        }
      } catch (e) {
        debugPrint('Failed to reconcile saved tunnel mappings: $e');
      }

      for (final forward in forwards) {
        await bind.sessionAddPortForward(
          sessionId: ffi.sessionId,
          localPort: forward.localPort,
          remoteHost: forward.remoteHost,
          remotePort: forward.remotePort,
        );
      }

      final ok = await ffi.invokeMethod(
        'start_tunnel_service',
        {
          'description':
              '${forwards.length} TCP port(s) via RustDesk peer $peerId',
        },
      );
      if (!ok) {
        throw StateError('Unable to start Android tunnel service');
      }
      running = true;
      notifyListeners();

      // Port-forward connections are normally lazy and authenticate only when
      // an app first connects to the local listener. Open one local probe now
      // so Start Tunnel immediately resolves the peer and, when required,
      // shows RustDesk's native password / 2FA prompt before the user switches
      // to the browser.
      await _startPreflight(forwards.first.localPort);
    } catch (e) {
      errorText = e.toString();
      running = false;
      authPending = false;
      await _closeSession();
      notifyListeners();
      rethrow;
    }
  }

  Future<void> _closeFailedSession() async {
    if (_closingAfterFailure) return;
    _closingAfterFailure = true;
    try {
      // Let the msgbox handler finish presenting the error before closing the
      // failed port-forward session. FFI.close() does not dismiss its dialog
      // manager, but it does release the local listeners and native session.
      await Future<void>.delayed(Duration.zero);
      await _closeSession();
    } finally {
      _closingAfterFailure = false;
      notifyListeners();
    }
  }

  Future<void> _startPreflight(int localPort) async {
    Object? lastError;
    for (var attempt = 0; attempt < 20; attempt++) {
      if (_ffi == null) {
        return;
      }
      try {
        final socket = await Socket.connect(
          InternetAddress.loopbackIPv4,
          localPort,
          timeout: const Duration(milliseconds: 500),
        );
        _closeProbeSocket();
        _probeSocket = socket;
        socket.listen(
          (_) {},
          onError: (_) {
            if (identical(_probeSocket, socket)) {
              _probeSocket = null;
            }
          },
          onDone: () {
            if (identical(_probeSocket, socket)) {
              _probeSocket = null;
            }
          },
          cancelOnError: true,
        );
        return;
      } catch (e) {
        lastError = e;
        await Future<void>.delayed(const Duration(milliseconds: 100));
      }
    }
    throw StateError(
      'Failed to probe local tunnel port $localPort: $lastError',
    );
  }

  void _closeProbeSocket() {
    final socket = _probeSocket;
    _probeSocket = null;
    socket?.destroy();
  }

  Future<void> stop() async {
    // Closing the session stops all listeners. Do not remove port forwards
    // here: they are RustDesk's persisted per-peer tunnel configuration and
    // should be available the next time this peer is selected.
    await _closeSession();
    running = false;
    peerId = '';
    forwards = <_TunnelForward>[];
    _resetStatus();
    notifyListeners();
  }

  Future<void> _closeSession() async {
    _closeProbeSocket();
    final ffi = _ffi;
    _ffi = null;
    if (ffi != null) {
      ffi.ffiModel.removeListener(_syncStatus);
      try {
        await ffi.close();
      } catch (e) {
        debugPrint('Failed to close tunnel session: $e');
      }
      try {
        await ffi.invokeMethod('stop_tunnel_service');
      } catch (e) {
        debugPrint('Failed to stop tunnel service: $e');
      }
    }
  }
}

class TunnelPage extends StatefulWidget implements PageShape {
  TunnelPage({super.key});

  @override
  final icon = const Icon(Icons.swap_horiz);

  @override
  final title = translate('Tunnel');

  @override
  final List<Widget> appBarActions = const [];

  @override
  State<TunnelPage> createState() => _TunnelPageState();
}

class _TunnelPageState extends State<TunnelPage> {
  final _peerId = TextEditingController();
  final _peerFocusNode = FocusNode();
  final AllPeersLoader _allPeersLoader = AllPeersLoader();

  List<_TunnelForward> _forwards = <_TunnelForward>[];
  bool _working = false;
  bool _loadingSavedPeer = false;
  String _loadedPeerId = '';

  bool get _running => _androidTunnelController.running;

  @override
  void initState() {
    super.initState();
    _allPeersLoader.init(setState);
    if (_running) {
      _peerId.text = _androidTunnelController.peerId;
      _loadedPeerId = _androidTunnelController.peerId;
      _forwards = List<_TunnelForward>.from(_androidTunnelController.forwards);
    } else {
      WidgetsBinding.instance.addPostFrameCallback((_) async {
        await _allPeersLoader.getAllPeers();
        await _loadLastSavedPeer();
      });
    }
    _androidTunnelController.addListener(_onTunnelStatusChanged);
  }

  Future<void> _loadLastSavedPeer() async {
    if (_loadingSavedPeer || _running) return;
    _loadingSavedPeer = true;
    try {
      final id = (await bind.mainGetLastRemoteId()).trim();
      if (id.isEmpty || !mounted) return;
      await _selectPeer(id);
    } catch (e) {
      debugPrint('Failed to load last saved peer for tunnel: $e');
    } finally {
      _loadingSavedPeer = false;
    }
  }

  Future<void> _selectPeer(String peerId) async {
    final id = peerId.replaceAll(' ', '').trim();
    if (id.isEmpty || _running) return;

    _peerId.text = id;
    _loadedPeerId = id;
    _loadSavedTunnelsForPeer(id);
    if (mounted) {
      setState(() {});
    }
  }

  Map<int, String> _loadTunnelNotesForPeer(String peerId) {
    try {
      final raw = bind.mainGetPeerFlutterOptionSync(
        id: peerId,
        k: _tunnelNotesOptionKey,
      );
      if (raw.isEmpty) {
        return <int, String>{};
      }
      final decoded = jsonDecode(raw);
      if (decoded is! Map) {
        return <int, String>{};
      }
      final notes = <int, String>{};
      decoded.forEach((key, value) {
        final port = int.tryParse(key.toString());
        if (port != null && value is String && value.trim().isNotEmpty) {
          notes[port] = value;
        }
      });
      return notes;
    } catch (e) {
      debugPrint('Failed to load tunnel notes for $peerId: $e');
      return <int, String>{};
    }
  }

  void _saveTunnelNotesForPeer(String peerId) {
    if (peerId.isEmpty) {
      return;
    }
    final notes = <String, String>{
      for (final forward in _forwards)
        if (forward.note.trim().isNotEmpty)
          forward.localPort.toString(): forward.note.trim(),
    };
    try {
      bind.mainSetPeerFlutterOptionSync(
        id: peerId,
        k: _tunnelNotesOptionKey,
        v: jsonEncode(notes),
      );
    } catch (e) {
      debugPrint('Failed to save tunnel notes for $peerId: $e');
    }
  }

  void _loadSavedTunnelsForPeer(String peerId) {
    final result = <_TunnelForward>[];
    final notes = _loadTunnelNotesForPeer(peerId);
    try {
      final peer = bind.mainGetPeerSync(id: peerId);
      final config = jsonDecode(peer) as Map<String, dynamic>;
      final existing =
          (config['port_forwards'] as List<dynamic>? ?? const <dynamic>[]);
      for (final item in existing) {
        if (item is List &&
            item.length >= 3 &&
            item[0] is int &&
            item[1] is String &&
            item[2] is int) {
          result.add(
            _TunnelForward.fromJson(
              item,
              note: notes[item[0] as int] ?? '',
            ),
          );
        }
      }
    } catch (e) {
      debugPrint('Failed to load saved tunnel mappings for $peerId: $e');
    }
    result.sort((a, b) => a.localPort.compareTo(b.localPort));
    _forwards = result;
  }

  void _onTunnelStatusChanged() {
    if (mounted) {
      setState(() {});
    }
  }

  @override
  void dispose() {
    _androidTunnelController.removeListener(_onTunnelStatusChanged);
    _allPeersLoader.clear();
    _peerId.dispose();
    _peerFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final disabled = _working || _running;
    return ListView(
      padding: const EdgeInsets.fromLTRB(12, 10, 12, 24),
      children: [
        _buildPeerSelector(disabled),
        const SizedBox(height: 18),
        _buildForwardHeader(disabled),
        const SizedBox(height: 8),
        if (_forwards.isEmpty)
          _buildEmptyForwardCard()
        else
          ..._forwards.asMap().entries.map(
                (entry) => _buildForwardCard(
                  entry.key,
                  entry.value,
                  disabled,
                ),
              ),
        if (_running || _androidTunnelController.failed) ...[
          const SizedBox(height: 12),
          _buildStatusCard(),
        ],
        const SizedBox(height: 16),
        SizedBox(
          height: 48,
          child: ElevatedButton.icon(
            onPressed: _working
                ? null
                : (_running
                    ? _stop
                    : (_forwards.isEmpty ? null : _start)),
            icon: _working
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : Icon(_running ? Icons.stop : Icons.play_arrow),
            label: Text(
              translate(_running ? 'Stop Tunnel' : 'Start Tunnel'),
            ),
          ),
        ),
      ],
    );
  }

  Widget _buildPeerSelector(bool disabled) {
    return Ink(
      decoration: BoxDecoration(
        color: Theme.of(context).cardColor,
        borderRadius: const BorderRadius.all(Radius.circular(13)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(left: 16),
              child: TextField(
                controller: _peerId,
                focusNode: _peerFocusNode,
                enabled: !disabled,
                autocorrect: false,
                enableSuggestions: false,
                keyboardType: TextInputType.visiblePassword,
                decoration: InputDecoration(
                  labelText: translate('Remote ID'),
                  border: InputBorder.none,
                ),
                onChanged: (value) {
                  final id = value.replaceAll(' ', '').trim();
                  if (id != _loadedPeerId) {
                    setState(() {
                      _loadedPeerId = id;
                      _forwards = <_TunnelForward>[];
                    });
                  }
                },
                onSubmitted: _selectPeer,
              ),
            ),
          ),
          IconButton(
            tooltip: translate('Remote ID'),
            onPressed: disabled ? null : _showPeerPicker,
            icon: const Icon(Icons.arrow_drop_down),
          ),
          const SizedBox(width: 4),
        ],
      ),
    );
  }

  Widget _buildForwardHeader(bool disabled) {
    return Row(
      children: [
        Expanded(
          child: Text(
            translate('TCP tunneling'),
            style: Theme.of(context).textTheme.titleMedium,
          ),
        ),
        TextButton.icon(
          onPressed: disabled ? null : () => _editForward(),
          icon: const Icon(Icons.add),
          label: Text(translate('Add')),
        ),
      ],
    );
  }

  Widget _buildEmptyForwardCard() {
    return Card(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 20),
        child: Column(
          children: [
            Icon(
              Icons.swap_horiz,
              size: 32,
              color: Theme.of(context).disabledColor,
            ),
            const SizedBox(height: 8),
            Text(
              translate('No port forwards configured'),
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildForwardCard(
      int index, _TunnelForward forward, bool disabled) {
    final localUrl = _androidTunnelController.localUrl(forward);
    return Card(
      margin: const EdgeInsets.only(bottom: 8),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 10, 8, 10),
        child: Row(
          children: [
            const Icon(Icons.lan_outlined),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '127.0.0.1:${forward.localPort}',
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                  const SizedBox(height: 2),
                  Row(
                    children: [
                      const Icon(Icons.arrow_forward, size: 14),
                      const SizedBox(width: 5),
                      Expanded(
                        child: Text(
                          '${forward.remoteHost}:${forward.remotePort}',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 2),
                  Row(
                    children: [
                      const Icon(Icons.notes_outlined, size: 14),
                      const SizedBox(width: 5),
                      Expanded(
                        child: Text(
                          '${translate('Note')}: '
                          '${forward.note.isEmpty ? '-' : forward.note}',
                          overflow: TextOverflow.ellipsis,
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            if (_running) ...[
              IconButton(
                tooltip: translate('Open in Browser'),
                onPressed: () => _openBrowser(localUrl),
                icon: const Icon(Icons.open_in_browser),
              ),
              IconButton(
                tooltip: translate('Copy Local URL'),
                onPressed: () => _copyUrl(localUrl),
                icon: const Icon(Icons.copy),
              ),
            ] else ...[
              IconButton(
                tooltip: translate('Edit'),
                onPressed: disabled ? null : () => _editForward(index: index),
                icon: const Icon(Icons.edit_outlined),
              ),
              IconButton(
                tooltip: translate('Delete'),
                onPressed: disabled ? null : () => _removeForward(index),
                icon: const Icon(Icons.delete_outline),
              ),
            ],
          ],
        ),
      ),
    );
  }

  Widget _buildStatusCard() {
    final controller = _androidTunnelController;
    final failed = controller.failed;
    final connected = controller.connected;

    final IconData icon;
    if (failed) {
      icon = Icons.error;
    } else if (connected) {
      icon = Icons.check_circle;
    } else if (controller.authPending) {
      icon = Icons.lock_outline;
    } else {
      icon = Icons.hourglass_top;
    }

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Icon(
                  icon,
                  color: failed
                      ? Theme.of(context).colorScheme.error
                      : (connected ? Colors.green : null),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    controller.connectionLabel,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Text(
              '${translate('Remote ID')}: ${controller.peerId} · '
              '${_forwards.length} ${translate('Port mappings')}',
            ),
            const Divider(height: 24),
            _statusRow(
              translate('Connection'),
              controller.connectionLabel,
            ),
            if (failed) ...[
              const SizedBox(height: 4),
              Text(
                translate(controller.errorText),
                style: TextStyle(
                  color: Theme.of(context).colorScheme.error,
                ),
              ),
            ] else if (connected) ...[
              _statusRow(
                translate('Encryption'),
                controller.encryptionLabel,
              ),
              _statusRow(
                translate('Tunnel mode'),
                controller.tunnelModeLabel,
              ),
              _statusRow(
                translate('Transport'),
                controller.transportLabel,
              ),
              if (controller.peerVersion.isNotEmpty)
                _statusRow(
                  translate('Peer version'),
                  controller.peerVersion,
                ),
              if (controller.hasSecurityWarning) ...[
                const SizedBox(height: 10),
                Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(Icons.warning_amber_rounded),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(controller.securityWarning),
                    ),
                  ],
                ),
              ],
            ],
          ],
        ),
      ),
    );
  }

  Widget _statusRow(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 4),
      child: Row(
        children: [
          SizedBox(
            width: 105,
            child: Text(
              label,
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
          ),
          Expanded(child: SelectableText(value)),
        ],
      ),
    );
  }

  Future<void> _showPeerPicker() async {
    FocusScope.of(context).unfocus();
    if (_allPeersLoader.needLoad) {
      await _allPeersLoader.getAllPeers();
    }
    if (!mounted) return;

    final search = TextEditingController();
    var query = '';

    await showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) {
        return StatefulBuilder(
          builder: (context, setSheetState) {
            final normalized = query.trim().toLowerCase();
            final peers = _allPeersLoader.peers.where((peer) {
              if (normalized.isEmpty) return true;
              return peer.id.toLowerCase().contains(normalized) ||
                  peer.alias.toLowerCase().contains(normalized) ||
                  peer.hostname.toLowerCase().contains(normalized) ||
                  peer.username.toLowerCase().contains(normalized);
            }).toList();

            return FractionallySizedBox(
              heightFactor: 0.78,
              child: SafeArea(
                top: false,
                child: Column(
                  children: [
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 4, 16, 10),
                      child: TextField(
                        controller: search,
                        autofocus: false,
                        decoration: InputDecoration(
                          hintText: translate('Remote ID'),
                          prefixIcon: const Icon(Icons.search),
                        ),
                        onChanged: (value) {
                          setSheetState(() => query = value);
                        },
                        onSubmitted: (value) {
                          final id = value.replaceAll(' ', '').trim();
                          if (id.isEmpty) return;
                          Navigator.of(sheetContext).pop();
                          _selectPeer(id);
                        },
                      ),
                    ),
                    Expanded(
                      child: peers.isEmpty
                          ? Center(
                              child: Text(
                                translate(
                                  'No saved device matches. Enter an ID above.',
                                ),
                              ),
                            )
                          : ListView.builder(
                              padding:
                                  const EdgeInsets.symmetric(horizontal: 12),
                              itemCount: peers.length,
                              itemBuilder: (context, index) {
                                final peer = peers[index];
                                return AutocompletePeerTile(
                                  peer: peer,
                                  onSelect: () {
                                    Navigator.of(sheetContext).pop();
                                    _selectPeer(peer.id);
                                  },
                                );
                              },
                            ),
                    ),
                  ],
                ),
              ),
            );
          },
        );
      },
    );

    search.dispose();
  }

  int _suggestLocalPort() {
    final used = _forwards.map((e) => e.localPort).toSet();
    var candidate = 18080;
    while (used.contains(candidate) && candidate < 65535) {
      candidate++;
    }
    return candidate;
  }

  Future<void> _editForward({int? index}) async {
    final editing = index == null ? null : _forwards[index];
    final formKey = GlobalKey<FormState>();
    final localPort = TextEditingController(
      text: editing?.localPort.toString() ?? _suggestLocalPort().toString(),
    );
    final remoteHost = TextEditingController(
      text: editing?.remoteHost ?? '192.168.1.1',
    );
    final remotePort = TextEditingController(
      text: editing?.remotePort.toString() ?? '80',
    );
    final note = TextEditingController(text: editing?.note ?? '');

    final result = await showModalBottomSheet<_TunnelForward>(
      context: context,
      isScrollControlled: true,
      showDragHandle: true,
      builder: (sheetContext) {
        return SafeArea(
          top: false,
          child: SingleChildScrollView(
            padding: EdgeInsets.fromLTRB(
              20,
              4,
              20,
              20 + MediaQuery.of(sheetContext).viewInsets.bottom,
            ),
            child: Form(
              key: formKey,
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    editing == null ? translate('Add') : translate('Edit'),
                    style: Theme.of(sheetContext).textTheme.titleLarge,
                  ),
                  const SizedBox(height: 16),
                  TextFormField(
                    controller: note,
                    textInputAction: TextInputAction.next,
                    decoration: InputDecoration(
                      labelText: translate('Note'),
                      hintText: translate('input note here'),
                    ),
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: localPort,
                    autofocus: editing == null,
                    keyboardType: TextInputType.number,
                    textInputAction: TextInputAction.next,
                    autovalidateMode: AutovalidateMode.onUserInteraction,
                    inputFormatters: [
                      FilteringTextInputFormatter.digitsOnly,
                    ],
                    decoration: InputDecoration(
                      labelText: translate('Local Port'),
                      helperText: translate('Range: 1024-65535'),
                    ),
                    validator: (value) {
                      final lp = int.tryParse(value?.trim() ?? '');
                      if (lp == null || lp < 1024 || lp > 65535) {
                        return translate(
                          'Local port must be between 1024 and 65535.',
                        );
                      }
                      final duplicate = _forwards.asMap().entries.any(
                            (entry) =>
                                entry.key != index &&
                                entry.value.localPort == lp,
                          );
                      if (duplicate) {
                        return '${translate('Local port is already configured')}: $lp';
                      }
                      return null;
                    },
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: remoteHost,
                    textInputAction: TextInputAction.next,
                    autovalidateMode: AutovalidateMode.onUserInteraction,
                    decoration: InputDecoration(
                      labelText: translate('Remote Host'),
                      hintText: '192.168.1.100',
                    ),
                    validator: (value) {
                      if ((value ?? '').trim().isEmpty) {
                        return translate('Remote host is required.');
                      }
                      return null;
                    },
                  ),
                  const SizedBox(height: 12),
                  TextFormField(
                    controller: remotePort,
                    keyboardType: TextInputType.number,
                    textInputAction: TextInputAction.done,
                    autovalidateMode: AutovalidateMode.onUserInteraction,
                    inputFormatters: [
                      FilteringTextInputFormatter.digitsOnly,
                    ],
                    decoration: InputDecoration(
                      labelText: translate('Remote Port'),
                      helperText: translate('Range: 1-65535'),
                    ),
                    validator: (value) {
                      final rp = int.tryParse(value?.trim() ?? '');
                      if (rp == null || rp < 1 || rp > 65535) {
                        return translate(
                          'Remote port must be between 1 and 65535.',
                        );
                      }
                      return null;
                    },
                    onFieldSubmitted: (_) {
                      if (formKey.currentState?.validate() == true) {
                        Navigator.of(sheetContext).pop(
                          _TunnelForward(
                            localPort: int.parse(localPort.text.trim()),
                            remoteHost: remoteHost.text.trim(),
                            remotePort: int.parse(remotePort.text.trim()),
                            note: note.text.trim(),
                          ),
                        );
                      }
                    },
                  ),
                  const SizedBox(height: 20),
                  SizedBox(
                    width: double.infinity,
                    height: 46,
                    child: ElevatedButton(
                      onPressed: () {
                        if (formKey.currentState?.validate() != true) {
                          return;
                        }
                        Navigator.of(sheetContext).pop(
                          _TunnelForward(
                            localPort: int.parse(localPort.text.trim()),
                            remoteHost: remoteHost.text.trim(),
                            remotePort: int.parse(remotePort.text.trim()),
                            note: note.text.trim(),
                          ),
                        );
                      },
                      child: Text(
                        editing == null ? translate('Add') : translate('Save'),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );

    localPort.dispose();
    remoteHost.dispose();
    remotePort.dispose();
    note.dispose();

    if (result == null || !mounted) return;
    setState(() {
      if (index == null) {
        _forwards.add(result);
      } else {
        _forwards[index] = result;
      }
      _forwards.sort((a, b) => a.localPort.compareTo(b.localPort));
    });
    _saveTunnelNotesForPeer(_peerId.text.replaceAll(' ', '').trim());
  }

  void _removeForward(int index) {
    setState(() {
      _forwards.removeAt(index);
    });
    _saveTunnelNotesForPeer(_peerId.text.replaceAll(' ', '').trim());
  }

  Future<void> _start() async {
    final peer = _peerId.text.replaceAll(' ', '').trim();
    if (peer.isEmpty) {
      _error(translate('RustDesk ID is required.'));
      return;
    }
    // A manually typed saved ID may not have been submitted yet. If there is
    // no edited mapping list, give it one last chance to load that peer's
    // native saved port_forwards before rejecting the start.
    if (_forwards.isEmpty) {
      _loadSavedTunnelsForPeer(peer);
      if (mounted) {
        setState(() {});
      }
    }
    if (_forwards.isEmpty) {
      _error(translate('Add at least one port forward.'));
      return;
    }

    _saveTunnelNotesForPeer(peer);
    setState(() => _working = true);
    try {
      await _androidTunnelController.start(
        peerId: peer,
        forwards: List<_TunnelForward>.from(_forwards),
      );
      if (mounted) {
        setState(() {});
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(
            content: Text(
              translate('Tunnel started'),
            ),
          ),
        );
      }
    } catch (e) {
      _error('${translate('Failed to start tunnel')}: $e');
    } finally {
      if (mounted) {
        setState(() => _working = false);
      }
    }
  }

  Future<void> _stop() async {
    setState(() => _working = true);
    try {
      await _androidTunnelController.stop();
      if (mounted) {
        setState(() {});
      }
    } finally {
      if (mounted) {
        setState(() => _working = false);
      }
    }
  }

  Future<void> _openBrowser(String url) async {
    final ok = await launchUrl(
      Uri.parse(url),
      mode: LaunchMode.externalApplication,
    );
    if (!ok) {
      _error('${translate('Unable to open')}: $url');
    }
  }

  Future<void> _copyUrl(String url) async {
    await Clipboard.setData(ClipboardData(text: url));
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(translate('Local URL copied'))),
    );
  }

  void _error(String text) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text(text)),
    );
  }
}
