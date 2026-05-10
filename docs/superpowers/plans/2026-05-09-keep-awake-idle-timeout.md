# Keep-Awake Idle Timeout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a configurable idle timeout to the keep-awake feature — after X seconds of no user interaction during a remote session, release the wakelock and allow the screen to lock; restore the wakelock when the user unlocks back to Tabby.

**Architecture:** A new string option `kOptionKeepAwakeIdleTimeout` stores the chosen threshold (`"never"` | `"30s"` | `"1m"` | `"5m"` | `"15m"` | `"30m"` | `"custom:<seconds>"`). `WakelockManager` gains a static idle-timer helper that `_MobilePageState` in `remote_page.dart` starts/resets on touch and keyboard events, and restores on `AppLifecycleState.resumed`. Settings UI adds a dropdown below the existing toggle.

**Tech Stack:** Flutter/Dart, `wakelock_plus` (already in use), `dart:async` Timer, existing `SettingsTile` / `CustomAlertDialog` patterns from `settings_page.dart`.

---

## File Map

| File | Change |
|---|---|
| `flutter/lib/consts.dart` | Add `kOptionKeepAwakeIdleTimeout` constant |
| `flutter/lib/common.dart` | Add idle-timer helpers to `WakelockManager` |
| `flutter/lib/mobile/pages/remote_page.dart` | Wire idle timer: start/reset on input, restore on resume |
| `flutter/lib/mobile/pages/settings_page.dart` | Add dropdown UI below the keep-awake toggle |
| `flutter/lib/common/widgets/dialog.dart` | Add `changeKeepAwakeCustomTimeout` dialog |

---

### Task 1: Add the config key constant

**Files:**
- Modify: `flutter/lib/consts.dart:205`

- [ ] **Step 1: Add the constant**

Open `flutter/lib/consts.dart`. After line 205 (`kOptionKeepAwakeDuringOutgoingSessions`), add:

```dart
const String kOptionKeepAwakeIdleTimeout = "keep-awake-idle-timeout";
```

- [ ] **Step 2: Verify it compiles**

```bash
cd flutter && flutter analyze lib/consts.dart
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add flutter/lib/consts.dart
git commit -m "feat: add kOptionKeepAwakeIdleTimeout constant"
```

---

### Task 2: Add idle-timer helpers to WakelockManager

**Files:**
- Modify: `flutter/lib/common.dart` — `WakelockManager` class (around line 2712)

The `WakelockManager` class needs two static helpers:
- `parseIdleTimeoutSeconds()` — reads the option and returns `null` (never) or an `int` in seconds
- `startIdleTimer(UniqueKey key, VoidCallback onTimeout)` / `cancelIdleTimer()` — manage a single static `Timer`

- [ ] **Step 1: Add timer state and helpers inside WakelockManager**

Locate `class WakelockManager {` in `flutter/lib/common.dart`. Add the following fields and methods inside the class (after the existing `static bool _enabled = false;` line):

```dart
  static Timer? _idleTimer;

  /// Returns the configured idle timeout in seconds, or null if "never".
  static int? parseIdleTimeoutSeconds() {
    final raw = mainGetLocalOption(kOptionKeepAwakeIdleTimeout);
    switch (raw) {
      case '30s':
        return 30;
      case '1m':
        return 60;
      case '5m':
        return 300;
      case '15m':
        return 900;
      case '30m':
        return 1800;
      default:
        if (raw.startsWith('custom:')) {
          return int.tryParse(raw.substring(7));
        }
        return null; // "never" or empty
    }
  }

  /// Starts (or restarts) the idle timer. Calls [onTimeout] when it fires.
  static void startIdleTimer(VoidCallback onTimeout) {
    _idleTimer?.cancel();
    final seconds = parseIdleTimeoutSeconds();
    if (seconds == null) return;
    _idleTimer = Timer(Duration(seconds: seconds), onTimeout);
  }

  /// Cancels any running idle timer.
  static void cancelIdleTimer() {
    _idleTimer?.cancel();
    _idleTimer = null;
  }
```

- [ ] **Step 2: Add `dart:async` import if not already present**

