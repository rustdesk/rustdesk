// Master — the technician's console.
//
// The screen only offers what the core can actually do from the main window:
// start an outgoing session for an ID, list the session windows this window
// opened, bring one to the front, and close it. It never accepts an incoming
// connection: that refusal is enforced in the Rust layer, not here.

import 'dart:async';
import 'dart:convert';

import 'package:desktop_multi_window/desktop_multi_window.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/formatter/id_formatter.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:flutter_hbb/tefway/design.dart';
import 'package:flutter_hbb/tefway/session_registry.dart';
import 'package:flutter_hbb/tefway/strings.dart';
import 'package:flutter_hbb/tefway/widgets.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';

class TefwayMasterHomePage extends StatefulWidget {
  const TefwayMasterHomePage({super.key});

  @override
  State<TefwayMasterHomePage> createState() => _TefwayMasterHomePageState();
}

class _TefwayMasterHomePageState extends State<TefwayMasterHomePage> {
  final _idController = IDTextEditingController();
  final _registry = TefwaySessionRegistry.instance;

  Timer? _timer;
  SvcStatus _svcStatus = SvcStatus.notReady;
  String _validationError = '';
  String _connectError = '';
  bool _connecting = false;
  String _version = '';
  Set<int> _activeWindows = {};

  @override
  void initState() {
    super.initState();
    _loadVersion();
    _timer = periodic_immediate(const Duration(seconds: 1), _refresh);
  }

  @override
  void dispose() {
    _timer?.cancel();
    _idController.dispose();
    super.dispose();
  }

  Future<void> _loadVersion() async {
    try {
      final version = await bind.mainGetVersion();
      if (!mounted) return;
      setState(() => _version = version);
    } catch (e) {
      debugPrint('Tefway: failed to read app version: $e');
    }
  }

  Future<void> _refresh() async {
    var status = _svcStatus;
    try {
      final raw =
          jsonDecode(await bind.mainGetConnectStatus()) as Map<String, dynamic>;
      final statusNum = raw['status_num'] as int?;
      if (statusNum == null) {
        status = SvcStatus.notReady;
      } else if (statusNum == 0) {
        status = SvcStatus.connecting;
      } else if (statusNum == 1) {
        status = SvcStatus.ready;
      } else {
        status = SvcStatus.notReady;
      }
    } catch (e) {
      status = SvcStatus.notReady;
    }

    Set<int> liveWindows = {};
    try {
      liveWindows = (await rustDeskWinManager.getAllSubWindowIds()).toSet();
    } catch (e) {
      debugPrint('Tefway: failed to list session windows: $e');
    }
    _registry.retainWindows(liveWindows);

    if (!mounted) return;
    setState(() {
      _svcStatus = status;
      _activeWindows = rustDeskWinManager.getActiveWindows();
    });
  }

  String? _validate(String id) {
    if (id.isEmpty) return TefwayStrings.idRequired;
    if (!RegExp(r'^\d{6,}$').hasMatch(id)) return TefwayStrings.idInvalid;
    if (id == trimID(gFFI.serverModel.serverId.text)) {
      return TefwayStrings.idSelf;
    }
    return null;
  }

  Future<void> _connect() async {
    final id = _idController.id;
    final error = _validate(id);
    if (error != null) {
      setState(() {
        _validationError = error;
        _connectError = '';
      });
      return;
    }
    setState(() {
      _validationError = '';
      _connectError = '';
      _connecting = true;
    });
    try {
      final relayId = await bind.mainHandleRelayId(id: id);
      final result = await rustDeskWinManager.newRemoteDesktop(
        relayId,
        forceRelay: relayId != id,
      );
      if (result.windowId == kInvalidWindowId) {
        throw StateError('session window was not created');
      }
      _registry.add(peerId: relayId, windowId: result.windowId);
      await WindowController.fromWindowId(result.windowId).show();
      if (!mounted) return;
      setState(() {
        _connecting = false;
        _idController.id = '';
      });
    } catch (e) {
      debugPrint('Tefway: failed to open remote session: $e');
      if (!mounted) return;
      setState(() {
        _connecting = false;
        _connectError = TefwayStrings.connectFailed;
      });
    }
  }

  Future<void> _openSession(int windowId) async {
    try {
      await WindowController.fromWindowId(windowId).show();
    } catch (e) {
      debugPrint('Tefway: failed to focus session window $windowId: $e');
      _registry.removeByWindow(windowId);
    }
  }

  Future<void> _endSession(TefwaySession session) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        backgroundColor: TefwayColors.surface,
        title: const Text(TefwayStrings.endSessionTitle),
        content: Text(
          TefwayStrings.endSessionBody(formatID(session.peerId)),
          style: TefwayTextStyles.bodySecondary,
        ),
        actions: [
          TefwaySecondaryButton(
            label: TefwayStrings.cancel,
            onPressed: () => Navigator.of(context).pop(false),
          ),
          TefwayPrimaryButton(
            label: TefwayStrings.end,
            onPressed: () => Navigator.of(context).pop(true),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await WindowController.fromWindowId(session.windowId).close();
    } catch (e) {
      debugPrint('Tefway: failed to close session window: $e');
    }
    _registry.removeByWindow(session.windowId);
  }

