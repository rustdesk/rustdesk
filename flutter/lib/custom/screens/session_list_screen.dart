import 'package:flutter/material.dart';

import '../../common.dart';
import '../../models/peer_model.dart';
import '../../models/platform_model.dart';
import '../theme/tokens.dart';

class SessionListScreen extends StatefulWidget {
  const SessionListScreen({super.key});

  @override
  State<SessionListScreen> createState() => _SessionListScreenState();
}

class _SessionListScreenState extends State<SessionListScreen> {
  final _searchController = TextEditingController();
  String _query = '';

  @override
  void initState() {
    super.initState();
    bind.mainLoadRecentPeers();
    bind.mainLoadFavPeers();
    _searchController.addListener(() {
      setState(() => _query = _searchController.text.trim().toLowerCase());
    });
  }

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  List<Peer> _filter(List<Peer> peers) {
    if (_query.isEmpty) return peers;
    return peers.where((p) {
      return p.id.toLowerCase().contains(_query) ||
          p.alias.toLowerCase().contains(_query) ||
          p.hostname.toLowerCase().contains(_query);
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppTokens.colorBgBase,
      body: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(
                AppTokens.spaceXl,
                AppTokens.spaceLg,
                AppTokens.spaceXl,
                AppTokens.spaceSm,
              ),
              child: TextField(
                controller: _searchController,
                style:
                    AppTokens.fontBody.copyWith(color: AppTokens.colorTextHigh),
                decoration: InputDecoration(
                  hintText: 'Search peers…',
                  hintStyle: AppTokens.fontBody
                      .copyWith(color: AppTokens.colorTextMid),
                  prefixIcon: const Icon(Icons.search,
                      color: AppTokens.colorTextMid, size: 20),
                  filled: true,
                  fillColor: AppTokens.colorBgSurface,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(AppTokens.radiusCard),
                    borderSide: BorderSide.none,
                  ),
                  contentPadding: const EdgeInsets.symmetric(
                    vertical: AppTokens.spaceSm,
                  ),
                ),
              ),
            ),
            Expanded(
              child: ListenableBuilder(
                listenable: Listenable.merge([
                  gFFI.favoritePeersModel,
                  gFFI.recentPeersModel,
                ]),
                builder: (context, _) {
                  final favs =
                      _filter(gFFI.favoritePeersModel.peers.toList());
                  final recents =
                      _filter(gFFI.recentPeersModel.peers.toList());

                  if (favs.isEmpty && recents.isEmpty) {
                    return Center(
                      child: Text(
                        _query.isEmpty
                            ? 'No saved peers yet'
                            : 'No peers match "$_query"',
                        style: AppTokens.fontBody
                            .copyWith(color: AppTokens.colorTextMid),
                      ),
                    );
                  }

                  return ListView(
                    padding: const EdgeInsets.symmetric(
                      horizontal: AppTokens.spaceXl,
                      vertical: AppTokens.spaceSm,
                    ),
                    children: [
                      if (favs.isNotEmpty) ...[
                        _SectionHeader(label: 'Favorites'),
                        ...favs.map((p) => _SessionTile(peer: p)),
                        const SizedBox(height: AppTokens.spaceLg),
                      ],
                      if (recents.isNotEmpty) ...[
                        _SectionHeader(label: 'Recent'),
                        ...recents.map((p) => _SessionTile(peer: p)),
                      ],
                    ],
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader({required this.label});
  final String label;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: AppTokens.spaceSm),
      child: Text(
        label,
        style:
            AppTokens.fontKeySmall.copyWith(color: AppTokens.colorTextMid),
      ),
    );
  }
}

class _SessionTile extends StatelessWidget {
  const _SessionTile({required this.peer});
  final Peer peer;

  @override
  Widget build(BuildContext context) {
    final label = peer.alias.isNotEmpty ? peer.alias : peer.id;
    final sub = [peer.hostname, peer.platform]
        .where((s) => s.isNotEmpty)
        .join(' · ');

    return Padding(
      padding: const EdgeInsets.only(bottom: AppTokens.spaceSm),
      child: Material(
        color: AppTokens.colorBgSurface,
        borderRadius: BorderRadius.circular(AppTokens.radiusCard),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
          onTap: () => connect(context, peer.id),
          child: Padding(
            padding: const EdgeInsets.symmetric(
              horizontal: AppTokens.spaceLg,
              vertical: AppTokens.spaceMd,
            ),
            child: Row(
              children: [
                const Icon(Icons.computer,
                    color: AppTokens.colorTextMid, size: 20),
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
                      if (sub.isNotEmpty)
                        Text(
                          sub,
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
