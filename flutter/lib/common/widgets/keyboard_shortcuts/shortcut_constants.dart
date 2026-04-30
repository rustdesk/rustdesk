/// Keyboard shortcut action IDs - must match
/// src/keyboard/shortcuts.rs::action_id.
const kShortcutActionSendCtrlAltDel = 'send_ctrl_alt_del';
const kShortcutActionToggleFullscreen = 'toggle_fullscreen';
const kShortcutActionSwitchDisplayNext = 'switch_display_next';
const kShortcutActionSwitchDisplayPrev = 'switch_display_prev';
const kShortcutActionSwitchDisplayAll = 'switch_display_all';
const kShortcutActionScreenshot = 'screenshot';
const kShortcutActionInsertLock = 'insert_lock';
const kShortcutActionRefresh = 'refresh';
const kShortcutActionToggleBlockInput = 'toggle_block_input';
const kShortcutActionToggleRecording = 'toggle_recording';
const kShortcutActionSwitchSides = 'switch_sides';
const kShortcutActionCloseTab = 'close_tab';
const kShortcutActionToggleToolbar = 'toggle_toolbar';
const kShortcutActionRestartRemote = 'restart_remote';
const kShortcutActionResetCanvas = 'reset_canvas';
const kShortcutActionSwitchTabNext = 'switch_tab_next';
const kShortcutActionSwitchTabPrev = 'switch_tab_prev';
const kShortcutActionToggleMute = 'toggle_mute';
const kShortcutActionPinToolbar = 'pin_toolbar';
const kShortcutActionViewModeOriginal = 'view_mode_original';
const kShortcutActionViewModeAdaptive = 'view_mode_adaptive';
const kShortcutActionToggleChat = 'toggle_chat';
const kShortcutActionToggleQualityMonitor = 'toggle_quality_monitor';
const kShortcutActionToggleShowRemoteCursor = 'toggle_show_remote_cursor';
const kShortcutActionToggleShowMyCursor = 'toggle_show_my_cursor';
const kShortcutActionToggleDisableClipboard = 'toggle_disable_clipboard';
const kShortcutActionPrivacyMode1 = 'privacy_mode_1';
const kShortcutActionPrivacyMode2 = 'privacy_mode_2';
// Keyboard mode (Map / Translate / Legacy).
const kShortcutActionKeyboardModeMap = 'keyboard_mode_map';
const kShortcutActionKeyboardModeTranslate = 'keyboard_mode_translate';
const kShortcutActionKeyboardModeLegacy = 'keyboard_mode_legacy';
// Codec preference (Auto + the four optional codecs the toolbar surfaces).
const kShortcutActionCodecAuto = 'codec_auto';
const kShortcutActionCodecVp8 = 'codec_vp8';
const kShortcutActionCodecVp9 = 'codec_vp9';
const kShortcutActionCodecAv1 = 'codec_av1';
const kShortcutActionCodecH264 = 'codec_h264';
const kShortcutActionCodecH265 = 'codec_h265';
// Plug out every virtual display in one shot — toolbar exposes this in
// both IDD modes (RustDesk and Amyuni). Per-index virtual-display toggles
// (RustDesk IDD's 4 checkboxes) and the +/- count buttons (Amyuni-only)
// are NOT exposed as shortcuts: per-index is too granular, and +/- has
// no toolbar counterpart on RustDesk IDD peers.
const kShortcutActionPlugOutAllVirtualDisplays =
    'plug_out_all_virtual_displays';
const kShortcutActionToggleRelativeMouseMode = 'toggle_relative_mouse_mode';
const kShortcutActionToggleFollowRemoteCursor = 'toggle_follow_remote_cursor';
const kShortcutActionToggleFollowRemoteWindow = 'toggle_follow_remote_window';
const kShortcutActionToggleZoomCursor = 'toggle_zoom_cursor';
const kShortcutActionToggleReverseMouseWheel = 'toggle_reverse_mouse_wheel';
const kShortcutActionToggleSwapLeftRightMouse = 'toggle_swap_left_right_mouse';
const kShortcutActionToggleLockAfterSessionEnd = 'toggle_lock_after_session_end';
const kShortcutActionToggleTrueColor = 'toggle_true_color';
const kShortcutActionToggleSwapCtrlCmd = 'toggle_swap_ctrl_cmd';
const kShortcutActionToggleEnableFileCopyPaste = 'toggle_enable_file_copy_paste';
const kShortcutActionViewModeCustom = 'view_mode_custom';
const kShortcutActionImageQualityBest = 'image_quality_best';
const kShortcutActionImageQualityBalanced = 'image_quality_balanced';
const kShortcutActionImageQualityLow = 'image_quality_low';
const kShortcutActionSendClipboardKeystrokes = 'send_clipboard_keystrokes';
const kShortcutActionToggleInputSource = 'toggle_input_source';
const kShortcutActionToggleVoiceCall = 'toggle_voice_call';
const kShortcutActionToggleViewOnly = 'toggle_view_only';

const kShortcutLocalConfigKey = 'keyboard-shortcuts';
const kShortcutEventName = 'shortcut_triggered';

/// Canonical default keyboard-shortcut bindings, mirroring Rust's
/// `default_bindings()` in `src/keyboard/shortcuts.rs`. Used by:
///   * the Web bridge (`flutter/lib/web/bridge.dart::mainGetDefaultKeyboardShortcuts`)
///     — Web has no Rust at runtime, so the seed list is read from this Dart
///     constant instead of going through FFI.
///   * the configuration page when seeding defaults on first enable, after
///     [filterDefaultBindingsForPlatform] has trimmed platform-specific
///     entries.
///
/// Parity with Rust is unit-tested on both sides against
/// `flutter/test/fixtures/default_keyboard_shortcuts.json` — see the
/// `kDefaultShortcutBindings matches fixture` test in
/// `flutter/test/keyboard_shortcuts_test.dart` and
/// `default_bindings_match_fixture_json` in `src/keyboard/shortcuts.rs`.
/// Any change here MUST also update the fixture and the Rust source, or CI
/// will fail in the side that drifted.
final List<Map<String, Object>> kDefaultShortcutBindings = [
  for (final entry in <List<Object>>[
    [kShortcutActionSendCtrlAltDel,         'delete'],
    [kShortcutActionToggleFullscreen,       'enter'],
    [kShortcutActionSwitchDisplayNext,      'arrow_right'],
    [kShortcutActionSwitchDisplayPrev,      'arrow_left'],
    [kShortcutActionScreenshot,             'p'],
    [kShortcutActionToggleShowRemoteCursor, 'm'],
    [kShortcutActionToggleMute,             's'],
    [kShortcutActionToggleBlockInput,       'i'],
    [kShortcutActionToggleChat,             'c'],
  ])
    {
      'action': entry[0],
      'mods': const ['primary', 'alt', 'shift'],
      'key': entry[1],
    },
];
