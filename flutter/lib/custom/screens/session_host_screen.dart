import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/models/platform_model.dart';

import 'remote_session_screen.dart';
import 'session_registry.dart';

class SessionHostScreen extends StatefulWidget {
  final String initialPeerId;
  final String? password;
  final bool? isSharedPassword;
  final bool? forceRelay;

  const SessionHostScreen({
    super.key,
    required this.initialPeerId,
    this.password,
    this.isSharedPassword,
    this.forceRelay,
  });

  @override
  State<SessionHostScreen> createState() => _SessionHostScreenState();
}

class _SessionHostScreenState extends State<SessionHostScreen> {
  String _activePeerId = '';

  @override
  void initState() {
    super.initState();
    _activePeerId = widget.initialPeerId;
    SessionRegistry.instance.addSession(
      peerId: widget.initialPeerId,
      password: widget.password,
      isSharedPassword: widget.isSharedPassword,
      forceRelay: widget.forceRelay,
    );
    SessionRegistry.instance.addListener(_onRegistryChanged);
  }

  @override
  void dispose() {
    SessionRegistry.instance.removeListener(_onRegistryChanged);
    super.dispose();
  }

  void _onRegistryChanged() {
    final ids = SessionRegistry.instance.peerIds;
    if (ids.isEmpty) {
      // All sessions closed — pop back to home.
      closeConnection();
      return;
    }
    if (!ids.contains(_activePeerId)) {
      // Active session was closed; switch to most recent remaining.
      setState(() => _activePeerId = ids.last);
    } else {
      setState(() {});
    }
  }

  Future<void> _switchSession(String peerId) async {
    if (peerId == _activePeerId) return;

    // Suppress the outgoing background session (view-only = no video decode).
    final currentFfi = SessionRegistry.instance.get(_activePeerId)?.ffi;
    if (currentFfi != null) {
      await bind.sessionToggleOption(
        sessionId: currentFfi.sessionId,
        value: kOptionToggleViewOnly,
      );
    }

    setState(() => _activePeerId = peerId);

    // Restore the newly active session.
    final nextFfi = SessionRegistry.instance.get(peerId)?.ffi;
    if (nextFfi != null) {
      // Only un-toggle if it was actually set view-only by us.
      final isViewOnly = bind.sessionGetToggleOptionSync(
        sessionId: nextFfi.sessionId,
        arg: kOptionToggleViewOnly,
      );
      if (isViewOnly) {
        await bind.sessionToggleOption(
          sessionId: nextFfi.sessionId,
          value: kOptionToggleViewOnly,
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final ids = SessionRegistry.instance.peerIds;
    if (ids.isEmpty) return const SizedBox.shrink();

    final activeIndex = ids.indexOf(_activePeerId).clamp(0, ids.length - 1);

    return IndexedStack(
      index: activeIndex,
      children: ids.map((id) {
        final info = SessionRegistry.instance.get(id)!;
        return RemoteSessionScreen(
          key: ValueKey(id),
          id: id,
          password: info.password,
          isSharedPassword: info.isSharedPassword,
          forceRelay: info.forceRelay,
          onSessionClosed: () {
            // Handled via registry listener in _onRegistryChanged.
          },
          onActivated: () => WakelockManager.startIdleTimer(() {
            final ffi = SessionRegistry.instance.get(id)?.ffi;
            if (ffi != null) WakelockManager.disable(UniqueKey());
          }),
          onDeactivated: WakelockManager.cancelIdleTimer,
          onSwitchSession: _switchSession,
        );
      }).toList(),
    );
  }
}