Check the top of `flutter/lib/common.dart` for `import 'dart:async';`. It is almost certainly already there (the file uses `StreamSubscription`). If missing, add it.

- [ ] **Step 3: Verify it compiles**

```bash
cd flutter && flutter analyze lib/common.dart
```

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add flutter/lib/common.dart
git commit -m "feat: add idle timer helpers to WakelockManager"
```

---

### Task 3: Wire idle timer into mobile remote_page

**Files:**
- Modify: `flutter/lib/mobile/pages/remote_page.dart`

The `_MobilePageState` class needs to:
1. Start the idle timer after `WakelockManager.enable()` in `initState`
2. Cancel it in `dispose`
3. Reset it on any user interaction (touch + keyboard)
4. Restore wakelock + restart timer on `AppLifecycleState.resumed`

- [ ] **Step 1: Add `_resetIdleTimer` method to `_MobilePageState`**

Find `void trySyncClipboard()` in `remote_page.dart` (around line 175). Add this method just before it:

```dart
  void _resetIdleTimer() {
    WakelockManager.startIdleTimer(() {
      WakelockManager.disable(_uniqueKey);
    });
  }
```

- [ ] **Step 2: Start idle timer in initState**

Find the line `WakelockManager.enable(_uniqueKey);` in `initState` (around line 119). Add the call immediately after it:

```dart
    WakelockManager.enable(_uniqueKey);
    _resetIdleTimer();
```

- [ ] **Step 3: Cancel idle timer in dispose**

Find the line `WakelockManager.disable(_uniqueKey);` in `dispose` (around line 158). Add the cancel call immediately before it:

```dart
    WakelockManager.cancelIdleTimer();
    WakelockManager.disable(_uniqueKey);
```

- [ ] **Step 4: Restore wakelock on screen unlock (resumed)**

Find `didChangeAppLifecycleState` (around line 168). Replace it with:

```dart
  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      trySyncClipboard();
      WakelockManager.enable(_uniqueKey);
      _resetIdleTimer();
    }
  }
```

- [ ] **Step 5: Reset timer on touch input**

Find the `RawTouchGestureDetectorRegion` widget in `build` (around line 448). It currently is:

```dart
RawTouchGestureDetectorRegion(
  child: getBodyForMobile(),
  ffi: gFFI,
  onTwoFingerScroll: widget.onTwoFingerScroll,
)
```

Wrap it in a `Listener` that resets the timer on any pointer-down event:

```dart
Listener(
  onPointerDown: (_) => _resetIdleTimer(),
  child: RawTouchGestureDetectorRegion(
    child: getBodyForMobile(),
    ffi: gFFI,
    onTwoFingerScroll: widget.onTwoFingerScroll,
  ),
)
```

- [ ] **Step 6: Reset timer on physical keyboard input**

Find `getRawPointerAndKeyBody` (around line 488). It wraps the child in `RawKeyFocusScope`. The `RawPointerMouseRegion` wrapping it already handles pointer events for physical mouse — for physical keyboard, add a `Focus` `onKeyEvent` reset. Replace the return statement:

```dart
  Widget getRawPointerAndKeyBody(Widget child) {
    final ffiModel = Provider.of<FfiModel>(context);
    return RawPointerMouseRegion(
      cursor: ffiModel.keyboard ? SystemMouseCursors.none : MouseCursor.defer,
      inputModel: inputModel,
      child: gFFI.ffiModel.pi.isSet.isTrue
          ? RawKeyFocusScope(
              focusNode: _physicalFocusNode,
              inputModel: inputModel,
              child: KeyboardListener(
                focusNode: FocusNode(canRequestFocus: false),
                onKeyEvent: (_) => _resetIdleTimer(),
                child: child,
              ))
          : child,
    );
  }
