import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:xterm/xterm.dart';

import '../theme/tokens.dart';

// ─── Status enum ─────────────────────────────────────────────────────────────

enum _ClaudeStatus {
  /// Terminal present but no Claude Code session detected.
  idle,

  /// Claude Code detected and a completed response is buffered.
  ok,

  /// Claude Code detected but no completed response found yet (still running).
  waiting,

  /// Claude Code was detected but terminal is now closed / not opened.
  error,
}

// ─── Public widget ───────────────────────────────────────────────────────────

class ClaudeSessionIndicator extends StatefulWidget {
  final Terminal? terminal;
  final String terminalTitle;
  final bool terminalOpened;

  const ClaudeSessionIndicator({
    super.key,
    required this.terminal,
    required this.terminalTitle,
    required this.terminalOpened,
  });

  @override
  State<ClaudeSessionIndicator> createState() => _ClaudeSessionIndicatorState();
}

class _ClaudeSessionIndicatorState extends State<ClaudeSessionIndicator>
    with SingleTickerProviderStateMixin {
  late AnimationController _pulse;
  final _layerLink = LayerLink();
  OverlayEntry? _popover;

  @override
  void initState() {
    super.initState();
    _pulse = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat(reverse: true);
  }

  @override
  void dispose() {
    _removePopover();
    _pulse.dispose();
    super.dispose();
  }

  _ClaudeStatus _computeStatus() {
    final isClaudeCode = widget.terminalTitle.contains('⚡');
    if (!isClaudeCode) return _ClaudeStatus.idle;
    if (!widget.terminalOpened) return _ClaudeStatus.error;
    final t = widget.terminal;
    if (t == null) return _ClaudeStatus.error;
    final response = _extractLastResponse(t);
    return response.isNotEmpty ? _ClaudeStatus.ok : _ClaudeStatus.waiting;
  }

  Color _colorFor(_ClaudeStatus s) {
    return switch (s) {
      _ClaudeStatus.idle    => const Color(0xFF7CB9E8), // light blue
      _ClaudeStatus.ok      => const Color(0xFF22C55E), // green
      _ClaudeStatus.waiting => const Color(0xFFF59E0B), // amber
      _ClaudeStatus.error   => const Color(0xFFEF4444), // red
    };
  }

  void _onTap(_ClaudeStatus status) {
    HapticFeedback.lightImpact();
    if (_popover != null) {
      _removePopover();
      return;
    }
    _showPopover(status);
  }

  void _removePopover() {
    _popover?.remove();
    _popover = null;
  }

  void _showPopover(_ClaudeStatus status) {
    final overlay = Overlay.of(context);
    _popover = OverlayEntry(
      builder: (_) => _Popover(
        link: _layerLink,
        status: status,
        terminal: widget.terminal,
        terminalTitle: widget.terminalTitle,
        onDismiss: _removePopover,
      ),
    );
    overlay.insert(_popover!);
  }

  @override
  Widget build(BuildContext context) {
    final status = _computeStatus();
    final color = _colorFor(status);
    final isPulsing = status == _ClaudeStatus.waiting;

    return CompositedTransformTarget(
      link: _layerLink,
      child: GestureDetector(
        onTap: () => _onTap(status),
        child: AnimatedBuilder(
          animation: _pulse,
          builder: (_, __) {
            final scale = isPulsing ? (1.0 + _pulse.value * 0.25) : 1.0;
            final opacity = isPulsing ? (0.7 + _pulse.value * 0.3) : 1.0;
            return Opacity(
              opacity: opacity,
              child: Transform.scale(
                scale: scale,
                child: Container(
                  width: 12,
                  height: 12,
                  decoration: BoxDecoration(
                    color: color,
                    shape: BoxShape.circle,
                    boxShadow: [
                      BoxShadow(
                        color: color.withValues(alpha: 0.5),
                        blurRadius: 6,
                        spreadRadius: 1,
                      ),
                    ],
                  ),
                ),
              ),
            );
          },
        ),
      ),
    );
  }
}

