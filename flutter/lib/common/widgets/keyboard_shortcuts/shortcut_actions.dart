import 'shortcut_constants.dart';
import 'shortcut_utils.dart';

/// Marker for the union of [KeyboardShortcutActionEntry] /
/// [KeyboardShortcutActionSubgroup] — anything a top-level
/// [KeyboardShortcutActionGroup] can directly contain. Sealed so renderers
/// and filters can `switch` on it without a default branch.
sealed class KeyboardShortcutActionGroupChild {
  const KeyboardShortcutActionGroupChild();
}

/// One configurable action — id + i18n key for its label.
class KeyboardShortcutActionEntry extends KeyboardShortcutActionGroupChild {
  final String id;
  final String labelKey;
  const KeyboardShortcutActionEntry(this.id, this.labelKey);
}

/// A nested subgroup (e.g. "View Mode" under "Display"). Renders with extra
/// indent so its items are visually distinguished from the parent group's
/// direct items.
class KeyboardShortcutActionSubgroup extends KeyboardShortcutActionGroupChild {
  final String titleKey;
  final List<KeyboardShortcutActionEntry> entries;
  const KeyboardShortcutActionSubgroup(this.titleKey, this.entries);
}

/// A top-level group ("Display", "Keyboard", "Chat", …). `children` is an
/// *ordered* mix of direct entries and subgroups, so layouts like
/// "subgroups first → direct items → trailing subgroup" — exactly the
/// shape `_DisplayMenu` uses (Privacy mode lives after the cursor / display
/// toggles direct items) — are first-class instead of needing a wrapper
/// "Display Settings" subgroup just to insert the items.
class KeyboardShortcutActionGroup {
  final String titleKey;
  final List<KeyboardShortcutActionGroupChild> children;
  const KeyboardShortcutActionGroup(this.titleKey, this.children);
}