```

- [ ] **Step 7: Verify it compiles**

```bash
cd flutter && flutter analyze lib/mobile/pages/remote_page.dart
```

Expected: no errors.

- [ ] **Step 8: Commit**

```bash
git add flutter/lib/mobile/pages/remote_page.dart
git commit -m "feat: wire keep-awake idle timer into mobile remote session"
```

---

### Task 4: Add custom timeout dialog

**Files:**
- Modify: `flutter/lib/common/widgets/dialog.dart`

Pattern mirrors `changeAutoDisconnectTimeout` at line 335. This dialog accepts a number of **minutes** and returns a `custom:<seconds>` string.

- [ ] **Step 1: Add the dialog function**

Open `flutter/lib/common/widgets/dialog.dart`. After the closing `}` of `changeAutoDisconnectTimeout`, add:

```dart
Future<String?> changeKeepAwakeCustomTimeout(String current) async {
  // current is either empty or "custom:<seconds>"
  final initialMinutes = current.startsWith('custom:')
      ? ((int.tryParse(current.substring(7)) ?? 0) ~/ 60).toString()
      : '';
  final controller = TextEditingController(text: initialMinutes);
  String? result;
  await gFFI.dialogManager.show((setState, close, context) {
    return CustomAlertDialog(
      title: Text(translate("Timeout in minutes")),
      content: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 8.0),
          Row(
            children: [
              Expanded(
                child: TextField(
                  maxLines: null,
                  keyboardType: TextInputType.number,
                  decoration: InputDecoration(
                    hintText: '30',
                    isCollapsed: true,
                    suffix: IconButton(
                      padding: EdgeInsets.zero,
                      icon: const Icon(Icons.clear, size: 16),
                      onPressed: () => controller.clear(),
                    ),
                  ),
                  inputFormatters: [
                    FilteringTextInputFormatter.allow(RegExp(r'^\d{1,4}$')),
                  ],
                  controller: controller,
                  autofocus: true,
                ).workaroundFreezeLinuxMint(),
              ),
            ],
          ),
        ],
      ),
      actions: [
        dialogButton("Cancel", onPressed: close, isOutline: true),
        dialogButton("OK", onPressed: () {
          final mins = int.tryParse(controller.text);
          if (mins != null && mins > 0) {
            result = 'custom:${mins * 60}';
          }
          close();
        }),
      ],
      onCancel: close,
    );
  });
  return result;
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cd flutter && flutter analyze lib/common/widgets/dialog.dart
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add flutter/lib/common/widgets/dialog.dart
git commit -m "feat: add changeKeepAwakeCustomTimeout dialog"
```

---

### Task 5: Add dropdown UI to Settings

**Files:**
- Modify: `flutter/lib/mobile/pages/settings_page.dart`

Add a new state variable `_keepAwakeIdleTimeout`, initialize it from the option, and render a dropdown row below the existing keep-awake toggle, visible only when `_preventSleepWhileConnected` is true.

The dropdown options are:
- `"never"` → "Never (keep screen on)"
- `"30s"` → "After 30 seconds"
- `"1m"` → "After 1 minute"
- `"5m"` → "After 5 minutes"
- `"15m"` → "After 15 minutes"
- `"30m"` → "After 30 minutes"
- `"custom:..."` → "Custom..." (opens dialog, stored as `custom:<seconds>`)

- [ ] **Step 1: Add state variable**

Find `var _preventSleepWhileConnected = true;` (around line 105). Add below it:

```dart
  var _keepAwakeIdleTimeout = 'never';
```

- [ ] **Step 2: Initialize from storage in constructor**

Find `_preventSleepWhileConnected = mainGetLocalBoolOptionSync(kOptionKeepAwakeDuringOutgoingSessions);` (around line 147). Add below it:

```dart
    _keepAwakeIdleTimeout =
        mainGetLocalOption(kOptionKeepAwakeIdleTimeout).let((v) =>
            v.isEmpty ? 'never' : v);
```

Note: `mainGetLocalOption` is synchronous here (like the other options in this constructor). If `.let` is not available in the codebase, use a local variable instead:

```dart
    final _rawIdleTimeout = mainGetLocalOption(kOptionKeepAwakeIdleTimeout);
    _keepAwakeIdleTimeout = _rawIdleTimeout.isEmpty ? 'never' : _rawIdleTimeout;
