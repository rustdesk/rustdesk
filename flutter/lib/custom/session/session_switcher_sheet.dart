import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/custom/session/session_registry.dart';
import 'package:flutter_hbb/custom/theme/tokens.dart';

class SessionSwitcherSheet extends StatelessWidget {
  final SessionID activeSessionId;
  final ValueChanged<SessionID> onSwitch;
  final VoidCallback onAddSession;

  const SessionSwitcherSheet({
    super.key,
    required this.activeSessionId,
    required this.onSwitch,
    required this.onAddSession,
  });

  @override
  Widget build(BuildContext context) {
    final registry = SessionRegistry.instance;
    final entries = registry.entries;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 16),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                const Text(
                  'Sessions',
                  style: TextStyle(
                    color: Colors.white70,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 0.5,
                  ),
                ),
                if (!registry.isFull)
                  IconButton(
                    onPressed: () {
                      Navigator.of(context).pop();
                      onAddSession();
                    },
                    icon: const Icon(Icons.add, color: Colors.white70, size: 20),
                    tooltip: 'Add session',
                    padding: EdgeInsets.zero,
                    constraints: const BoxConstraints(),
                  ),
              ],
            ),
            const SizedBox(height: 8),
            ...entries.map((entry) {
              final isActive = entry.ffi.sessionId == activeSessionId;
              return _SessionRow(
                entry: entry,
                isActive: isActive,
                onTap: () {
                  HapticFeedback.selectionClick();
                  Navigator.of(context).pop();
                  if (!isActive) onSwitch(entry.ffi.sessionId);
                },
              );
            }),
            if (registry.isFull)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  'Maximum $kMaxSessions sessions reached',
                  style: const TextStyle(color: AppTokens.colorTextMid, fontSize: 12),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _SessionRow extends StatelessWidget {
  final SessionEntry entry;
  final bool isActive;
  final VoidCallback onTap;

  const _SessionRow({required this.entry, required this.isActive, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Material(
        color: isActive
            ? AppTokens.colorPrimary.withOpacity(0.15)
            : AppTokens.colorBgSurface,
        borderRadius: BorderRadius.circular(AppTokens.radiusCard),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
            child: Row(
              children: [
                Icon(
                  Icons.computer,
                  color: isActive ? AppTokens.colorPrimary : AppTokens.colorTextMid,
                  size: 18,
                ),
                const SizedBox(width: 10),
                Expanded(
                  child: Text(
                    entry.label,
                    style: TextStyle(
                      color: isActive ? AppTokens.colorTextHigh : AppTokens.colorTextMid,
                      fontSize: 14,
                      fontWeight: isActive ? FontWeight.w600 : FontWeight.w400,
                    ),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                if (isActive)
                  const Icon(Icons.radio_button_checked,
                      color: AppTokens.colorPrimary, size: 16),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
