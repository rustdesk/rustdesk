/// Decides how a soft-keyboard committed string is delivered to the peer.
///
/// Multi-character commits always use the KeyEvent.seq text channel (as
/// before). CJK IMEs usually commit one character at a time, and the raw-key
/// path cannot carry IME-committed text, so a single committed character also
/// uses seq — except control characters with dedicated key mappings
/// ('\n' -> VK_RETURN, ' ' -> VK_SPACE) and keystrokes made while a toolbar
/// modifier (Ctrl/Alt/Shift/Command) is engaged, which stay on the key-event
/// path so their key semantics and modifier flags are preserved.
bool mobileSoftKeyboardCommitUsesSeq(String committed,
    {required bool hasModifierOn}) {
  if (committed.isEmpty) return false;
  if (committed == '\n' || committed == ' ') return false;
  if (committed.length > 1) return true;
  return !hasModifierOn;
}
