import 'package:flutter/material.dart';

import '../../common.dart';
import '../../common/formatter/id_formatter.dart';
import '../../mobile/widgets/dialog.dart' show showServerSettings;
import '../../models/model.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import '../session/session_registry.dart';
import '../theme/tokens.dart';
import 'remote_session_screen.dart';

class ConnectScreen extends StatefulWidget {
  const ConnectScreen({super.key});

  @override
  State<ConnectScreen> createState() => _ConnectScreenState();
}

class _ConnectScreenState extends State<ConnectScreen> {
  final _idController = IDTextEditingController();
  final _passwordController = TextEditingController();
  bool _obscurePassword = true;

  @override
  void initState() {
    super.initState();
    bind.mainLoadRecentPeers();
    gFFI.recentPeersModel.addListener(_onPeersChanged);
  }

  @override
  void dispose() {
    gFFI.recentPeersModel.removeListener(_onPeersChanged);
    _idController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  void _onPeersChanged() {
    if (gFFI.recentPeersModel.event == UpdateEvent.load) {
      final ids = gFFI.recentPeersModel.peers.map((p) => p.id).toList();
      if (ids.isNotEmpty) bind.queryOnlines(ids: ids);
    }
  }

  void _onConnect([String? peerId]) async {
    final registry = SessionRegistry.instance;
    if (registry.isFull) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Maximum 5 sessions reached. Close one first.')),
      );
      return;
    }
    final rawId = peerId ?? _idController.id;
    if (rawId.isEmpty) return;
    final pw = _passwordController.text.trim();
    final id = await bind.mainHandleRelayId(id: rawId);
    final forceRelay = id != rawId;
    if (!mounted) return;

    final sessionFfi = FFI(null);
    registry.register(sessionFfi, id);

    Navigator.push(
      context,
      MaterialPageRoute(
        builder: (_) => RemoteSessionScreen(
          id: id,
          ffi: sessionFfi,
          password: pw.isEmpty ? null : pw,
          forceRelay: forceRelay,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppTokens.colorBgBase,
      resizeToAvoidBottomInset: true,
      body: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.symmetric(
            horizontal: AppTokens.spaceXl,
            vertical: AppTokens.spaceXl,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const SizedBox(height: AppTokens.spaceXl),
              loadIcon(72),
              const SizedBox(height: AppTokens.spaceLg),
              Text(
                'Tabby',
                textAlign: TextAlign.center,
                style: AppTokens.fontTitle
                    .copyWith(color: AppTokens.colorTextHigh),
              ),
              const SizedBox(height: 40),
              _PeerIdField(
                controller: _idController,
                onSubmitted: (_) => _onConnect(),
              ),
              const SizedBox(height: AppTokens.spaceSm),
              _PasswordField(
                controller: _passwordController,
                obscure: _obscurePassword,
                onToggleObscure: () =>
                    setState(() => _obscurePassword = !_obscurePassword),
                onSubmitted: (_) => _onConnect(),
              ),
              const SizedBox(height: AppTokens.spaceLg),
              FilledButton(
                onPressed: _onConnect,
                style: FilledButton.styleFrom(
                  backgroundColor: AppTokens.colorPrimary,
                  padding: const EdgeInsets.symmetric(
                      vertical: AppTokens.spaceMd),
                  shape: RoundedRectangleBorder(
                    borderRadius:
                        BorderRadius.circular(AppTokens.radiusCard),
                  ),
                ),
                child: Text(
                  'Connect',
                  style: AppTokens.fontKey
                      .copyWith(color: AppTokens.colorTextHigh),
                ),
              ),
              const SizedBox(height: AppTokens.spaceSm),
              _EditServerIdButton(),
              const SizedBox(height: AppTokens.spaceXl),
              _RecentPeers(onConnect: _onConnect),
            ],
          ),
        ),
      ),
    );
  }
}

class _PeerIdField extends StatelessWidget {
  const _PeerIdField(
      {required this.controller, required this.onSubmitted});

  final IDTextEditingController controller;
  final ValueChanged<String> onSubmitted;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      inputFormatters: [IDTextInputFormatter()],
      keyboardType: TextInputType.number,
      textInputAction: TextInputAction.next,
      textAlign: TextAlign.center,
      style: const TextStyle(
        fontSize: 28,
        fontWeight: FontWeight.w600,
        color: AppTokens.colorTextHigh,
        letterSpacing: 4,
      ),
      decoration: InputDecoration(
        hintText: 'Peer ID',
        hintStyle: TextStyle(
          fontSize: 28,
          color: AppTokens.colorTextMid.withValues(alpha: 0.5),
          letterSpacing: 2,
        ),
        filled: true,
        fillColor: AppTokens.colorBgSurface,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
          borderSide: BorderSide.none,
        ),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: AppTokens.spaceLg,
          vertical: AppTokens.spaceMd,
        ),
      ),
      onSubmitted: onSubmitted,
    );
  }
}