```

- [ ] **Step 3: Add the dropdown tile after the keep-awake toggle**

Find the closing `),` of the keep-awake `SettingsTile.switchTile` (the one ending around line 863). Immediately after it (still inside the `tiles: [...]` list), add:

```dart
          if (!incomingOnly && _preventSleepWhileConnected)
            SettingsTile(
              title: Text(translate('Lock screen after idle')),
              trailing: _buildIdleTimeoutDropdown(),
            ),
```

- [ ] **Step 4: Add `_buildIdleTimeoutDropdown` method to `_SettingsState`**

Find a convenient place in `_SettingsState` (e.g. near the bottom, before the closing `}`). Add:

```dart
  static const _idleTimeoutOptions = [
    ('never', 'Never (keep screen on)'),
    ('30s', 'After 30 seconds'),
    ('1m', 'After 1 minute'),
    ('5m', 'After 5 minutes'),
    ('15m', 'After 15 minutes'),
    ('30m', 'After 30 minutes'),
    ('custom', 'Custom...'),
  ];

  Widget _buildIdleTimeoutDropdown() {
    final isCustom = _keepAwakeIdleTimeout.startsWith('custom:');
    final dropdownValue = isCustom ? 'custom' : _keepAwakeIdleTimeout;
    return DropdownButton<String>(
      value: _idleTimeoutOptions.any((o) => o.$1 == dropdownValue)
          ? dropdownValue
          : 'never',
      underline: const SizedBox(),
      items: _idleTimeoutOptions
          .map((o) => DropdownMenuItem(value: o.$1, child: Text(translate(o.$2))))
          .toList(),
      onChanged: (v) async {
        if (v == null) return;
        if (v == 'custom') {
          final result =
              await changeKeepAwakeCustomTimeout(_keepAwakeIdleTimeout);
          if (result == null) return;
          await mainSetLocalOption(kOptionKeepAwakeIdleTimeout, result);
          setState(() => _keepAwakeIdleTimeout = result);
        } else {
          await mainSetLocalOption(kOptionKeepAwakeIdleTimeout, v);
          setState(() => _keepAwakeIdleTimeout = v);
        }
      },
    );
  }
```

- [ ] **Step 5: Add import for the dialog if needed**

`changeKeepAwakeCustomTimeout` is defined in `flutter/lib/common/widgets/dialog.dart`. Check the imports at the top of `settings_page.dart` — `dialog.dart` is very likely already imported. If not, add:

```dart
import '../../common/widgets/dialog.dart';
```

- [ ] **Step 6: Verify it compiles**

```bash
cd flutter && flutter analyze lib/mobile/pages/settings_page.dart
```

Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add flutter/lib/mobile/pages/settings_page.dart
git commit -m "feat: add keep-awake idle timeout dropdown to mobile settings"
```

---

### Task 6: End-to-end smoke test

No automated test exists for this feature (it depends on device screen-lock behavior). Manual verification steps:

- [ ] **Step 1: Build and run on iOS simulator or device**

```bash
cd flutter && flutter run --debug
```

- [ ] **Step 2: Verify Settings UI**
  - Open Settings
  - Confirm "Keep awake during outgoing sessions" toggle is visible
  - Toggle ON → confirm dropdown appears below it with options
  - Toggle OFF → confirm dropdown disappears
  - Select "After 30 seconds" → confirm it persists after leaving and re-entering Settings
  - Select "Custom..." → confirm dialog opens, enter `2` (minutes) → confirm stored value reads as `custom:120`

- [ ] **Step 3: Verify timer behavior in session**
  - Set timeout to "After 30 seconds"
  - Connect to a remote peer
  - Leave the device idle for >30 seconds
  - Confirm the screen locks normally
  - Unlock the phone back to Tabby
  - Confirm the session resumes and wakelock is re-engaged (screen stays on again)
  - Touch the remote canvas → confirm timer resets (screen stays on for another 30 seconds of idle)

- [ ] **Step 4: Verify "Never" behavior**
  - Set timeout to "Never (keep screen on)"
  - Connect to a remote peer
  - Confirm screen stays on indefinitely

- [ ] **Step 5: Deploy to TestFlight for device testing**

Follow `tabby-testflight` skill or the quick reference in `CLAUDE.md`.
