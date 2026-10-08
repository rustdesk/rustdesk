part of 'terminal_mouse_handler.dart';

const _prepareTerminalClipboardCommand = 'prepare_terminal_clipboard';
const _finishTerminalClipboardCommand = 'finish_terminal_clipboard';
const _cancelTerminalClipboardCommand = 'cancel_terminal_clipboard';

extension _TerminalWebClipboardGesture on _TerminalMouseInteractionState {
  void _prepareTerminalClipboardWrite() {
    if (!kIsWeb) return;
    _cancelTerminalClipboardWrite();
    final terminal = widget.terminal;
    if (terminal is! RustDeskTerminal || !terminal.isClipboardWriteAllowed) {
      return;
    }
    try {
      ffiSetByName(_prepareTerminalClipboardCommand);
      _terminalClipboardGesturePrepared = true;
    } catch (error) {
      debugPrint('[Terminal] Failed to prepare Web clipboard write: $error');
    }
  }

  void _finishTerminalClipboardWrite(bool responseExpected) {
    if (!_terminalClipboardGesturePrepared) return;
    _terminalClipboardGesturePrepared = false;
    if (!kIsWeb) return;
    try {
      ffiSetByName(
        _finishTerminalClipboardCommand,
        responseExpected ? 'true' : 'false',
      );
    } catch (error) {
      debugPrint('[Terminal] Failed to finish Web clipboard write: $error');
    }
  }

  void _cancelTerminalClipboardWrite() {
    if (!_terminalClipboardGesturePrepared) return;
    _terminalClipboardGesturePrepared = false;
    _sendTerminalClipboardCancel();
  }

  void _discardPendingTerminalClipboardWrites() {
    _cancelTerminalClipboardWrite();
    _sendTerminalClipboardCancel();
  }

  void _sendTerminalClipboardCancel() {
    if (!kIsWeb) return;
    try {
      ffiSetByName(_cancelTerminalClipboardCommand);
    } catch (error) {
      debugPrint('[Terminal] Failed to cancel Web clipboard write: $error');
    }
  }
}