class _PasswordField extends StatelessWidget {
  const _PasswordField({
    required this.controller,
    required this.obscure,
    required this.onToggleObscure,
    required this.onSubmitted,
  });

  final TextEditingController controller;
  final bool obscure;
  final VoidCallback onToggleObscure;
  final ValueChanged<String> onSubmitted;

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: controller,
      obscureText: obscure,
      textInputAction: TextInputAction.done,
      style: AppTokens.fontBody.copyWith(color: AppTokens.colorTextHigh),
      decoration: InputDecoration(
        hintText: 'Password (optional)',
        hintStyle: AppTokens.fontBody.copyWith(
          color: AppTokens.colorTextMid.withValues(alpha: 0.6),
        ),
        filled: true,
        fillColor: AppTokens.colorBgSurface,
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
          borderSide: BorderSide.none,
        ),
        contentPadding: const EdgeInsets.symmetric(
          horizontal: AppTokens.spaceLg,
          vertical: AppTokens.spaceMd,
        ),
        suffixIcon: IconButton(
          icon: Icon(
            obscure
                ? Icons.visibility_outlined
                : Icons.visibility_off_outlined,
            color: AppTokens.colorTextMid,
          ),
          onPressed: onToggleObscure,
        ),
      ),
      onSubmitted: onSubmitted,
    );
  }
}

class _RecentPeers extends StatelessWidget {
  const _RecentPeers({required this.onConnect});

  final ValueChanged<String> onConnect;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: gFFI.recentPeersModel,
      builder: (context, _) {
        final peers = gFFI.recentPeersModel.peers.take(5).toList();
        if (peers.isEmpty) return const SizedBox.shrink();
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Recent',
              style: AppTokens.fontKeySmall
                  .copyWith(color: AppTokens.colorTextMid),
            ),
            const SizedBox(height: AppTokens.spaceSm),
            ...peers.map((peer) => _PeerTile(
                  peer: peer,
                  onTap: () => onConnect(peer.id),
                )),
          ],
        );
      },
    );
  }
}


class _EditServerIdButton extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return Center(
      child: TextButton.icon(
        onPressed: () => showServerSettings(gFFI.dialogManager, (fn) => fn()),
        icon: const Icon(Icons.dns_outlined,
            size: 16, color: AppTokens.colorTextMid),
        label: Text(
          'Edit server ID',
          style: AppTokens.fontKeySmall.copyWith(color: AppTokens.colorTextMid),
        ),
      ),
    );
  }
}

class _PeerTile extends StatelessWidget {
  const _PeerTile({required this.peer, required this.onTap});

  final Peer peer;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final label = peer.alias.isNotEmpty ? peer.alias : peer.id;
    final subtitle = peer.hostname.isNotEmpty ? peer.hostname : null;

    return Padding(
      padding: const EdgeInsets.only(bottom: AppTokens.spaceSm),
      child: Material(
        color: AppTokens.colorBgSurface,
        borderRadius: BorderRadius.circular(AppTokens.radiusCard),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: AppTokens.spaceLg,
              vertical: AppTokens.spaceMd,
            ),
            child: Row(
              children: [
                Stack(
                  clipBehavior: Clip.none,
                  children: [
                    const Icon(Icons.computer,
                        color: AppTokens.colorTextMid, size: 20),
                    Positioned(
                      right: -2,
                      bottom: -2,
                      child: Container(
                        width: 8,
                        height: 8,
                        decoration: BoxDecoration(
                          shape: BoxShape.circle,
                          color: peer.online
                              ? AppTokens.colorOnline
                              : AppTokens.colorTextMid.withValues(alpha: 0.3),
                          border: Border.all(
                              color: AppTokens.colorBgSurface, width: 1.5),
                        ),
                      ),
                    ),
                  ],
                ),
                const SizedBox(width: AppTokens.spaceMd),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        label,
                        style: AppTokens.fontBody
                            .copyWith(color: AppTokens.colorTextHigh),
                      ),
                      if (subtitle != null)
                        Text(
                          subtitle,
                          style: AppTokens.fontKeySmall
                              .copyWith(color: AppTokens.colorTextMid),
                        ),
                    ],
                  ),
                ),
                const Icon(Icons.chevron_right,
                    color: AppTokens.colorTextMid, size: 20),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
