import 'dart:convert';

import 'package:flutter/foundation.dart';

import '../common.dart';
import '../consts.dart';
import '../desktop/widgets/tabbar_widget.dart' show DesktopTabController;
import '../models/model.dart';
import '../models/platform_model.dart';
import '../models/state_model.dart';

/// Per-session shortcut dispatcher. Attached to FFI when a session is created.
///
/// The Rust matcher (src/keyboard/shortcuts.rs) emits `shortcut_triggered`
/// session events containing the matched `action` id. The session event
/// listener in [FfiModel.startEventListener] forwards those to this model
/// via [onTriggered], which runs whatever callback the toolbar / menu
/// builders previously registered for that action id.
class ShortcutModel {
  final WeakReference<FFI> parent;
  final Map<String, VoidCallback> _callbacks = {};

  ShortcutModel(this.parent);

  /// Called by toolbar / menu builders to register what to do when the
  /// matched shortcut fires.
  void register(String actionId, VoidCallback callback) {
    _callbacks[actionId] = callback;
  }

  void unregister(String actionId) {
    _callbacks.remove(actionId);
  }

  /// Called by the session event listener when a `shortcut_triggered` event
  /// arrives for this session.
  void onTriggered(String actionId) {
    final cb = _callbacks[actionId];
    if (cb != null) {
      cb();
    } else {
      debugPrint('shortcut_triggered: no handler for $actionId');
    }
  }

  /// Read the bindings JSON from LocalConfig.
  static List<Map<String, dynamic>> readBindings() {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return [];
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      final list = (parsed['bindings'] as List?) ?? [];
      return list.cast<Map<String, dynamic>>();
    } catch (_) {
      return [];
    }
  }

  static bool isEnabled() {
    final raw = bind.mainGetLocalOption(key: kShortcutLocalConfigKey);
    if (raw.isEmpty) return false;
    try {
      final parsed = jsonDecode(raw) as Map<String, dynamic>;
      return parsed['enabled'] == true;
    } catch (_) {
      return false;
    }
  }
}

/// Register the default-bound shortcut actions that aren't already wired by
/// `toolbarControls(...)` (which handles things like Ctrl+Alt+Shift+Del and the
/// screenshot action). Called once per session from the desktop / mobile
/// remote page, after the toolbar registrations have run.
///
/// [tabController] is the desktop window's tab controller; `null` on mobile /
/// web (where tab-switch shortcuts don't apply).
///
/// Each callback below is a no-op when the underlying state required to
/// service the action isn't available (e.g. only one display, only one tab).
void registerSessionShortcutActions(
  FFI ffi, {
  DesktopTabController? tabController,
}) {
  final sessionId = ffi.sessionId;

  // Toggle Fullscreen — desktop & web-desktop only. `stateGlobal.setFullscreen`
  // handles native window vs. browser fullscreen; on mobile fullscreen is the
  // permanent default, so we leave the action unregistered (becomes a logged
  // no-op if a mobile user binds it).
  if (isDesktop || isWebDesktop) {
    ffi.shortcutModel.register(kShortcutActionToggleFullscreen, () {
      stateGlobal.setFullscreen(!stateGlobal.fullscreen.value);
    });
  }

  // Switch Display Next / Prev — requires the peer to have at least 2
  // displays. No-op when only one display is available or when the user has
  // selected the "All displays" pseudo-display.
  void switchDisplayBy(int delta) {
    final pi = ffi.ffiModel.pi;
    final count = pi.displays.length;
    if (count <= 1) return;
    final current = pi.currentDisplay;
    if (current == kAllDisplayValue) return;
    final next = ((current + delta) % count + count) % count;
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

  // Switch Tab 1..9 — desktop only. The remote-screen tabs live in the
  // window-scoped DesktopTabController, not on the FFI itself, so we need
  // the controller from the page that owns this session. No-op on mobile /
  // web (no controller passed) and when the requested tab index is out of
  // range.
  if (tabController != null) {
    for (var n = 1; n <= 9; n++) {
      final idx = n - 1;
      ffi.shortcutModel.register(kShortcutActionSwitchTab(n), () {
        if (tabController.state.value.tabs.length > idx) {
          tabController.jumpTo(idx);
        }
      });
    }
  }
}
