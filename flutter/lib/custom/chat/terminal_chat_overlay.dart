import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:xterm/xterm.dart';

import '../input/input_bridge.dart';
import '../theme/tokens.dart';

// ─── Partial bar ─────────────────────────────────────────────────────────────

class TerminalChatPartialBar extends StatefulWidget {
  final InputBridge inputBridge;
  final VoidCallback onMaximize;
  final VoidCallback onClose;

  const TerminalChatPartialBar({
    super.key,
    required this.inputBridge,
    required this.onMaximize,
    required this.onClose,
  });

  @override
  State<TerminalChatPartialBar> createState() => _TerminalChatPartialBarState();
}

class _TerminalChatPartialBarState extends State<TerminalChatPartialBar> {
  final _textController = TextEditingController();
  final _focusNode = FocusNode();
  bool _sending = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focusNode.requestFocus());
  }

  @override
  void dispose() {
    _textController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _textController.text;
    if (text.isEmpty || _sending) return;
    setState(() => _sending = true);
    await widget.inputBridge.typeString(text);
    await widget.inputBridge.tapKey('return');
    if (mounted) {
      _textController.clear();
      widget.onClose();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: AppTokens.colorBgSurface,
        boxShadow: [
          BoxShadow(blurRadius: 8, color: Colors.black38, offset: Offset(0, -2)),
        ],
        borderRadius: BorderRadius.vertical(top: Radius.circular(AppTokens.radiusSheet)),
      ),
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceMd,
        vertical: AppTokens.spaceSm,
      ),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: _textController,
              focusNode: _focusNode,
              style: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 14,
                color: AppTokens.colorTextHigh,
              ),
              decoration: InputDecoration(
                hintText: 'Type a command…',
                hintStyle: TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 14,
                  color: AppTokens.colorTextMid.withValues(alpha: 0.5),
                ),
                border: InputBorder.none,
                contentPadding: EdgeInsets.zero,
                isDense: true,
              ),
              maxLines: 1,
              keyboardType: TextInputType.multiline,
              textInputAction: TextInputAction.send,
              onSubmitted: (_) => _send(),
            ),
          ),
          const SizedBox(width: AppTokens.spaceSm),
          _IconBtn(
            icon: Icons.open_in_full,
            tooltip: 'Maximize',
            onTap: widget.onMaximize,
          ),
          const SizedBox(width: AppTokens.spaceXs),
          _SendButton(sending: _sending, onSend: _send),
        ],
      ),
    );
  }
}

// ─── Max view ─────────────────────────────────────────────────────────────────

class TerminalChatMaxView extends StatefulWidget {
  final InputBridge inputBridge;
  final Terminal? terminal;
  final String terminalTitle;
  final VoidCallback onMinimize;
  final VoidCallback onClose;

  const TerminalChatMaxView({
    super.key,
    required this.inputBridge,
    required this.terminal,
    required this.terminalTitle,
    required this.onMinimize,
    required this.onClose,
  });

  @override
  State<TerminalChatMaxView> createState() => _TerminalChatMaxViewState();
}

class _TerminalChatMaxViewState extends State<TerminalChatMaxView> {
  final _textController = TextEditingController();
  final _focusNode = FocusNode();
  bool _sending = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _focusNode.requestFocus());
  }

  @override
  void dispose() {
    _textController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _textController.text;
    if (text.isEmpty || _sending) return;
    setState(() => _sending = true);
    await widget.inputBridge.typeString(text);
    await widget.inputBridge.tapKey('return');
    if (mounted) {
      _textController.clear();
      widget.onClose();
    }
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final keyboardHeight = mq.viewInsets.bottom;
    final overlayHeight = mq.size.height - mq.viewPadding.top - keyboardHeight;

    return AnimatedContainer(
      duration: const Duration(milliseconds: 250),
      curve: Curves.easeInOut,
      height: overlayHeight,
      decoration: const BoxDecoration(
        color: AppTokens.colorBgBase,
        boxShadow: [
          BoxShadow(blurRadius: 12, color: Colors.black38, offset: Offset(0, -3)),
        ],
        borderRadius: BorderRadius.vertical(top: Radius.circular(AppTokens.radiusSheet)),
      ),
      child: Column(
        children: [
          SafeArea(
            bottom: false,
            child: _Header(onMinimize: widget.onMinimize, onClose: widget.onClose),
          ),
          const Divider(height: 1, color: Color(0xFF2D3748)),
          _TerminalContext(
            terminal: widget.terminal,
            terminalTitle: widget.terminalTitle,
          ),
          const Divider(height: 1, color: Color(0xFF2D3748)),
          Expanded(
            child: _TextArea(
              controller: _textController,
              focusNode: _focusNode,
            ),
          ),
          _InputBar(sending: _sending, onSend: _send),
        ],
      ),
    );
  }
}