  Widget _serverChip() {
    switch (_svcStatus) {
      case SvcStatus.ready:
        return const TefwayStatusChip(
          status: TefwayStatus.positive,
          label: TefwayStrings.serverOk,
        );
      case SvcStatus.connecting:
        return const TefwayStatusChip(
          status: TefwayStatus.busy,
          label: TefwayStrings.serverConnecting,
        );
      case SvcStatus.notReady:
        return const TefwayStatusChip(
          status: TefwayStatus.critical,
          label: TefwayStrings.serverDown,
        );
    }
  }

  Widget _connectCard() {
    return TefwaySurfaceCard(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          const Text(TefwayStrings.newConnection,
              style: TefwayTextStyles.sectionTitle),
          const SizedBox(height: TefwaySpacing.md),
          TextField(
            controller: _idController,
            autofocus: true,
            style: TefwayTextStyles.body.copyWith(fontSize: 15),
            cursorColor: TefwayColors.actionBlue,
            inputFormatters: [IDTextInputFormatter()],
            textInputAction: TextInputAction.go,
            onChanged: (_) {
              if (_validationError.isNotEmpty || _connectError.isNotEmpty) {
                setState(() {
                  _validationError = '';
                  _connectError = '';
                });
              }
            },
            onSubmitted: (_) {
              if (!_connecting) _connect();
            },
            decoration: const InputDecoration(
              hintText: TefwayStrings.remoteIdHint,
              prefixIcon: Icon(
                Icons.desktop_windows_outlined,
                size: 18,
                color: TefwayColors.textSecondary,
              ),
            ),
          ),
          const SizedBox(height: TefwaySpacing.md),
          Row(
            crossAxisAlignment: CrossAxisAlignment.center,
            children: [
              Expanded(
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (_validationError.isNotEmpty)
                      TefwayStatusIndicator(
                        status: TefwayStatus.critical,
                        label: _validationError,
                        compact: true,
                      )
                    else if (_connectError.isNotEmpty)
                      TefwayStatusIndicator(
                        status: TefwayStatus.critical,
                        label: _connectError,
                        compact: true,
                      )
                    else if (_connecting)
                      const TefwayStatusIndicator(
                        status: TefwayStatus.busy,
                        label: TefwayStrings.connecting,
                        compact: true,
                      )
                    else if (_svcStatus == SvcStatus.notReady)
                      const TefwayStatusIndicator(
                        status: TefwayStatus.critical,
                        label: TefwayStrings.serverUnavailable,
                        compact: true,
                      ),
                  ],
                ),
              ),
              const SizedBox(width: TefwaySpacing.md),
              TefwayPrimaryButton(
                label: TefwayStrings.connect,
                icon: Icons.arrow_forward_rounded,
                onPressed: _connecting ? null : _connect,
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _sessionsSection() {
    return AnimatedBuilder(
      animation: _registry,
      builder: (context, _) {
        final sessions = _registry.sessions;
        return Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TefwaySectionTitle(
              title: TefwayStrings.activeSessions,
              trailing: TefwayStatusChip(
                status: sessions.isEmpty
                    ? TefwayStatus.neutral
                    : TefwayStatus.positive,
                label: sessions.length == 1
                    ? '1 conexão'
                    : '${sessions.length} conexões',
              ),
            ),
            const SizedBox(height: TefwaySpacing.md),
            if (sessions.isEmpty)
              const TefwayEmptyState(
                icon: Icons.desktop_access_disabled_outlined,
                title: TefwayStrings.noSessions,
                message: TefwayStrings.noSessionsHint,
              )
            else
              for (final session in sessions) ...[
                TefwaySessionCard(
                  title: 'Computador ${formatID(session.peerId)}',
                  subtitle:
                      '${TefwayStrings.sessionSince} ${_formatTime(session.startedAt)}',
                  status: _activeWindows.contains(session.windowId)
                      ? TefwayStatus.positive
                      : TefwayStatus.neutral,
                  statusLabel: _activeWindows.contains(session.windowId)
                      ? TefwayStrings.sessionFocused
                      : TefwayStrings.sessionOpen,
                  onOpen: () => _openSession(session.windowId),
                  onEnd: () => _endSession(session),
                ),
                if (session != sessions.last)
                  const SizedBox(height: TefwaySpacing.sm),
              ],
            const SizedBox(height: TefwaySpacing.sm),
            Text(TefwayStrings.sessionsHint, style: TefwayTextStyles.caption),
          ],
        );
      },
    );
  }

  static String _formatTime(DateTime time) {
    String two(int v) => v.toString().padLeft(2, '0');
    return '${two(time.hour)}:${two(time.minute)}';
  }

  @override
  Widget build(BuildContext context) {
    return TefwayAppShell(
      maxWidth: 820,
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TefwayBrandHeader(
            subtitle: TefwayStrings.masterSubtitle,
            trailing: _serverChip(),
          ),
          const SizedBox(height: TefwaySpacing.xl),
          _connectCard(),
          const SizedBox(height: TefwaySpacing.xl),
          _sessionsSection(),
          const SizedBox(height: TefwaySpacing.xl),
          TefwaySupportFooter(version: _version),
        ],
      ),
    );
  }
}
