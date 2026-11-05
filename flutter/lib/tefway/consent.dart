// Tefway consent surface for an incoming session.
//
// Presentation only: the accept/refuse/permission calls stay in
// `desktop/pages/server_page.dart`, next to the click-tampering guard the core
// window already relies on. This widget must never grant anything on its own.

import 'package:flutter/material.dart';

import 'design.dart';
import 'strings.dart';
import 'widgets.dart';

/// One capability the remote side is asking for, with its current state.
class TefwayCapability {
  const TefwayCapability({
    required this.key,
    required this.label,
    required this.icon,
    required this.enabled,
  });

  /// Permission name understood by the core (`cmSwitchPermission`).
  final String key;
  final String label;
  final IconData icon;
  final bool enabled;
}

class TefwayConsentCard extends StatefulWidget {
  const TefwayConsentCard({
    super.key,
    required this.peerName,
    required this.peerId,
    required this.capabilities,
    required this.onToggle,
    required this.onAllow,
    required this.onRefuse,
    this.onAllowAndElevate,
  });

  final String peerName;
  final String peerId;
  final List<TefwayCapability> capabilities;
  final void Function(String key, bool enabled) onToggle;
  final VoidCallback onAllow;
  final VoidCallback onRefuse;
  final VoidCallback? onAllowAndElevate;

  @override
  State<TefwayConsentCard> createState() => _TefwayConsentCardState();
}

class _TefwayConsentCardState extends State<TefwayConsentCard> {
  late final Map<String, bool> _enabled = {
    for (final capability in widget.capabilities) capability.key: capability.enabled,
  };

  @override
  Widget build(BuildContext context) {
    // The core window hosting this card is 300x490, so the decision is pinned
    // below the scrollable detail instead of scrolling out of reach.
    return ColoredBox(
      color: TefwayColors.deepNavy,
      child: Column(
        children: [
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.fromLTRB(
                  TefwaySpacing.xl, TefwaySpacing.xl, TefwaySpacing.xl, 0),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const TefwayBrandHeader(
                    subtitle: TefwayStrings.consentSubtitle,
                    trailing: TefwayStatusChip(
                      status: TefwayStatus.attention,
                      label: TefwayStrings.consentWaiting,
                    ),
                  ),
                  const SizedBox(height: TefwaySpacing.xl),
                  TefwaySurfaceCard(
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(TefwayStrings.consentPeerLabel,
                            style: TefwayTextStyles.label),
                        const SizedBox(height: TefwaySpacing.xs),
                        Text(widget.peerName,
                            style: TefwayTextStyles.sectionTitle),
                        const SizedBox(height: 2),
                        Text(widget.peerId, style: TefwayTextStyles.caption),
                        const SizedBox(height: TefwaySpacing.lg),
                        Text(TefwayStrings.consentCapabilities,
                            style: TefwayTextStyles.label),
                        const SizedBox(height: TefwaySpacing.sm),
                        for (final capability in widget.capabilities)
                          _capabilityRow(capability),
                        const SizedBox(height: TefwaySpacing.sm),
                        Text(TefwayStrings.consentNote,
                            style: TefwayTextStyles.bodySecondary),
                      ],
                    ),
                  ),
                  const SizedBox(height: TefwaySpacing.lg),
                  Text(
                    TefwayStrings.consentSessionNote,
                    style: TefwayTextStyles.caption,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: TefwaySpacing.lg),
                ],
              ),
            ),
          ),
          _actions(),
        ],
      ),
    );
  }

  Widget _actions() {
    final refuse = TefwaySecondaryButton(
      label: TefwayStrings.consentRefuse,
      icon: Icons.block_rounded,
      onPressed: widget.onRefuse,
    );
    final allow = TefwayPrimaryButton(
      label: TefwayStrings.consentAllow,
      icon: Icons.check_rounded,
      onPressed: widget.onAllow,
    );
    final elevate = widget.onAllowAndElevate;
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.symmetric(
          horizontal: TefwaySpacing.xl, vertical: TefwaySpacing.md),
      decoration: const BoxDecoration(
        color: TefwayColors.deepNavy,
        border: Border(top: BorderSide(color: TefwayColors.border)),
      ),
      child: LayoutBuilder(
        builder: (context, constraints) {
          // Two labels cannot share a ~250px row, so they stack while the chat
          // panel is closed and sit side by side once the window widens.
          final stacked = constraints.maxWidth < 300;
          return Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              if (stacked) ...[
                allow,
                const SizedBox(height: TefwaySpacing.sm),
                refuse,
              ] else
                Row(
                  children: [
                    Expanded(child: refuse),
                    const SizedBox(width: TefwaySpacing.md),
                    Expanded(child: allow),
                  ],
                ),
              if (elevate != null) ...[
                const SizedBox(height: TefwaySpacing.sm),
                TefwaySecondaryButton(
                  label: TefwayStrings.consentAllowElevate,
                  icon: Icons.security_rounded,
                  onPressed: elevate,
                ),
              ],
            ],
          );
        },
      ),
    );
  }

  Widget _capabilityRow(TefwayCapability capability) {
    final enabled = _enabled[capability.key] ?? false;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Row(
        children: [
          Icon(
            capability.icon,
            size: 16,
            color: enabled ? TefwayColors.textPrimary : TefwayColors.textSecondary,
          ),
          const SizedBox(width: TefwaySpacing.sm),
          Expanded(
            child: Text(
              capability.label,
              style: enabled
                  ? TefwayTextStyles.body
                  : TefwayTextStyles.bodySecondary,
            ),
          ),
          Switch.adaptive(
            value: enabled,
            onChanged: (value) {
              setState(() => _enabled[capability.key] = value);
              widget.onToggle(capability.key, value);
            },
          ),
        ],
      ),
    );
  }
}