/// Canonical action group definitions used by both the desktop and mobile
/// configuration pages. The order of groups, subgroups, and entries here
/// is the order the user sees in the UI, and mirrors the corresponding
/// toolbar submenu (`_DisplayMenu` / `_KeyboardMenu` in
/// `desktop/widgets/remote_toolbar.dart`) child order — modulo entries
/// without shortcut counterparts (e.g. `_screenAdjustor.adjustWindow`,
/// `scrollStyle`, `_ResolutionsMenu`, `localKeyboardType`).
final List<KeyboardShortcutActionGroup> kKeyboardShortcutActionGroups = [
  KeyboardShortcutActionGroup('Monitor', [
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayNext, 'Switch to next display'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayPrev, 'Switch to previous display'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchDisplayAll, 'All monitors'),
  ]),
  KeyboardShortcutActionGroup('Control Actions', [
    KeyboardShortcutActionEntry(
        kShortcutActionSendClipboardKeystrokes, 'Send clipboard keystrokes'),
    KeyboardShortcutActionEntry(kShortcutActionResetCanvas, 'Reset canvas'),
    KeyboardShortcutActionEntry(
        kShortcutActionSendCtrlAltDel, 'Insert Ctrl + Alt + Del'),
    KeyboardShortcutActionEntry(
        kShortcutActionRestartRemote, 'Restart remote device'),
    KeyboardShortcutActionEntry(kShortcutActionInsertLock, 'Insert Lock'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleBlockInput, 'Block user input'),
    KeyboardShortcutActionEntry(kShortcutActionSwitchSides, 'Switch Sides'),
    KeyboardShortcutActionEntry(kShortcutActionRefresh, 'Refresh'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleRecording, 'Toggle session recording'),
    KeyboardShortcutActionEntry(kShortcutActionScreenshot, 'Take screenshot'),
  ]),
  // Display: subgroups (View Mode → Image Quality → Codec → Virtual display)
  // first, then the direct items (cursor toggles + display toggles), then
  // Privacy mode subgroup last — matching `_DisplayMenu.menuChildrenGetter`
  // exactly. Rebalancing this order should also rebalance the toolbar.
  KeyboardShortcutActionGroup('Display', [
    KeyboardShortcutActionSubgroup('View Mode', [
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeOriginal, 'Scale original'),
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeAdaptive, 'Scale adaptive'),
      KeyboardShortcutActionEntry(
          kShortcutActionViewModeCustom, 'Scale custom'),
    ]),
    KeyboardShortcutActionSubgroup('Image Quality', [
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityBest, 'Good image quality'),
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityBalanced, 'Balanced'),
      KeyboardShortcutActionEntry(
          kShortcutActionImageQualityLow, 'Optimize reaction time'),
    ]),
    KeyboardShortcutActionSubgroup('Codec', [
      KeyboardShortcutActionEntry(kShortcutActionCodecAuto, 'Auto'),
      KeyboardShortcutActionEntry(kShortcutActionCodecVp8, 'VP8'),
      KeyboardShortcutActionEntry(kShortcutActionCodecVp9, 'VP9'),
      KeyboardShortcutActionEntry(kShortcutActionCodecAv1, 'AV1'),
      KeyboardShortcutActionEntry(kShortcutActionCodecH264, 'H264'),
      KeyboardShortcutActionEntry(kShortcutActionCodecH265, 'H265'),
    ]),
    KeyboardShortcutActionSubgroup('Virtual display', [
      KeyboardShortcutActionEntry(
          kShortcutActionPlugOutAllVirtualDisplays, 'Plug out all'),
    ]),
    // Direct items: cursorToggles + display toggles, in toolbar order.
    KeyboardShortcutActionEntry(
        kShortcutActionToggleShowRemoteCursor, 'Show remote cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFollowRemoteCursor, 'Follow remote cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFollowRemoteWindow, 'Follow remote window focus'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleZoomCursor, 'Zoom cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleQualityMonitor, 'Show quality monitor'),
    KeyboardShortcutActionEntry(kShortcutActionToggleMute, 'Mute'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleEnableFileCopyPaste, 'Enable file copy and paste'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleDisableClipboard, 'Disable clipboard'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleLockAfterSessionEnd, 'Lock after session end'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleTrueColor, 'True color (4:4:4)'),
    // Privacy mode at the bottom — mirrors `_DisplayMenu` where it's the
    // last submenu added (line ~1023 of remote_toolbar.dart, after toggles).
    KeyboardShortcutActionSubgroup('Privacy mode', [
      // Reuse toolbar's existing impl-name i18n keys. The handler at
      // runtime matches `privacy_mode_impl_mag_tip` /
      // `privacy_mode_impl_virtual_display_tip` against the peer's
      // advertised impls — same logic the toolbar's `toolbarPrivacyMode`
      // submenu uses.
      KeyboardShortcutActionEntry(
          kShortcutActionPrivacyMode1, 'privacy_mode_impl_mag_tip'),
      KeyboardShortcutActionEntry(
          kShortcutActionPrivacyMode2, 'privacy_mode_impl_virtual_display_tip'),
    ]),
  ]),
  // Keyboard: Keyboard mode subgroup first, then direct items
  // (inputSource → viewMode → showMyCursor → toolbarKeyboardToggles),
  // matching `_KeyboardMenu.menuChildrenGetter`.
  KeyboardShortcutActionGroup('Keyboard', [
    KeyboardShortcutActionSubgroup('Keyboard mode', [
      KeyboardShortcutActionEntry(
          kShortcutActionKeyboardModeLegacy, 'Legacy mode'),
      KeyboardShortcutActionEntry(kShortcutActionKeyboardModeMap, 'Map mode'),
      KeyboardShortcutActionEntry(
          kShortcutActionKeyboardModeTranslate, 'Translate mode'),
    ]),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleInputSource, 'Toggle input source'),
    KeyboardShortcutActionEntry(kShortcutActionToggleViewOnly, 'View Mode'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleShowMyCursor, 'Show my cursor'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleSwapCtrlCmd, 'Swap control-command key'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleRelativeMouseMode, 'Relative mouse mode'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleReverseMouseWheel, 'Reverse mouse wheel'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleSwapLeftRightMouse, 'swap-left-right-mouse'),
  ]),
  KeyboardShortcutActionGroup('Chat', [
    KeyboardShortcutActionEntry(kShortcutActionToggleChat, 'Text chat'),
    KeyboardShortcutActionEntry(kShortcutActionToggleVoiceCall, 'Voice call'),
  ]),
  // "Other" collects single-icon toolbar buttons that have no dropdown
  // (Pin, Close), plus actions with no toolbar entry at all (Fullscreen —
  // driven by callback, not menu; Toggle Toolbar / tab navigation — tab
  // right-click menu, not toolbar). Combined into one group rather than
  // several 1-item groups for cleaner visual hierarchy.
  KeyboardShortcutActionGroup('Other', [
    KeyboardShortcutActionEntry(kShortcutActionPinToolbar, 'Pin Toolbar'),
    KeyboardShortcutActionEntry(
        kShortcutActionToggleFullscreen, 'Toggle fullscreen'),
    KeyboardShortcutActionEntry(kShortcutActionToggleToolbar, 'Toggle toolbar'),
    KeyboardShortcutActionEntry(kShortcutActionCloseTab, 'Close tab'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchTabNext, 'Switch to next tab'),
    KeyboardShortcutActionEntry(
        kShortcutActionSwitchTabPrev, 'Switch to previous tab'),
  ]),
];