// ─── Terminal context panel ───────────────────────────────────────────────────

class _TerminalContext extends StatelessWidget {
  final Terminal? terminal;
  final String terminalTitle;

  const _TerminalContext({required this.terminal, required this.terminalTitle});

  bool get _isClaudeCode => terminalTitle.contains('⚡');

  @override
  Widget build(BuildContext context) {
    final t = terminal;
    if (t == null) {
      return _placeholder();
    }
    if (_isClaudeCode) {
      return _ClaudeCodeContext(terminal: t);
    }
    return _LastLinesContext(terminal: t);
  }

  Widget _placeholder() {
    return Container(
      color: AppTokens.colorBgSurface,
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceLg,
        vertical: AppTokens.spaceMd,
      ),
      child: const Text(
        '—',
        style: TextStyle(
          fontFamily: 'monospace',
          fontSize: 13,
          color: AppTokens.colorTextMid,
        ),
      ),
    );
  }
}

// Extracts the last Claude Code response by scanning buffer backwards for
// the second prompt line (> or ❯), then collects lines between prompts.
class _ClaudeCodeContext extends StatelessWidget {
  final Terminal terminal;

  const _ClaudeCodeContext({required this.terminal});

  static bool _isPromptLine(String line) {
    final t = line.trimLeft();
    return t.startsWith('> ') || t.startsWith('❯ ') || t == '>' || t == '❯';
  }

  static String _lineText(Terminal t, int absoluteRow) {
    final buf = t.buffer;
    if (absoluteRow < 0 || absoluteRow >= buf.lines.length) return '';
    final line = buf.lines[absoluteRow];
    final sb = StringBuffer();
    for (int col = 0; col < t.viewWidth; col++) {
      final cp = line.getCodePoint(col);
      if (cp == 0) break;
      sb.writeCharCode(cp);
    }
    return sb.toString().trimRight();
  }

  String _extractLastResponse() {
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
          // First prompt found — response ends just above here
          responseEnd = r - 1;
        } else {
          // Second prompt found — response starts just below here
          responseStart = r + 1;
          break;
        }
      }
    }

    if (responseStart < 0 || responseEnd < responseStart) {
      // Fallback: grab last 10 lines above cursor
      responseStart = (cursorAbs - 10).clamp(0, cursorAbs);
      responseEnd = cursorAbs - 1;
    }

    final lines = <String>[];
    for (int r = responseStart; r <= responseEnd; r++) {
      lines.add(_lineText(terminal, r));
    }
    // Trim leading/trailing blank lines
    while (lines.isNotEmpty && lines.first.isEmpty) { lines.removeAt(0); }
    while (lines.isNotEmpty && lines.last.isEmpty) { lines.removeLast(); }
    return lines.join('\n');
  }

  @override
  Widget build(BuildContext context) {
    final text = _extractLastResponse();
    return ConstrainedBox(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * 0.38,
      ),
      child: Container(
        width: double.infinity,
        color: AppTokens.colorBgSurface,
        padding: const EdgeInsets.symmetric(
          horizontal: AppTokens.spaceLg,
          vertical: AppTokens.spaceMd,
        ),
        child: text.isEmpty
            ? const Text(
                '—',
                style: TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 13,
                  color: AppTokens.colorTextMid,
                ),
              )
            : Scrollbar(
                child: SingleChildScrollView(
                  child: SelectableText(
                    text,
                    style: const TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 13,
                      color: AppTokens.colorTextHigh,
                    ),
                  ),
                ),
              ),
      ),
    );
  }
}

// Shows last 2 non-empty terminal lines for normal (non-Claude-Code) sessions.
class _LastLinesContext extends StatelessWidget {
  final Terminal terminal;

  const _LastLinesContext({required this.terminal});

  List<String> _lastLines() {
    final buf = terminal.buffer;
    final cursorAbs = buf.absoluteCursorY;
    final result = <String>[];
    for (int r = cursorAbs; r >= 0 && result.length < 2; r--) {
      final line = buf.lines[r];
      final sb = StringBuffer();
      for (int col = 0; col < terminal.viewWidth; col++) {
        final cp = line.getCodePoint(col);
        if (cp == 0) break;
        sb.writeCharCode(cp);
      }
      final text = sb.toString().trimRight();
      if (text.isNotEmpty) result.insert(0, text);
    }
    return result;
  }

  @override
  Widget build(BuildContext context) {
    final lines = _lastLines();
    return Container(
      width: double.infinity,
      color: AppTokens.colorBgSurface,
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceLg,
        vertical: AppTokens.spaceMd,
      ),
      child: lines.isEmpty
          ? const Text(
              '—',
              style: TextStyle(
                fontFamily: 'monospace',
                fontSize: 13,
                color: AppTokens.colorTextMid,
              ),
            )
          : Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: lines
                  .map((l) => Text(
                        l,
                        style: const TextStyle(
                          fontFamily: 'monospace',
                          fontSize: 13,
                          color: AppTokens.colorTextMid,
                        ),
                        overflow: TextOverflow.ellipsis,
                        maxLines: 1,
                      ))
                  .toList(),
            ),
    );
  }
}

