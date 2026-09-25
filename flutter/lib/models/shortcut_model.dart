import 'dart:async';
import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../common.dart';
import '../common/shared_state.dart' show PrivacyModeState;
import '../common/widgets/dialog.dart'
    show desktopTryShowTabAuditDialogCloseCancelled;
import '../common/widgets/keyboard_shortcuts/shortcut_utils.dart';
import '../common/widgets/toolbar.dart'
    show allowDisplaySwitchInPrivacyMode, showVirtualDisplayMenu;
import '../consts.dart';
import '../desktop/widgets/remote_toolbar.dart' show ToolbarState;
import 'chat_model.dart' show VoiceCallStatus;
import '../desktop/widgets/tabbar_widget.dart' show DesktopTabController;
import '../models/model.dart';
import '../models/platform_model.dart';
import '../models/state_model.dart';

typedef ShortcutCallback = FutureOr<void> Function();

/// Per-session shortcut dispatcher. Attached to FFI when a session is created.
///
/// The Rust matcher (src/keyboard/shortcuts.rs) emits `shortcut_triggered`
/// session events containing the matched `action` id. The session event
/// listener in [FfiModel.startEventListener] forwards those to this model
/// via [onTriggered], which runs whatever callback the toolbar / menu
/// builders previously registered for that action id.
class ShortcutModel {
  static WeakReference<ShortcutModel>? _activeWebModel;

  final WeakReference<FFI> parent;
  final Map<String, ShortcutCallback> _callbacks = {};
  final Map<String, ShortcutCallback> _refreshers = {};

  ShortcutModel(this.parent);

  /// Called by toolbar / menu builders to register what to do when the
  /// matched shortcut fires.
  void register(String actionId, ShortcutCallback callback) {
    if (parent.target?.closed != false) return;
    _callbacks[actionId] = callback;
    _activeWebModel = WeakReference(this);
  }

  void unregister(String actionId) {
    _callbacks.remove(actionId);
  }

  void registerRefresher(List<String> actionIds, ShortcutCallback refresh) {
    if (parent.target?.closed != false) return;
    for (final actionId in actionIds) {
      _refreshers[actionId] = refresh;
    }
  }

  void clear() {
    _callbacks.clear();
    _refreshers.clear();
    if (identical(_activeWebModel?.target, this)) {
      _activeWebModel = null;
    }
  }

  static void onWebTriggered(String actionId) {
    final model = _activeWebModel?.target;
    if (model != null) {
      model.onTriggered(actionId);
    } else {
      debugPrint('shortcut_triggered: no active web shortcut model');
    }
  }

  /// Called by the session event listener when a `shortcut_triggered` event
  /// arrives for this session.
  ///
  /// The matcher has already consumed the chord. With no handler the action
  /// is unavailable in this session (e.g. screenshot on a peer without
  /// support), and the chord is a deliberate no-op: a bound chord always
  /// belongs to RustDesk and is never forwarded to the remote.
  void onTriggered(String actionId) {
    if (parent.target?.closed != false) return;
    unawaited(Future.sync(() async {
      final refresh = _refreshers[actionId];
      if (refresh != null) {
        await refresh();
        if (!identical(_refreshers[actionId], refresh)) return;
      }
      if (parent.target?.closed != false) return;
      final cb = _callbacks[actionId];
      if (cb != null) {
        await cb();
      } else {
        debugPrint('shortcut_triggered: no handler for $actionId');
      }
    }).catchError((e, st) {
      debugPrint('shortcut_triggered: handler failed for $actionId: $e\n$st');
    }));
  }

  static String _configRaw = '';
  static ShortcutConfig _config = ShortcutConfig.parse('');

