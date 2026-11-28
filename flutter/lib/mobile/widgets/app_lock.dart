import 'package:flutter/material.dart';
import 'package:get/get.dart';

import '../../common.dart';
import '../../consts.dart';
import '../../models/platform_model.dart';

const kAppLockOk = 'ok';
const kAppLockUnavailable = 'unavailable';
const kAppLockDefaultTimeout = '60';
const _failed = 'failed';

final appLock = AppLock();

class AppLock {
  AppLock({Future<String?> Function(String title)? authenticator})
      : _authenticator = authenticator ?? _androidAuthenticate;

  final Future<String?> Function(String title) _authenticator;
  late final locked = enabled.obs;
  bool _authenticating = false;
  bool _graceEligible = false;
  bool _promptOnResume = false;

  static Future<String?> _androidAuthenticate(String title) => gFFI
      .invokeMethodWithResult<String>('app_lock_authenticate', {'title': title});

  bool get enabled => mainGetLocalBoolOptionSync(kOptionAllowAppLock);

  String get timeout {
    final value = bind.mainGetLocalOption(key: kOptionAppLockTimeout);
    return int.tryParse(value) == null ? kAppLockDefaultTimeout : value;
  }

  Future<String> authenticate() async {
    if (_authenticating) return _failed;
    _authenticating = true;
    try {
      final result =
          await _authenticator(bind.mainGetAppNameSync()) ?? _failed;
      if (result == kAppLockOk) locked.value = false;
      return result;
    } catch (e) {
      debugPrint('App lock authentication failed: $e');
      return _failed;
    } finally {
      _authenticating = false;
    }
  }

  Future<void> setEnabled(bool value) async {
    final result = await authenticate();
    if (result == kAppLockUnavailable) {
      showToast(translate('app-lock-no-screen-lock-tip'));
    }
    if (result != kAppLockOk) return;
    await mainSetLocalBoolOption(kOptionAllowAppLock, value);
  }

  Future<void> whenUnlocked() async {
    if (!locked.value) return;
    await locked.stream.firstWhere((locked) => !locked);
  }

  void onPaused() {
    if (!enabled) return;
    // Before Android 11 the system credential screen pauses the app too, and
    // asking again when it closes would never let the user cancel.
    _promptOnResume = !_authenticating;
    if (locked.value) return;
    locked.value = true;
    _graceEligible = true;
  }

  // Android reports this before it resumes the app, so the grace period is
  // settled before the first frame and the lock screen does not flash.
  void onIdle(int idleMs) {
    if (!_graceEligible) return;
    _graceEligible = false;
    if (idleMs < int.parse(timeout) * 1000) locked.value = false;
  }

  // Closing the system prompt also resumes the app, so only a return from the
  // background asks by itself.
  Future<void> onResumed() async {
    _graceEligible = false;
    if (!_promptOnResume) return;
    _promptOnResume = false;
    if (locked.value) await authenticate();
  }
}

class AppLockGate extends StatefulWidget {
  const AppLockGate({super.key, required this.lock, required this.child});

  final AppLock lock;
  final Widget child;

  @override
  State<AppLockGate> createState() => _AppLockGateState();
}

class _AppLockGateState extends State<AppLockGate> with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    if (widget.lock.locked.value) {
      WidgetsBinding.instance
          .addPostFrameCallback((_) => widget.lock.authenticate());
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.paused) {
      widget.lock.onPaused();
    } else if (state == AppLifecycleState.resumed) {
      widget.lock.onResumed();
    }
  }

  @override
  Widget build(BuildContext context) {
    return Obx(() => Stack(fit: StackFit.expand, children: [
          widget.child,
          if (widget.lock.locked.value)
            _LockScreen(onUnlock: widget.lock.authenticate),
        ]));
  }
}

class _LockScreen extends StatefulWidget {
  const _LockScreen({required this.onUnlock});

  final Future<String> Function() onUnlock;

  @override
  State<_LockScreen> createState() => _LockScreenState();
}

class _LockScreenState extends State<_LockScreen> {
  final _focusScope = FocusScopeNode();
  bool _unavailable = false;

  @override
  void initState() {
    super.initState();
    // A session underneath keeps reading the hardware keyboard while it has
    // focus, so the lock screen has to take it.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focusScope.requestFocus();
    });
  }

  // Not in dispose(): the nodes are already detached there, and focus would
  // fall back to the root scope instead of the page underneath.
  @override
  void deactivate() {
    _focusScope.unfocus(disposition: UnfocusDisposition.previouslyFocusedChild);
    super.deactivate();
  }

  @override
  void dispose() {
    _focusScope.dispose();
    super.dispose();
  }

  Future<void> _unlock() async {
    final unavailable = await widget.onUnlock() == kAppLockUnavailable;
    if (mounted) setState(() => _unavailable = unavailable);
  }

  @override
  Widget build(BuildContext context) {
    return FocusScope(
      node: _focusScope,
      // Keeps screen readers from reading and activating what is underneath.
      child: BlockSemantics(
          child: Material(
        color: Theme.of(context).scaffoldBackgroundColor,
        child: Center(
          child: Column(mainAxisSize: MainAxisSize.min, children: [
            const Icon(Icons.lock_outline, size: 64),
            const SizedBox(height: 24),
            ElevatedButton(
              onPressed: _unlock,
              child: Text(translate('Unlock')),
            ),
            if (_unavailable)
              Padding(
                padding: const EdgeInsets.all(24),
                child: Text(translate('app-lock-no-screen-lock-tip'),
                    textAlign: TextAlign.center),
              ),
          ]),
        ),
      )),
    );
  }
}