// ─── Shared sub-widgets ───────────────────────────────────────────────────────

class _Header extends StatelessWidget {
  final VoidCallback onMinimize;
  final VoidCallback onClose;

  const _Header({required this.onMinimize, required this.onClose});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceMd,
        vertical: AppTokens.spaceSm,
      ),
      child: Row(
        children: [
          const _TerminalDots(),
          const SizedBox(width: AppTokens.spaceMd),
          Text(
            'Terminal',
            style: AppTokens.fontKeySmall.copyWith(color: AppTokens.colorTextMid),
          ),
          const Spacer(),
          _IconBtn(
            icon: Icons.close_fullscreen,
            tooltip: 'Minimize',
            onTap: onMinimize,
          ),
          const SizedBox(width: AppTokens.spaceXs),
          _IconBtn(
            icon: Icons.close,
            tooltip: 'Close',
            onTap: onClose,
          ),
        ],
      ),
    );
  }
}

class _TextArea extends StatelessWidget {
  final TextEditingController controller;
  final FocusNode focusNode;

  const _TextArea({required this.controller, required this.focusNode});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceLg,
        vertical: AppTokens.spaceSm,
      ),
      child: TextField(
        controller: controller,
        focusNode: focusNode,
        style: const TextStyle(
          fontFamily: 'monospace',
          fontSize: 14,
          color: AppTokens.colorTextHigh,
        ),
        decoration: InputDecoration(
          hintText: 'Type a command…',
          hintStyle: TextStyle(
            fontFamily: 'monospace',
            fontSize: 14,
            color: AppTokens.colorTextMid.withValues(alpha: 0.5),
          ),
          border: InputBorder.none,
          contentPadding: EdgeInsets.zero,
        ),
        maxLines: null,
        expands: true,
        textAlignVertical: TextAlignVertical.top,
        keyboardType: TextInputType.multiline,
        textInputAction: TextInputAction.newline,
      ),
    );
  }
}

class _InputBar extends StatelessWidget {
  final bool sending;
  final VoidCallback onSend;

  const _InputBar({required this.sending, required this.onSend});

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    return Container(
      padding: EdgeInsets.fromLTRB(
        AppTokens.spaceMd,
        AppTokens.spaceSm,
        AppTokens.spaceMd,
        AppTokens.spaceSm + mq.viewPadding.bottom,
      ),
      decoration: const BoxDecoration(
        color: AppTokens.colorBgSurface,
        border: Border(top: BorderSide(color: Color(0xFF2D3748))),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.end,
        children: [
          _SendButton(sending: sending, onSend: onSend),
        ],
      ),
    );
  }
}

class _SendButton extends StatelessWidget {
  final bool sending;
  final VoidCallback onSend;

  const _SendButton({required this.sending, required this.onSend});

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: sending ? null : () {
        HapticFeedback.lightImpact();
        onSend();
      },
      child: AnimatedOpacity(
        duration: const Duration(milliseconds: 150),
        opacity: sending ? 0.4 : 1.0,
        child: Container(
          width: 36,
          height: 36,
          decoration: BoxDecoration(
            color: AppTokens.colorPrimary,
            borderRadius: BorderRadius.circular(AppTokens.radiusKey),
          ),
          child: sending
              ? const Center(
                  child: SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: Colors.white,
                    ),
                  ),
                )
              : const Icon(Icons.send, size: 16, color: Colors.white),
        ),
      ),
    );
  }
}

class _TerminalDots extends StatelessWidget {
  const _TerminalDots();

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        _dot(const Color(0xFFFF5F57)),
        const SizedBox(width: 5),
        _dot(const Color(0xFFFFBD2E)),
        const SizedBox(width: 5),
        _dot(const Color(0xFF28C840)),
      ],
    );
  }

  Widget _dot(Color color) => Container(
        width: 10,
        height: 10,
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      );
}

class _IconBtn extends StatelessWidget {
  final IconData icon;
  final String tooltip;
  final VoidCallback onTap;

  const _IconBtn({required this.icon, required this.tooltip, required this.onTap});

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: GestureDetector(
        onTap: () {
          HapticFeedback.lightImpact();
          onTap();
        },
        child: Container(
          width: 32,
          height: 32,
          decoration: BoxDecoration(
            color: AppTokens.colorBgSurface,
            borderRadius: BorderRadius.circular(AppTokens.radiusKey),
          ),
          child: Icon(icon, size: 16, color: AppTokens.colorTextMid),
        ),
      ),
    );
  }
}