  /// The stored config, parsed once per distinct stored value. The Flutter
  /// matcher reads it on every key press, so a key press costs one config
  /// read and no JSON parsing while the config is unchanged.
  static ShortcutConfig config() {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw != _configRaw) {
      _config = ShortcutConfig.parse(raw);
      _configRaw = raw;
    }
    return _config;
  }

  /// Read the bindings JSON from LocalConfig. Returns a copy the caller may
  /// edit.
  static List<Map<String, dynamic>> readBindings() => [
        for (final binding in config().bindings)
          Map<String, dynamic>.of(binding)
      ];

  static bool isEnabled() => config().enabled;

  static bool isPassThrough() => config().passThrough;

  /// Persistent companion to [isEnabled]: when on, the matchers return early
  /// and every keystroke flows through to the remote (i.e. all bindings are
  /// suspended). Stored in the same JSON blob so a single reload refreshes
  /// both flags on every active matcher.
  static Future<void> setPassThrough(bool v) async {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    Map<String, dynamic> json = {};
    if (raw.isNotEmpty) {
      try {
        json = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        json = {};
      }
    }
    json['pass_through'] = v;
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    bind.mainReloadKeyboardShortcuts();
  }

  /// Flip the master `enabled` flag and persist. On the first enable we seed
  /// the default bindings so common combos work out of the box; otherwise we
  /// preserve whatever the user already has, an empty list included. Refreshes the matcher cache so
  /// the change takes effect immediately (Rust on native, JS via the bridge
  /// on Web).
  static Future<void> setEnabled(bool v) async {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    Map<String, dynamic> json = {};
    if (raw.isNotEmpty) {
      try {
        json = jsonDecode(raw) as Map<String, dynamic>;
      } catch (_) {
        json = {};
      }
    }
    json['enabled'] = v;
    if (v && shouldSeedDefaultShortcutBindings(json)) {
      json['bindings'] = filterDefaultBindingsForPlatform(
        jsonDecode(bind.mainGetDefaultKeyboardShortcuts()) as List,
        currentPlatformCapabilities(),
      );
    } else if (json.containsKey('bindings')) {
      json['bindings'] = shortcutBindingMapsFrom(json['bindings']);
    }
    await bind.mainSetLocalOption(
        key: kShortcutLocalConfigKey, value: jsonEncode(json));
    bind.mainReloadKeyboardShortcuts();
  }

  /// Single source of truth for the per-platform "is this shortcut applicable"
  /// decisions. Both [setEnabled]'s default-seeding pass and the configuration
  /// page's reset / list-rendering paths read from here, so the seed list and
  /// the visible action list can never disagree on which platform a given
  /// action belongs to.
  ///
  /// Capability rationale:
  ///   * Fullscreen / Toolbar / Pin / View Mode: rendered wherever the
  ///     desktop layout applies (native desktop + Web). Native mobile is
  ///     permanently full-screen and doesn't have a desktop-style toolbar.
  ///   * Screenshot / Switch Sides: native desktop only. The Web bridge
  ///     throws UnimplementedError for `sessionTakeScreenshot`; mobile
  ///     toolbars don't surface either action.
  ///   * Tab navigation / Close Tab: only native desktop ships
  ///     `DesktopTabController`; Web's `RemotePage` is invoked without one.
  ///   * Recording: native desktop has the `_RecordMenu` widget +
  ///     `registerSessionShortcutActions` registration; native Android has
  ///     the `toolbarControls` entry; iOS short-circuits inside
  ///     `recordingModel.toggle()`; Web has no implementation.
  ///   * Reset Canvas: only the mobile toolbar builds the menu entry
  ///     (`isDefaultConn && isMobile` in `toolbarControls`).
  ///   * Voice Call: Web bridge throws `UnimplementedError` for both
  ///     `sessionRequestVoiceCall` and `sessionCloseVoiceCall`.
  static ShortcutPlatformCapabilities currentPlatformCapabilities() {
    final desktopLayout = isDesktop || isWeb;
    return ShortcutPlatformCapabilities(
      includeFullscreenShortcut: desktopLayout,
      includeScreenshotShortcut: isDesktop,
      includeTabShortcuts: isDesktop,
      includeToolbarShortcut: desktopLayout,
      includeCloseTabShortcut: isDesktop,
      includeSwitchSidesShortcut: isDesktop,
      includeRecordingShortcut: !isWeb && !isIOS,
      includeResetCanvasShortcut: isMobile,
      includePinToolbarShortcut: desktopLayout,
      includeViewModeShortcut: desktopLayout,
      includeVoiceCallShortcut: !isWeb,
    );
  }
}

/// One parsed view of the `keyboard-shortcuts` LocalConfig value. Malformed
/// or missing input parses as disabled with no bindings.
class ShortcutConfig {
  final bool enabled;
  final bool passThrough;
  final List<Map<String, dynamic>> bindings;

  const ShortcutConfig._(this.enabled, this.passThrough, this.bindings);

  static ShortcutConfig parse(String raw) {
    if (raw.isEmpty) return const ShortcutConfig._(false, false, []);
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      return ShortcutConfig._(
        parsed['enabled'] == true,
        parsed['pass_through'] == true,
        List.unmodifiable(shortcutBindingMapsFrom(parsed['bindings'])),
      );
    } catch (_) {
      return const ShortcutConfig._(false, false, []);
    }
  }
}

