import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../input/input_bridge.dart';
import '../theme/tokens.dart';

class TerminalChatOverlay extends StatefulWidget {
  final InputBridge inputBridge;
  final VoidCallback onClose;

  const TerminalChatOverlay({
    super.key,
    required this.inputBridge,
    required this.onClose,
  });

  @override
  State<TerminalChatOverlay> createState() => _TerminalChatOverlayState();
}

class _TerminalChatOverlayState extends State<TerminalChatOverlay> {
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
            child: _Header(onClose: widget.onClose),
          ),
          const Divider(height: 1, color: Color(0xFF2D3748)),
          Expanded(
            child: _TextArea(
              controller: _textController,
              focusNode: _focusNode,
            ),
          ),
          _InputBar(
            sending: _sending,
            onSend: _send,
          ),
        ],
      ),
    );
  }
}

class _Header extends StatelessWidget {
  final VoidCallback onClose;

  const _Header({required this.onClose});

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