/// Walk the (filtered or unfiltered) group tree and yield every
/// [KeyboardShortcutActionEntry], regardless of whether it sits as a direct
/// child of a top-level group or inside a subgroup. Useful for label
/// lookups, ghost-action tests, and any consumer that just wants the flat
/// list of action ids.
Iterable<KeyboardShortcutActionEntry> allActionEntries(
  Iterable<KeyboardShortcutActionGroup> groups,
) sync* {
  for (final group in groups) {
    for (final child in group.children) {
      switch (child) {
        case KeyboardShortcutActionEntry():
          yield child;
        case KeyboardShortcutActionSubgroup():
          yield* child.entries;
      }
    }
  }
}

/// Return [kKeyboardShortcutActionGroups] with actions that aren't supported
/// on the current platform stripped out. Subgroups whose every entry was
/// filtered are dropped; top-level groups whose every child (direct entry
/// or subgroup) was dropped are themselves dropped.
///
/// Mirrors the capability flags used by [filterDefaultBindingsForPlatform]
/// so the configuration UI shows only what the matcher can actually
/// dispatch on this platform.
///
/// Note: callers should still walk the unfiltered
/// [kKeyboardShortcutActionGroups] for label lookups (e.g. conflict
/// warnings about a stale cross-platform binding), so an action bound on
/// desktop and carried over to mobile still has a human-readable name in
/// dialogs.
List<KeyboardShortcutActionGroup> filterKeyboardShortcutActionGroupsForPlatform(
  ShortcutPlatformCapabilities cap,
) {
  bool allowed(String id) {
    if (!cap.includeFullscreenShortcut &&
        id == kShortcutActionToggleFullscreen) {
      return false;
    }
    if (!cap.includeScreenshotShortcut && id == kShortcutActionScreenshot) {
      return false;
    }
    if (!cap.includeTabShortcuts && isSwitchTabShortcutAction(id)) return false;
    if (!cap.includeToolbarShortcut && id == kShortcutActionToggleToolbar) {
      return false;
    }
    if (!cap.includeCloseTabShortcut && id == kShortcutActionCloseTab) {
      return false;
    }
    if (!cap.includeSwitchSidesShortcut && id == kShortcutActionSwitchSides) {
      return false;
    }
    if (!cap.includeRecordingShortcut && id == kShortcutActionToggleRecording) {
      return false;
    }
    if (!cap.includeResetCanvasShortcut && id == kShortcutActionResetCanvas) {
      return false;
    }
    if (!cap.includePinToolbarShortcut && id == kShortcutActionPinToolbar) {
      return false;
    }
    if (!cap.includeViewModeShortcut &&
        (id == kShortcutActionViewModeOriginal ||
            id == kShortcutActionViewModeAdaptive ||
            id == kShortcutActionViewModeCustom)) {
      return false;
    }
    if (!cap.includeInputSourceShortcut &&
        id == kShortcutActionToggleInputSource) {
      return false;
    }
    if (!cap.includeVoiceCallShortcut && id == kShortcutActionToggleVoiceCall) {
      return false;
    }
    return true;
  }

  final out = <KeyboardShortcutActionGroup>[];
  for (final group in kKeyboardShortcutActionGroups) {
    final filteredChildren = <KeyboardShortcutActionGroupChild>[];
    for (final child in group.children) {
      switch (child) {
        case KeyboardShortcutActionEntry():
          if (allowed(child.id)) filteredChildren.add(child);
        case KeyboardShortcutActionSubgroup():
          final entries =
              child.entries.where((e) => allowed(e.id)).toList();
          if (entries.isNotEmpty) {
            filteredChildren.add(
                KeyboardShortcutActionSubgroup(child.titleKey, entries));
          }
      }
    }
    if (filteredChildren.isNotEmpty) {
      out.add(KeyboardShortcutActionGroup(group.titleKey, filteredChildren));
    }
  }
  return out;
}
