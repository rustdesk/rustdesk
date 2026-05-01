import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../input/input_bridge.dart';
import '../settings/settings_store.dart';
import '../theme/tokens.dart';

class TerminalChatOverlay extends StatefulWidget {
  final InputBridge inputBridge;
  final bool startMaximized;
  final VoidCallback onClose;

  const TerminalChatOverlay({
    super.key,
    required this.inputBridge,
    required this.startMaximized,
    required this.onClose,
  });

  @override
  State<TerminalChatOverlay> createState() => _TerminalChatOverlayState();
}

class _TerminalChatOverlayState extends State<TerminalChatOverlay>
    with SingleTickerProviderStateMixin {
  final _textController = TextEditingController();
  final _scrollController = ScrollController();
  final _focusNode = FocusNode();
  final _commands = <String>[];
  bool _maximized = false;
  bool _sending = false;

  // Partial height: enough for ~4 command rows + input bar.
  static const _partialHeightFraction = 0.45;

  @override
  void initState() {
    super.initState();
    _maximized = widget.startMaximized;
    WidgetsBinding.instance.addPostFrameCallback((_) => _focusNode.requestFocus());
  }

  @override
  void dispose() {
    _textController.dispose();
    _scrollController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _textController.text;
    if (text.isEmpty || _sending) return;
    setState(() {
      _sending = true;
      _commands.add(text);
      _textController.clear();
    });
    // Scroll to bottom after adding the command.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 150),
          curve: Curves.easeOut,
        );
      }
    });
    await widget.inputBridge.typeString(text);
    await widget.inputBridge.tapKey('return');
    if (mounted) setState(() => _sending = false);
  }

  void _toggleMaximize() {
    final newVal = !_maximized;
    if (newVal && !settingsStore.chatStartMaximized) {
      // First time maximizing — ask if they want to remember the choice.
      _showRememberDialog(newVal);
    } else {
      setState(() => _maximized = newVal);
    }
  }

  void _showRememberDialog(bool newMaximized) {
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: AppTokens.colorBgSurface,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(AppTokens.radiusCard),
        ),
        title: Text(
          'Always open maximized?',
          style: AppTokens.fontKey.copyWith(color: AppTokens.colorTextHigh),
        ),
        content: Text(
          'Start the chat panel maximized every time you open it?',
          style: AppTokens.fontBody.copyWith(color: AppTokens.colorTextMid),
        ),
        actions: [
          TextButton(
            onPressed: () {
              Navigator.of(ctx).pop();
              setState(() => _maximized = newMaximized);
            },
            child: Text('Just this once',
                style: AppTokens.fontKeySmall
                    .copyWith(color: AppTokens.colorTextMid)),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: AppTokens.colorPrimary,
            ),
            onPressed: () {
              Navigator.of(ctx).pop();
              settingsStore.setChatStartMaximized(true);
              setState(() => _maximized = newMaximized);
            },
            child: Text('Always',
                style: AppTokens.fontKeySmall
                    .copyWith(color: AppTokens.colorTextHigh)),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    final screenHeight = mq.size.height;
    final overlayHeight = _maximized
        ? screenHeight
        : screenHeight * _partialHeightFraction;

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
          _Header(
            maximized: _maximized,
            onMaximize: _toggleMaximize,
            onClose: widget.onClose,
          ),
          const Divider(height: 1, color: Color(0xFF2D3748)),
          Expanded(child: _CommandHistory(
            commands: _commands,
            scrollController: _scrollController,
          )),
          _InputBar(
            controller: _textController,
            focusNode: _focusNode,
            sending: _sending,
            onSend: _send,
            onReturn: () => widget.inputBridge.tapKey('return'),
          ),
        ],
      ),
    );
  }
}

class _Header extends StatelessWidget {
  final bool maximized;
  final VoidCallback onMaximize;
  final VoidCallback onClose;

  const _Header({
    required this.maximized,
    required this.onMaximize,
    required this.onClose,
  });

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
            icon: maximized ? Icons.unfold_less : Icons.unfold_more,
            tooltip: maximized ? 'Partial view' : 'Maximize',
            onTap: onMaximize,
          ),
          const SizedBox(width: AppTokens.spaceXs),
          _IconBtn(
            icon: Icons.close,
            tooltip: 'Close chat',
            onTap: onClose,
          ),
        ],
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

class _CommandHistory extends StatelessWidget {
  final List<String> commands;
  final ScrollController scrollController;

  const _CommandHistory({required this.commands, required this.scrollController});

  @override
  Widget build(BuildContext context) {
    if (commands.isEmpty) {
      return Center(
        child: Text(
          'Type a command and press Send',
          style: AppTokens.fontBody.copyWith(color: AppTokens.colorTextMid),
        ),
      );
    }
    return ListView.builder(
      controller: scrollController,
      padding: const EdgeInsets.symmetric(
        horizontal: AppTokens.spaceLg,
        vertical: AppTokens.spaceSm,
      ),
      itemCount: commands.length,
      itemBuilder: (_, i) => _CommandRow(text: commands[i], index: i),
    );
  }
}

class _CommandRow extends StatelessWidget {
  final String text;
  final int index;

  const _CommandRow({required this.text, required this.index});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            '${(index + 1).toString().padLeft(2)} ',
            style: const TextStyle(
              fontFamily: 'monospace',
              fontSize: 12,
              color: Color(0xFF4A5568),
            ),
          ),
          Text(
            '\$ ',
            style: const TextStyle(
              fontFamily: 'monospace',
              fontSize: 13,
              color: Color(0xFF68D391),
              fontWeight: FontWeight.w600,
            ),
          ),
          Expanded(
            child: Text(
              text,
              style: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 13,
                color: Color(0xFFE2E8F0),
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _InputBar extends StatelessWidget {
  final TextEditingController controller;
  final FocusNode focusNode;
  final bool sending;
  final VoidCallback onSend;
  final VoidCallback onReturn;

  const _InputBar({
    required this.controller,
    required this.focusNode,
    required this.sending,
    required this.onSend,
    required this.onReturn,
  });

  @override
  Widget build(BuildContext context) {
    final mq = MediaQuery.of(context);
    return Container(
      padding: EdgeInsets.fromLTRB(
        AppTokens.spaceMd,
        AppTokens.spaceSm,
        AppTokens.spaceMd,
        AppTokens.spaceSm + mq.viewInsets.bottom,
      ),
      decoration: const BoxDecoration(
        color: AppTokens.colorBgSurface,
        border: Border(top: BorderSide(color: Color(0xFF2D3748))),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            '\$',
            style: const TextStyle(
              fontFamily: 'monospace',
              fontSize: 15,
              color: Color(0xFF68D391),
              fontWeight: FontWeight.w700,
            ),
          ),
          const SizedBox(width: AppTokens.spaceSm),
          Expanded(
            child: TextField(
              controller: controller,
              focusNode: focusNode,
              style: const TextStyle(
                fontFamily: 'monospace',
                fontSize: 14,
                color: AppTokens.colorTextHigh,
              ),
              decoration: InputDecoration(
                hintText: 'command...',
                hintStyle: TextStyle(
                  fontFamily: 'monospace',
                  fontSize: 14,
                  color: AppTokens.colorTextMid.withValues(alpha: 0.5),
                ),
                isDense: true,
                border: InputBorder.none,
                contentPadding: const EdgeInsets.symmetric(vertical: 6),
              ),
              minLines: 1,
              maxLines: 4,
              textInputAction: TextInputAction.newline,
              onSubmitted: (_) => onReturn(),
            ),
          ),
          const SizedBox(width: AppTokens.spaceSm),
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