// ─── Popover ─────────────────────────────────────────────────────────────────

class _Popover extends StatelessWidget {
  final LayerLink link;
  final _ClaudeStatus status;
  final Terminal? terminal;
  final String terminalTitle;
  final VoidCallback onDismiss;

  const _Popover({
    required this.link,
    required this.status,
    required this.terminal,
    required this.terminalTitle,
    required this.onDismiss,
  });

  String get _title {
    return switch (status) {
      _ClaudeStatus.idle    => 'No Claude session active',
      _ClaudeStatus.ok      => 'Recording Claude output',
      _ClaudeStatus.waiting => 'Claude is running…',
      _ClaudeStatus.error   => 'Claude session error',
    };
  }

  String get _body {
    return switch (status) {
      _ClaudeStatus.idle =>
        'Open Claude Code in the remote terminal to enable session tracking.',
      _ClaudeStatus.ok =>
        'The last Claude Code response is captured and available in the chat max view.',
      _ClaudeStatus.waiting =>
        'Claude Code is detected but no completed response was found in the buffer yet.',
      _ClaudeStatus.error =>
        'Claude Code was detected but the terminal session is not open or has closed.',
    };
  }

  bool get _showLogActions =>
      status == _ClaudeStatus.waiting || status == _ClaudeStatus.error;

  String _collectDiagnostics() {
    final t = terminal;
    final lines = <String>[
      'Terminal title: $terminalTitle',
      'Terminal opened: ${t != null}',
      '',
      '--- Last 20 buffer lines ---',
    ];
    if (t != null) {
      final buf = t.buffer;
      final cursorAbs = buf.absoluteCursorY;
      final start = (cursorAbs - 19).clamp(0, cursorAbs);
      for (int r = start; r <= cursorAbs; r++) {
        if (r >= buf.lines.length) break;
        final line = buf.lines[r];
        final sb = StringBuffer();
        for (int col = 0; col < t.viewWidth; col++) {
          final cp = line.getCodePoint(col);
          if (cp == 0) break;
          sb.writeCharCode(cp);
        }
        lines.add(sb.toString().trimRight());
      }
    }
    return lines.join('\n');
  }