/// Register the default-bound shortcut actions that aren't already wired by
/// `toolbarControls(...)` (which handles things like Ctrl+Alt+Shift+Del and the
/// screenshot action). Called once per session from the desktop / mobile
/// remote page, after the toolbar registrations have run.
///
/// We register unconditionally — even when shortcuts are master-disabled —
/// because the matcher (Rust + JS) gates dispatch via the `enabled` flag,
/// so registered closures are functionally invisible until the user flips
/// shortcuts on. This keeps the wiring simple (no rebind callbacks across
/// sessions) and lets the user toggle shortcuts mid-session without
/// reconnecting.
///
/// [tabController] is the desktop window's tab controller; `null` on mobile /
/// web (where tab-switch shortcuts don't apply).
///
/// Each callback below is a no-op when the underlying state required to
/// service the action isn't available (e.g. only one display, only one tab).
void registerSessionShortcutActions(
  FFI ffi, {
  DesktopTabController? tabController,
  ToolbarState? toolbarState,
}) {
  final sessionId = ffi.sessionId;

  // Note on disposal: every closure registered below captures `ffi` via
  // closure environment, so the FFI object stays alive for the duration of
  // the closure's execution — even across awaits, even if the session is
  // closed mid-execution. We therefore don't add per-closure liveness
  // guards: a `WeakReference<FFI>` check would never go null while the
  // closure is on the call stack, and the underlying `bind.session*` /
  // model setters tolerate stale-session calls (they no-op on torn-down
  // sessions). ShortcutModel.onTriggered's existing entry guard
  // (`_callbacks` lookup returning null after disposal) is the actual
  // liveness gate.

  // Toggle Fullscreen — available wherever the desktop layout renders
  // (native desktop + every Web browser, since Web uses the desktop
  // RemotePage). `stateGlobal.setFullscreen` handles native window vs.
  // browser fullscreen. Native mobile is permanently full-screen, so the
  // action is intentionally not registered there.
  if (isDesktop || isWeb) {
    ffi.shortcutModel.register(kShortcutActionToggleFullscreen, () {
      stateGlobal.setFullscreen(!stateGlobal.fullscreen.value);
    });
  }

  // Toggle Recording — desktop only here. Mobile already wires this through
  // `toolbarControls` (which adds a recording entry on `!(isDesktop||isWeb)`),
  // but the desktop toolbar uses a separate `_RecordMenu` widget that has no
  // `actionId`. Without this explicit registration a desktop user could bind
  // Toggle Recording in settings and the press would have no handler.
  // `recordingModel.toggle()` itself short-circuits on iOS and on sessions
  // without recording permission.
  if (isDesktop) {
    ffi.shortcutModel.register(kShortcutActionToggleRecording, () {
      ffi.recordingModel.toggle();
    });
  }

  // Switch Display Next / Prev — requires the peer to have at least 2
  // displays. From the "All displays" merged view, Next jumps to display 0
  // and Prev to the last display, so the user can always escape the merged
  // view via these shortcuts.
  void switchDisplayBy(int delta) {
    final pi = ffi.ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (privacyMode.isNotEmpty &&
        !allowDisplaySwitchInPrivacyMode(pi, privacyMode)) {
      return;
    }
    final count = pi.displays.length;
    if (count <= 1) return;
    final current = pi.currentDisplay;
    final int next;
    if (current == kAllDisplayValue) {
      next = delta > 0 ? 0 : count - 1;
    } else {
      next = ((current + delta) % count + count) % count;
    }
    bind.sessionSwitchDisplay(
      isDesktop: isDesktop,
      sessionId: sessionId,
      value: Int32List.fromList([next]),
    );
    if (pi.isSupportMultiUiSession) {
      // On multi-ui-session peers no switch-display message is sent back, so
      // update the local state directly (mirrors `model.dart` handling).
      ffi.ffiModel.switchToNewDisplay(next, sessionId, ffi.id);
    }
  }

  ffi.shortcutModel.register(kShortcutActionSwitchDisplayNext, () {
    switchDisplayBy(1);
  });
  ffi.shortcutModel.register(kShortcutActionSwitchDisplayPrev, () {
    switchDisplayBy(-1);
  });

  // Switch to all-monitors view — mirrors the toolbar Monitor menu's
  // "all monitors" button (only built when peer has >1 display). Not a
  // toggle: the toolbar button just sets the merged view; another action
  // (Switch to next/previous display, or another monitor button) takes
  // you back to a single display.
  //
  // Use `openMonitorInTheSameTab(kAllDisplayValue, ...)` rather than calling
  // `sessionSwitchDisplay` with `[kAllDisplayValue]` directly — the toolbar
  // path treats `kAllDisplayValue` as a UI sentinel and expands it to the
  // real display index list (`[0, 1, ...]`) before sending, then updates
  // local FfiModel state. Sending `[-1]` raw produces a wire value the
  // remote can't act on and skips the local state update, so the merged
  // view never engages.
  ffi.shortcutModel.register(kShortcutActionSwitchDisplayAll, () {
    final pi = ffi.ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (privacyMode.isNotEmpty &&
        !allowDisplaySwitchInPrivacyMode(pi, privacyMode)) {
      return;
    }
    if (pi.displays.length <= 1) return;
    if (pi.currentDisplay == kAllDisplayValue) return;
    openMonitorInTheSameTab(kAllDisplayValue, ffi, pi);
  });

  // Switch tab next / prev — desktop only. The remote-screen tabs live in
  // the window-scoped DesktopTabController, not on the FFI itself, so we
  // need the controller from the page that owns this session. We
  // intentionally don't expose positional ("Switch to tab N") shortcuts:
  // counting tabs in a long list is impractical, and AnyDesk / Chrome
  // standard practice is to favour next/prev navigation.
  if (tabController != null) {
    void switchTabBy(int delta) {
      final tabs = tabController.state.value.tabs;
      if (tabs.length <= 1) return;
      final cur = tabs.indexWhere((t) => t.key == ffi.id);
      if (cur < 0) return;
      final next = (cur + delta + tabs.length) % tabs.length;
      tabController.jumpTo(next);
    }

    ffi.shortcutModel
        .register(kShortcutActionSwitchTabNext, () => switchTabBy(1));
    ffi.shortcutModel
        .register(kShortcutActionSwitchTabPrev, () => switchTabBy(-1));

    // Close Tab — desktop only. Mirrors the tab right-click "Close" entry,
    // including the audit-log confirmation dialog so a shortcut close goes
    // through the same path as a menu close.
    ffi.shortcutModel.register(kShortcutActionCloseTab, () async {
      if (await desktopTryShowTabAuditDialogCloseCancelled(
        id: ffi.id,
        tabController: tabController,
      )) {
        return;
      }
      tabController.closeBy(ffi.id);
    });
  }

  // Toggle Toolbar — desktop only. ToolbarState is window/session-scoped,
  // owned by the RemotePage that hosts this session.
  if (toolbarState != null) {
    ffi.shortcutModel.register(kShortcutActionToggleToolbar, () {
      toolbarState.switchHide(sessionId);
    });
    ffi.shortcutModel.register(kShortcutActionPinToolbar, () {
      toolbarState.switchPin();
    });
  }

  // Toggle Chat overlay (open/close the chat panel for this session).
  // _ChatMenu is a standalone toolbar icon — not part of any toolbar
  // helper that returns a TToggleMenu list — so its handler is wired
  // here rather than picked up by helper auto-register.
  ffi.shortcutModel.register(kShortcutActionToggleChat, () {
    ffi.chatModel.toggleChatOverlay();
  });

  // Toggle Voice Call — start when idle, hang up when active. Mirrors the
  // toolbar's `_VoiceCallMenu` state-driven button. Web bridge throws
  // UnimplementedError on both sessionRequestVoiceCall and
  // sessionCloseVoiceCall, so we don't register on web.
  if (!isWeb) {
    ffi.shortcutModel.register(kShortcutActionToggleVoiceCall, () {
      final status = ffi.chatModel.voiceCallStatus.value;
      if (status == VoiceCallStatus.connected ||
          status == VoiceCallStatus.waitingForResponse) {
        bind.sessionCloseVoiceCall(sessionId: sessionId);
      } else {
        bind.sessionRequestVoiceCall(sessionId: sessionId);
      }
    });
  }

  // ── Inline _KeyboardMenu items + actions with no toolbar TToggleMenu/TRadioMenu ─
  // The toolbar's TToggleMenu / TRadioMenu helpers (toolbarDisplayToggle,
  // toolbarCursor, toolbarKeyboardToggles, toolbarCodec, toolbarPrivacyMode,
  // toolbarViewStyle, toolbarImageQuality) auto-register their tagged entries
  // from the bottom of each helper. The handlers below cover what those
  // helpers DON'T own:
  //   * Show my cursor / Keyboard mode (Map/Translate/Legacy) / View Only
  //     (desktop) — built as widgets directly in `_KeyboardMenu`, not as
  //     TToggleMenu lists. (Mobile View Only IS in toolbarDisplayToggle and
  //     auto-registers; the desktop session-start handler below registers
  //     first and the helper's auto-register on mobile takes over after its
  //     unawaited future resolves.)
  //   * Plug out all virtual displays — built in `getVirtualDisplayMenuChildren`
  //     as a MenuButton, not a TToggleMenu.
  //   * Toggle Input Source — cycle action; the toolbar exposes per-source
  //     radios but no single "cycle to next source" entry.

  // Show my cursor — toolbar (`_KeyboardMenu.showMyCursor`) pushes the new
  // value into FfiModel.setShowMyCursor and auto-enables view-only when the
  // toggle goes on, so the user can never control the remote with their own
  // cursor visible.
  ffi.shortcutModel.register(kShortcutActionToggleShowMyCursor, () async {
    await bind.sessionToggleOption(
        sessionId: sessionId, value: kOptionToggleShowMyCursor);
    final showMyCursor = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleShowMyCursor) ??
        false;
    ffi.ffiModel.setShowMyCursor(showMyCursor);
    if (showMyCursor && !ffi.ffiModel.viewOnly) {
      await bind.sessionToggleOption(
          sessionId: sessionId, value: kOptionToggleViewOnly);
      final viewOnly = await bind.sessionGetToggleOption(
              sessionId: sessionId, arg: kOptionToggleViewOnly) ??
          false;
      ffi.ffiModel.setViewOnly(ffi.id, viewOnly);
    }
  });

  // Keyboard mode (Map / Translate / Legacy). Mirrors the radio buttons in
  // `_KeyboardMenu.keyboardMode()` (built as RdoMenuButton, not TRadioMenu).
  void registerKeyboardMode(String actionId, String mode) {
    ffi.shortcutModel.register(actionId, () async {
      if (!ffi.ffiModel.keyboard ||
          ffi.ffiModel.viewOnly ||
          !bind.sessionIsKeyboardModeSupported(
              sessionId: sessionId, mode: mode) ||
          (ffi.ffiModel.pi.isWayland && mode != kKeyMapMode)) {
        return;
      }
      if (isInputSourceFlutter && isDesktop) {
        final modeOnly = bind.sessionIsKeyboardModeSupported(
                sessionId: sessionId, mode: kKeyMapMode)
            ? kKeyMapMode
            : kKeyLegacyMode;
        if (mode != modeOnly) return;
      }
      await bind.sessionSetKeyboardMode(sessionId: sessionId, value: mode);
      await ffi.inputModel.updateKeyboardMode();
    });
  }

  registerKeyboardMode(kShortcutActionKeyboardModeMap, kKeyMapMode);
  registerKeyboardMode(kShortcutActionKeyboardModeTranslate, kKeyTranslateMode);
  registerKeyboardMode(kShortcutActionKeyboardModeLegacy, kKeyLegacyMode);

  // Plug out all virtual displays (Windows + IDD only). Mirrors the toolbar's
  // "Plug out all" button — present in both IDD modes (RustDesk + Amyuni),
  // built as a MenuButton inside `getVirtualDisplayMenuChildren`.
  ffi.shortcutModel.register(kShortcutActionPlugOutAllVirtualDisplays, () {
    if (ffi.connType != ConnType.defaultConn ||
        !showVirtualDisplayMenu(ffi) ||
        PrivacyModeState.find(ffi.id).value.isNotEmpty ||
        (ffi.ffiModel.pi.isAmyuniIdd &&
            ffi.ffiModel.pi.amyuniVirtualDisplayCount == 0)) {
      return;
    }
    bind.sessionToggleVirtualDisplay(
      sessionId: sessionId,
      index: kAllVirtualDisplay,
      on: false,
    );
  });

  // Privacy mode 1 / 2 — fallback handlers for the single-impl and null-impls
  // branches of `toolbarPrivacyMode`. The multi-impl branch tags each entry
  // with the matching actionId and `_registerToggleMenuShortcuts` overrides
  // these closures with the toolbar's own onChanged. But when the peer only
  // advertises a single impl (older Linux peers, certain platform configs)
  // toolbarPrivacyMode returns a `getDefaultMenu` entry without an actionId,
  // so the auto-register pass skips it — these fallbacks are what actually
  // wire the shortcut in that case.
  String? findPrivacyImpl(String nameKey) {
    final impls = ffi.ffiModel.pi
            .platformAdditions[kPlatformAdditionsSupportedPrivacyModeImpl]
        as List<dynamic>?;
    if (impls == null) return null;
    for (final e in impls) {
      if (e is List && e.length >= 2 && e[1] == nameKey) return e[0] as String;
    }
    return null;
  }

  bool checkPrivacyModeAllowed(String implKey) {
    final ffiModel = ffi.ffiModel;
    final pi = ffiModel.pi;
    final privacyMode = PrivacyModeState.find(ffi.id).value;
    if (ffi.connType != ConnType.defaultConn ||
        ffiModel.viewOnly ||
        (privacyMode.isEmpty &&
            (!pi.features.privacyMode || !ffiModel.keyboard))) {
      return false;
    }
    if (privacyMode == implKey) return true;
    if (ffiModel.permissions['privacy_mode'] == false) return false;
    if (allowDisplaySwitchInPrivacyMode(pi, implKey) ||
        (pi.currentDisplay == 0 &&
            !bind.sessionIsMultiUiSession(sessionId: sessionId))) {
      return true;
    }
    msgBox(sessionId, 'custom-nook-nocancel-hasclose', 'info',
        'Please switch to Display 1 first', '', ffi.dialogManager);
    return false;
  }

  // Match the multi-impl branch of `toolbarPrivacyMode`: turn this impl on iff
  // the active impl isn't already this one. Comparing `.value == implKey`
  // (rather than `.value.isEmpty`) means pressing the mode-1 shortcut while
  // mode 2 is on correctly turns mode 1 ON, instead of misreading the
  // "any-mode-active" state as "this-mode-active" and toggling OFF.
  ffi.shortcutModel.register(kShortcutActionPrivacyMode1, () {
    final implKey = findPrivacyImpl('privacy_mode_impl_mag_tip');
    if (implKey == null) return;
    if (!checkPrivacyModeAllowed(implKey)) return;
    bind.sessionTogglePrivacyMode(
      sessionId: sessionId,
      implKey: implKey,
      on: PrivacyModeState.find(ffi.id).value != implKey,
    );
  });
  ffi.shortcutModel.register(kShortcutActionPrivacyMode2, () {
    final implKey = findPrivacyImpl('privacy_mode_impl_virtual_display_tip');
    if (implKey == null) return;
    if (!checkPrivacyModeAllowed(implKey)) return;
    bind.sessionTogglePrivacyMode(
      sessionId: sessionId,
      implKey: implKey,
      on: PrivacyModeState.find(ffi.id).value != implKey,
    );
  });

  // View Only — desktop toolbar exposes this inline in `_KeyboardMenu.viewMode`
  // (mobile is in toolbarDisplayToggle and goes through helper auto-register).
  // Mirrors the desktop callback: toggle + sync FfiModel.viewOnly +
  // FfiModel.showMyCursor (the toolbar keeps these in step).
  ffi.shortcutModel.register(kShortcutActionToggleViewOnly, () async {
    await bind.sessionToggleOption(
        sessionId: sessionId, value: kOptionToggleViewOnly);
    final viewOnly = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleViewOnly) ??
        false;
    ffi.ffiModel.setViewOnly(ffi.id, viewOnly);
    final showMyCursor = await bind.sessionGetToggleOption(
            sessionId: sessionId, arg: kOptionToggleShowMyCursor) ??
        false;
    ffi.ffiModel.setShowMyCursor(showMyCursor);
  });

  // Toggle Reverse mouse wheel — read current 'Y'/'N' (falling back to user
  // default), flip, write back.
  ffi.shortcutModel.register(kShortcutActionToggleReverseMouseWheel, () async {
    if (!ffi.ffiModel.keyboard || ffi.ffiModel.viewOnly) return;
    var cur = bind.sessionGetReverseMouseWheelSync(sessionId: sessionId) ?? '';
    if (cur == '') {
      cur = bind.mainGetUserDefaultOption(key: kKeyReverseMouseWheel);
    }
    final next = cur == 'Y' ? 'N' : 'Y';
    await bind.sessionSetReverseMouseWheel(sessionId: sessionId, value: next);
  });

  // Toggle Relative mouse mode (gaming mode). Desktop only.
  if (isDesktop && !isWeb) {
    ffi.shortcutModel.register(kShortcutActionToggleRelativeMouseMode, () {
      ffi.inputModel.toggleRelativeMouseMode();
    });
  }
}