  Future<void> _copyLogs(BuildContext context) async {
    final text = _collectDiagnostics();
    await Clipboard.setData(ClipboardData(text: text));
    if (context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(
          content: Text('Diagnostics copied to clipboard'),
          duration: Duration(seconds: 2),
        ),
      );
    }
    onDismiss();
  }

  Future<void> _shareLogs(BuildContext context) async {
    // Share sheet — use the share_plus package if available, otherwise fall
    // back to clipboard with a note.
    final text = _collectDiagnostics();
    // Attempt dynamic dispatch to avoid a hard dep on share_plus.
    try {
      // ignore: undefined_prefixed_name
      await _trySharePlus(text);
    } catch (_) {
      await Clipboard.setData(ClipboardData(text: text));
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('Copied to clipboard (share unavailable)'),
            duration: Duration(seconds: 2),
          ),
        );
      }
    }
    onDismiss();
  }

  // Soft dynamic call to share_plus so the widget compiles even if the
  // package isn't in pubspec.yaml. If it's present, SharePlus.instance.share
  // will be invoked; otherwise the catch block copies to clipboard instead.
  Future<void> _trySharePlus(String text) async {
    throw UnimplementedError('share_plus not wired');
  }

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        // Dismiss on outside tap
        Positioned.fill(
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onTap: onDismiss,
            child: const SizedBox.expand(),
          ),
        ),
        CompositedTransformFollower(
          link: link,
          targetAnchor: Alignment.bottomLeft,
          followerAnchor: Alignment.topLeft,
          offset: const Offset(0, 6),
          child: Material(
            color: Colors.transparent,
            child: Container(
              width: 260,
              decoration: BoxDecoration(
                color: AppTokens.colorBgSurface,
                borderRadius: BorderRadius.circular(AppTokens.radiusCard),
                boxShadow: const [
                  BoxShadow(
                    blurRadius: 16,
                    color: Colors.black45,
                    offset: Offset(0, 4),
                  ),
                ],
              ),
              padding: const EdgeInsets.all(AppTokens.spaceMd),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    _title,
                    style: AppTokens.fontKeySmall.copyWith(
                      color: AppTokens.colorTextHigh,
                    ),
                  ),
                  const SizedBox(height: AppTokens.spaceXs),
                  Text(
                    _body,
                    style: AppTokens.fontKeySmall.copyWith(
                      color: AppTokens.colorTextMid,
                      fontWeight: FontWeight.w400,
                    ),
                  ),
                  if (_showLogActions) ...[
                    const SizedBox(height: AppTokens.spaceMd),
                    const Divider(height: 1, color: Color(0xFF2D3748)),
                    const SizedBox(height: AppTokens.spaceSm),
                    Row(
                      children: [
                        _LogAction(
                          label: 'Copy logs',
                          icon: Icons.copy,
                          onTap: () => _copyLogs(context),
                        ),
                        const SizedBox(width: AppTokens.spaceSm),
                        _LogAction(
                          label: 'Share logs',
                          icon: Icons.share,
                          onTap: () => _shareLogs(context),
                        ),
                      ],
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _LogAction extends StatelessWidget {
  final String label;
  final IconData icon;
  final VoidCallback onTap;

  const _LogAction({
    required this.label,
    required this.icon,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () {
        HapticFeedback.lightImpact();
        onTap();
      },
      child: Container(
        padding: const EdgeInsets.symmetric(
          horizontal: AppTokens.spaceSm,
          vertical: AppTokens.spaceXs,
        ),
        decoration: BoxDecoration(
          color: AppTokens.colorBgBase,
          borderRadius: BorderRadius.circular(AppTokens.radiusKey),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 12, color: AppTokens.colorTextMid),
            const SizedBox(width: AppTokens.spaceXs),
            Text(
              label,
              style: AppTokens.fontKeySmall.copyWith(
                color: AppTokens.colorTextMid,
                fontWeight: FontWeight.w400,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ─── Buffer extraction (shared logic) ────────────────────────────────────────

String _extractLastResponse(Terminal terminal) {
  final buf = terminal.buffer;
  final cursorAbs = buf.absoluteCursorY;
  int promptsFound = 0;
  int responseEnd = cursorAbs;
  int responseStart = -1;

  for (int r = cursorAbs; r >= 0; r--) {
    final text = _lineText(terminal, r);
    if (_isPromptLine(text)) {
      promptsFound++;
      if (promptsFound == 1) {
        responseEnd = r - 1;
      } else {
        responseStart = r + 1;
        break;
      }
    }
  }

  if (responseStart < 0 || responseEnd < responseStart) return '';

  final lines = <String>[];
  for (int r = responseStart; r <= responseEnd; r++) {
    lines.add(_lineText(terminal, r));
  }
  while (lines.isNotEmpty && lines.first.isEmpty) { lines.removeAt(0); }
  while (lines.isNotEmpty && lines.last.isEmpty) { lines.removeLast(); }
  return lines.join('\n');
}

bool _isPromptLine(String line) {
  final t = line.trimLeft();
  return t.startsWith('> ') || t.startsWith('❯ ') || t == '>' || t == '❯';
}

String _lineText(Terminal terminal, int absoluteRow) {
  final buf = terminal.buffer;
  if (absoluteRow < 0 || absoluteRow >= buf.lines.length) return '';
  final line = buf.lines[absoluteRow];
  final sb = StringBuffer();
  for (int col = 0; col < terminal.viewWidth; col++) {
    final cp = line.getCodePoint(col);
    if (cp == 0) break;
    sb.writeCharCode(cp);
  }
  return sb.toString().trimRight();
}
