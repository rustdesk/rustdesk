import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/common/shared_state.dart';
import 'package:flutter_hbb/common/widgets/dialog.dart';
import 'package:flutter_hbb/common/widgets/login.dart';
import 'package:flutter_hbb/consts.dart';
import 'package:flutter_hbb/desktop/widgets/remote_toolbar.dart';
import 'package:flutter_hbb/models/model.dart';
import 'package:flutter_hbb/models/platform_model.dart';
import 'package:flutter_hbb/models/shortcut_model.dart';
import 'package:flutter_hbb/utils/multi_window_manager.dart';
import 'package:get/get.dart';

bool isEditOsPassword = false;

/// Action IDs that `toolbarControls` is the sole registrar for. Wiped on
/// every call so stale closures don't outlive the menu entry that owned
/// them. Actions registered by `registerSessionShortcutActions` MUST NOT
/// appear here. `kShortcutActionToggleRecording` is platform-conditional
/// and handled separately in the unregister pass below.
const _kToolbarOwnedActionIds = <String>[
  kShortcutActionSendCtrlAltDel,
  kShortcutActionRestartRemote,
  kShortcutActionInsertLock,
  kShortcutActionToggleBlockInput,
  kShortcutActionSwitchSides,
  kShortcutActionRefresh,
  kShortcutActionScreenshot,
  kShortcutActionResetCanvas,
  kShortcutActionSendClipboardKeystrokes,
];

const _kToolbarViewStyleActionIds = <String>[
  kShortcutActionViewModeOriginal,
  kShortcutActionViewModeAdaptive,
  kShortcutActionViewModeCustom,
];

const _kToolbarImageQualityActionIds = <String>[
  kShortcutActionImageQualityBest,
  kShortcutActionImageQualityBalanced,
  kShortcutActionImageQualityLow,
];

const _kToolbarCodecActionIds = <String>[
  kShortcutActionCodecAuto,
  kShortcutActionCodecVp8,
  kShortcutActionCodecVp9,
  kShortcutActionCodecAv1,
  kShortcutActionCodecH264,
  kShortcutActionCodecH265,
];

const _kToolbarCursorActionIds = <String>[
  kShortcutActionToggleShowRemoteCursor,
  kShortcutActionToggleFollowRemoteCursor,
  kShortcutActionToggleFollowRemoteWindow,
  kShortcutActionToggleZoomCursor,
];

const _kToolbarDisplayToggleActionIds = <String>[
  kShortcutActionToggleQualityMonitor,
  kShortcutActionToggleMute,
  kShortcutActionToggleEnableFileCopyPaste,
  kShortcutActionToggleDisableClipboard,
  kShortcutActionToggleLockAfterSessionEnd,
  kShortcutActionToggleTrueColor,
];

const _kToolbarKeyboardToggleActionIds = <String>[
  kShortcutActionToggleSwapCtrlCmd,
  kShortcutActionToggleSwapLeftRightMouse,
];

class TTextMenu {
  final Widget child;
  final VoidCallback? onPressed;
  Widget? trailingIcon;
  bool divider;
  final String? actionId;
  TTextMenu(
      {required this.child,
      required this.onPressed,
      this.trailingIcon,
      this.divider = false,
      this.actionId});

  Widget getChild() {
    if (trailingIcon != null) {
      return Row(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          child,
          trailingIcon!,
        ],
      );
    } else {
      return child;
    }
  }
}

class TRadioMenu<T> {
  final Widget child;
  final T value;
  final T groupValue;
  final ValueChanged<T?>? onChanged;
  final String? actionId;

  TRadioMenu(
      {required this.child,
      required this.value,
      required this.groupValue,
      required this.onChanged,
      this.actionId});
}

class TToggleMenu {
  final Widget child;
  final bool value;
  final ValueChanged<bool?>? onChanged;
  final String? actionId;
  TToggleMenu(
      {required this.child,
      required this.value,
      required this.onChanged,
      this.actionId});
}

/// Register each tagged entry's `onChanged` with the session [ShortcutModel].
/// Passthrough — returns [menus] so a caller can wrap `return [...]` directly.
List<TToggleMenu> _registerToggleMenuShortcuts(
  FFI ffi,
  List<TToggleMenu> menus, {
  List<String> ownedActionIds = const [],
}) {
  for (final actionId in ownedActionIds) {
    ffi.shortcutModel.unregister(actionId);
  }
  for (final menu in menus) {
    final actionId = menu.actionId;
    if (actionId == null) continue;
    final onChanged = menu.onChanged;
    if (onChanged == null) {
      ffi.shortcutModel.unregister(actionId);
    } else {
      final value = menu.value;
      ffi.shortcutModel.register(actionId, () => onChanged(!value));
    }
  }
  return menus;
}

/// Radio variant of [_registerToggleMenuShortcuts].
List<TRadioMenu<T>> _registerRadioMenuShortcuts<T>(
  FFI ffi,
  List<TRadioMenu<T>> menus, {
  List<String> ownedActionIds = const [],
}) {
  for (final actionId in ownedActionIds) {
    ffi.shortcutModel.unregister(actionId);
  }
  for (final menu in menus) {
    final actionId = menu.actionId;
    if (actionId == null) continue;
    final onChanged = menu.onChanged;
    if (onChanged == null) {
      ffi.shortcutModel.unregister(actionId);
    } else {
      final value = menu.value;
      ffi.shortcutModel.register(actionId, () => onChanged(value));
    }
  }
  return menus;
}

handleOsPasswordEditIcon(
    SessionID sessionId, OverlayDialogManager dialogManager) {
  isEditOsPassword = true;
  showSetOSPassword(
      sessionId, false, dialogManager, null, () => isEditOsPassword = false);
}

handleOsPasswordAction(
    SessionID sessionId, OverlayDialogManager dialogManager) async {
  if (isEditOsPassword) {
    isEditOsPassword = false;
    return;
  }
  final password =
      await bind.sessionGetOption(sessionId: sessionId, arg: 'os-password') ??
          '';
  if (password.isEmpty) {
    showSetOSPassword(sessionId, true, dialogManager, password,
        () => isEditOsPassword = false);
  } else {
    bind.sessionInputOsPassword(sessionId: sessionId, value: password);
  }
}

List<TTextMenu> toolbarControls(BuildContext context, String id, FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final perms = ffiModel.permissions;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;

  // Wipe stale registrations from previous menu builds before re-registering
  // below; runs unconditionally so mid-session enable works without reconnect.
  for (final actionId in _kToolbarOwnedActionIds) {
    ffi.shortcutModel.unregister(actionId);
  }
  // toggle_recording is mobile-only here; desktop's registration is owned by
  // `registerSessionShortcutActions` and must not be touched.
  if (!(isDesktop || isWeb)) {
    ffi.shortcutModel.unregister(kShortcutActionToggleRecording);
  }

  List<TTextMenu> v = [];
  // elevation
  if (isDefaultConn &&
      perms['keyboard'] != false &&
      ffi.elevationModel.showRequestMenu) {
    v.add(
      TTextMenu(
          child: Text(translate('Request Elevation')),
          onPressed: () =>
              showRequestElevationDialog(sessionId, ffi.dialogManager)),
    );
  }
  // osAccount / osPassword
  if (isDefaultConn && perms['keyboard'] != false) {
    v.add(
      TTextMenu(
        child: Row(children: [
          Text(translate(pi.isHeadless ? 'OS Account' : 'OS Password')),
        ]),
        trailingIcon: Transform.scale(
          scale: (isDesktop || isWebDesktop) ? 0.8 : 1,
          child: IconButton(
            onPressed: () {
              if (isMobile && Navigator.canPop(context)) {
                Navigator.pop(context);
              }
              if (pi.isHeadless) {
                showSetOSAccount(sessionId, ffi.dialogManager);
              } else {
                handleOsPasswordEditIcon(sessionId, ffi.dialogManager);
              }
            },
            icon: Icon(Icons.edit, color: isMobile ? MyTheme.accent : null),
          ),
        ),
        onPressed: () => pi.isHeadless
            ? showSetOSAccount(sessionId, ffi.dialogManager)
            : handleOsPasswordAction(sessionId, ffi.dialogManager),
      ),
    );
  }
  // paste
  if (isDefaultConn &&
      pi.platform != kPeerPlatformAndroid &&
      perms['keyboard'] != false) {
    v.add(TTextMenu(
        child: Text(translate('Send clipboard keystrokes')),
        onPressed: () async {
          ClipboardData? data = await Clipboard.getData(Clipboard.kTextPlain);
          if (data != null && data.text != null) {
            bind.sessionInputString(
                sessionId: sessionId, value: data.text ?? "");
          }
        },
        actionId: kShortcutActionSendClipboardKeystrokes));
  }
  // reset canvas
  if (isDefaultConn && isMobile) {
    v.add(TTextMenu(
        child: Text(translate('Reset canvas')),
        onPressed: () => ffi.cursorModel.reset(),
        actionId: kShortcutActionResetCanvas));
  }

  // https://github.com/rustdesk/rustdesk/pull/9731
  // Does not work for connection established by "accept".
  connectWithToken(
      {bool isFileTransfer = false,
      bool isViewCamera = false,
      bool isTcpTunneling = false,
      bool isTerminal = false}) {
    final connToken = bind.sessionGetConnToken(sessionId: ffi.sessionId);
    connect(context, id,
        isFileTransfer: isFileTransfer,
        isViewCamera: isViewCamera,
        isTerminal: isTerminal,
        isTcpTunneling: isTcpTunneling,
        connToken: connToken);
  }

  if (isDefaultConn && isDesktop) {
    v.add(
      TTextMenu(
          child: Text(translate('Transfer file')),
          onPressed: () => connectWithToken(isFileTransfer: true)),
    );
    v.add(
      TTextMenu(
          child: Text(translate('View camera')),
          onPressed: () => connectWithToken(isViewCamera: true)),
    );
    v.add(
      TTextMenu(
          child: Text('${translate('Terminal')} (beta)'),
          onPressed: () => connectWithToken(isTerminal: true)),
    );
    v.add(
      TTextMenu(
          child: Text(translate('TCP tunneling')),
          onPressed: () => connectWithToken(isTcpTunneling: true)),
    );
  }
  // note
  if (isDefaultConn && !bind.isDisableAccount()) {
    v.add(
      TTextMenu(
          child: Text(translate('Note')),
          onPressed: () async {
            bool isLogin =
                bind.mainGetLocalOption(key: 'access_token').isNotEmpty;
            if (!isLogin) {
              final res = await loginDialog();
              if (res != true) return;
              // Desktop: send message to main window to refresh login status
              // Web: login is required before connection, so no need to refresh
              // Mobile: same isolate, no need to send message
              if (isDesktop) {
                rustDeskWinManager.call(
                    WindowType.Main, kWindowRefreshCurrentUser, "");
              }
            }
            showAuditDialog(ffi);
          }),
    );
  }
  // divider
  if (isDefaultConn && (isDesktop || isWebDesktop)) {
    v.add(TTextMenu(child: Offstage(), onPressed: () {}, divider: true));
  }
  // ctrlAltDel
  if (isDefaultConn &&
      !ffiModel.viewOnly &&
      ffiModel.keyboard &&
      (pi.platform == kPeerPlatformLinux || pi.sasEnabled)) {
    v.add(
      TTextMenu(
          child: Text('${translate("Insert Ctrl + Alt + Del")}'),
          onPressed: () => bind.sessionCtrlAltDel(sessionId: sessionId),
          actionId: kShortcutActionSendCtrlAltDel),
    );
  }
  // restart
  if (isDefaultConn &&
      perms['restart'] != false &&
      (pi.platform == kPeerPlatformLinux ||
          pi.platform == kPeerPlatformWindows ||
          pi.platform == kPeerPlatformMacOS)) {
    v.add(
      TTextMenu(
          child: Text(translate('Restart remote device')),
          onPressed: () =>
              showRestartRemoteDevice(pi, id, sessionId, ffi.dialogManager),
          actionId: kShortcutActionRestartRemote),
    );
  }
  // insertLock
  if (isDefaultConn && !ffiModel.viewOnly && ffi.ffiModel.keyboard) {
    v.add(
      TTextMenu(
          child: Text(translate('Insert Lock')),
          onPressed: () => bind.sessionLockScreen(sessionId: sessionId),
          actionId: kShortcutActionInsertLock),
    );
  }
  // blockUserInput
  if (isDefaultConn &&
      ffi.ffiModel.keyboard &&
      ffi.ffiModel.permissions['block_input'] != false &&
      pi.platform == kPeerPlatformWindows) // privacy-mode != true ??
  {
    v.add(TTextMenu(
        child: Obx(() => Text(translate(
            '${BlockInputState.find(id).value ? 'Unb' : 'B'}lock user input'))),
        onPressed: () {
          RxBool blockInput = BlockInputState.find(id);
          bind.sessionToggleOption(
              sessionId: sessionId,
              value: '${blockInput.value ? 'un' : ''}block-input');
          blockInput.value = !blockInput.value;
        },
        actionId: kShortcutActionToggleBlockInput));
  }
  // switchSides
  if (isDefaultConn &&
      isDesktop &&
      ffiModel.keyboard &&
      pi.platform != kPeerPlatformAndroid &&
      versionCmp(pi.version, '1.2.0') >= 0 &&
      bind.peerGetSessionsCount(id: id, connType: ffi.connType.index) == 1) {
    v.add(TTextMenu(
        child: Text(translate('Switch Sides')),
        onPressed: () =>
            showConfirmSwitchSidesDialog(sessionId, id, ffi.dialogManager),
        actionId: kShortcutActionSwitchSides));
  }
  // refresh
  if (pi.version.isNotEmpty) {
    v.add(TTextMenu(
      child: Text(translate('Refresh')),
      onPressed: () => sessionRefreshVideo(sessionId, pi),
      actionId: kShortcutActionRefresh,
    ));
  }
  // record
  if (!(isDesktop || isWeb) &&
      (ffi.recordingModel.start || (perms["recording"] != false))) {
    v.add(TTextMenu(
        child: Row(
          children: [
            Text(translate(ffi.recordingModel.start
                ? 'Stop session recording'
                : 'Start session recording')),
            Padding(
              padding: EdgeInsets.only(left: 12),
              child: Icon(
                  ffi.recordingModel.start
                      ? Icons.pause_circle_filled
                      : Icons.videocam_outlined,
                  color: MyTheme.accent),
            )
          ],
        ),
        onPressed: () => ffi.recordingModel.toggle(),
        actionId: kShortcutActionToggleRecording));
  }

  // to-do:
  // 1. Web desktop
  // 2. Mobile, copy the image to the clipboard
  if (isDesktop) {
    final isScreenshotSupported = bind.sessionGetCommonSync(
        sessionId: sessionId, key: 'is_screenshot_supported', param: '');
    if ('true' == isScreenshotSupported) {
      v.add(TTextMenu(
        child: Text(ffi.ffiModel.timerScreenshot != null
            ? '${translate('Taking screenshot')} ...'
            : translate('Take screenshot')),
        onPressed: ffi.ffiModel.timerScreenshot != null
            ? null
            : () {
                // Live cooldown check: the menu rebuilds onPressed=null
                // whenever toolbarControls runs and finds timerScreenshot
                // != null, but the keyboard-shortcut callback holds onto
                // the originally-enabled closure across cooldown periods
                // (toolbarControls only re-runs on menu open). Without
                // this guard the second shortcut press during the 30s
                // cooldown still fires sessionTakeScreenshot.
                if (ffi.ffiModel.timerScreenshot != null) return;
                if (pi.currentDisplay == kAllDisplayValue) {
                  msgBox(
                      sessionId,
                      'custom-nook-nocancel-hasclose-info',
                      'Take screenshot',
                      'screenshot-merged-screen-not-supported-tip',
                      '',
                      ffi.dialogManager);
                } else {
                  bind.sessionTakeScreenshot(
                      sessionId: sessionId, display: pi.currentDisplay);
                  ffi.ffiModel.timerScreenshot =
                      Timer(Duration(seconds: 30), () {
                    ffi.ffiModel.timerScreenshot = null;
                  });
                }
              },
        actionId: kShortcutActionScreenshot,
      ));
    }
  }
  // fingerprint
  if (!(isDesktop || isWebDesktop)) {
    v.add(TTextMenu(
      child: Text(translate('Copy Fingerprint')),
      onPressed: () => onCopyFingerprint(FingerprintState.find(id).value),
    ));
  }
  // Register tagged TTextMenu callbacks. The else-unregister is defense in
  // depth for actionIds tagged but missing from `_kToolbarOwnedActionIds`.
  for (final menu in v) {
    final actionId = menu.actionId;
    if (actionId == null) continue;
    if (menu.onPressed != null) {
      ffi.shortcutModel.register(actionId, menu.onPressed!);
    } else {
      ffi.shortcutModel.unregister(actionId);
    }
  }
  return v;
}

Future<List<TRadioMenu<String>>> toolbarViewStyle(
    BuildContext context, String id, FFI ffi) async {
  final groupValue =
      await bind.sessionGetViewStyle(sessionId: ffi.sessionId) ?? '';
  void onChanged(String? value) async {
    if (value == null) return;
    bind
        .sessionSetViewStyle(sessionId: ffi.sessionId, value: value)
        .then((_) => ffi.canvasModel.updateViewStyle());
  }

  return _registerRadioMenuShortcuts(ffi, [
    TRadioMenu<String>(
        child: Text(translate('Scale original')),
        value: kRemoteViewStyleOriginal,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionViewModeOriginal),
    TRadioMenu<String>(
        child: Text(translate('Scale adaptive')),
        value: kRemoteViewStyleAdaptive,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionViewModeAdaptive),
    TRadioMenu<String>(
        child: Text(translate('Scale custom')),
        value: kRemoteViewStyleCustom,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionViewModeCustom)
  ], ownedActionIds: _kToolbarViewStyleActionIds);
}

Future<List<TRadioMenu<String>>> toolbarImageQuality(
    BuildContext context, String id, FFI ffi) async {
  final groupValue =
      await bind.sessionGetImageQuality(sessionId: ffi.sessionId) ?? '';
  onChanged(String? value) async {
    if (value == null) return;
    await bind.sessionSetImageQuality(sessionId: ffi.sessionId, value: value);
  }

  return _registerRadioMenuShortcuts(ffi, [
    TRadioMenu<String>(
        child: Text(translate('Good image quality')),
        value: kRemoteImageQualityBest,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionImageQualityBest),
    TRadioMenu<String>(
        child: Text(translate('Balanced')),
        value: kRemoteImageQualityBalanced,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionImageQualityBalanced),
    TRadioMenu<String>(
        child: Text(translate('Optimize reaction time')),
        value: kRemoteImageQualityLow,
        groupValue: groupValue,
        onChanged: onChanged,
        actionId: kShortcutActionImageQualityLow),
    TRadioMenu<String>(
      child: Text(translate('Custom')),
      value: kRemoteImageQualityCustom,
      groupValue: groupValue,
      onChanged: (value) {
        onChanged(value);
        customImageQualityDialog(ffi.sessionId, id, ffi);
      },
    ),
  ], ownedActionIds: _kToolbarImageQualityActionIds);
}

Future<List<TRadioMenu<String>>> toolbarCodec(
    BuildContext context, String id, FFI ffi) async {
  final sessionId = ffi.sessionId;
  final alternativeCodecs =
      await bind.sessionAlternativeCodecs(sessionId: sessionId);
  final groupValue = await bind.sessionGetOption(
          sessionId: sessionId, arg: kOptionCodecPreference) ??
      '';
  final List<bool> codecs = [];
  try {
    final Map codecsJson = jsonDecode(alternativeCodecs);
    final vp8 = codecsJson['vp8'] ?? false;
    final av1 = codecsJson['av1'] ?? false;
    final h264 = codecsJson['h264'] ?? false;
    final h265 = codecsJson['h265'] ?? false;
    codecs.add(vp8);
    codecs.add(av1);
    codecs.add(h264);
    codecs.add(h265);
  } catch (e) {
    debugPrint("Show Codec Preference err=$e");
  }
  final visible =
      codecs.length == 4 && (codecs[0] || codecs[1] || codecs[2] || codecs[3]);
  if (!visible) {
    return _registerRadioMenuShortcuts<String>(ffi, [],
        ownedActionIds: _kToolbarCodecActionIds);
  }
  onChanged(String? value) async {
    if (value == null) return;
    await bind.sessionPeerOption(
        sessionId: sessionId, name: kOptionCodecPreference, value: value);
    bind.sessionChangePreferCodec(sessionId: sessionId);
  }

  TRadioMenu<String> radio(
      String label, String value, bool enabled, String actionId) {
    return TRadioMenu<String>(
        child: Text(label),
        value: value,
        groupValue: groupValue,
        onChanged: enabled ? onChanged : null,
        actionId: actionId);
  }

  var autoLabel = translate('Auto');
  if (groupValue == 'auto' &&
      ffi.qualityMonitorModel.data.codecFormat != null) {
    autoLabel = '$autoLabel (${ffi.qualityMonitorModel.data.codecFormat})';
  }
  return _registerRadioMenuShortcuts(ffi, [
    radio(autoLabel, 'auto', true, kShortcutActionCodecAuto),
    if (codecs[0]) radio('VP8', 'vp8', codecs[0], kShortcutActionCodecVp8),
    radio('VP9', 'vp9', true, kShortcutActionCodecVp9),
    if (codecs[1]) radio('AV1', 'av1', codecs[1], kShortcutActionCodecAv1),
    if (codecs[2]) radio('H264', 'h264', codecs[2], kShortcutActionCodecH264),
    if (codecs[3]) radio('H265', 'h265', codecs[3], kShortcutActionCodecH265),
  ], ownedActionIds: _kToolbarCodecActionIds);
}

Future<List<TToggleMenu>> toolbarCursor(
    BuildContext context, String id, FFI ffi) async {
  List<TToggleMenu> v = [];
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;

  // show remote cursor
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland) {
    final state = ShowRemoteCursorState.find(id);
    final lockState = ShowRemoteCursorLockState.find(id);
    final enabled = !ffiModel.viewOnly;
    final option = 'show-remote-cursor';
    if (pi.currentDisplay == kAllDisplayValue ||
        bind.sessionIsMultiUiSession(sessionId: sessionId)) {
      lockState.value = false;
    }
    v.add(TToggleMenu(
        child: Text(translate('Show remote cursor')),
        value: state.value,
        actionId: kShortcutActionToggleShowRemoteCursor,
        onChanged: enabled && !lockState.value
            ? (value) async {
                if (value == null) return;
                await bind.sessionToggleOption(
                    sessionId: sessionId, value: option);
                state.value = bind.sessionGetToggleOptionSync(
                    sessionId: sessionId, arg: option);
              }
            : null));
  }
  // follow remote cursor
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland &&
      versionCmp(pi.version, "1.2.4") >= 0 &&
      pi.displays.length > 1 &&
      pi.currentDisplay != kAllDisplayValue &&
      !bind.sessionIsMultiUiSession(sessionId: sessionId)) {
    final option = 'follow-remote-cursor';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    final showCursorOption = 'show-remote-cursor';
    final showCursorState = ShowRemoteCursorState.find(id);
    final showCursorLockState = ShowRemoteCursorLockState.find(id);
    final showCursorEnabled = bind.sessionGetToggleOptionSync(
        sessionId: sessionId, arg: showCursorOption);
    showCursorLockState.value = value;
    if (value && !showCursorEnabled) {
      await bind.sessionToggleOption(
          sessionId: sessionId, value: showCursorOption);
      showCursorState.value = bind.sessionGetToggleOptionSync(
          sessionId: sessionId, arg: showCursorOption);
    }
    v.add(TToggleMenu(
        child: Text(translate('Follow remote cursor')),
        value: value,
        actionId: kShortcutActionToggleFollowRemoteCursor,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          value = bind.sessionGetToggleOptionSync(
              sessionId: sessionId, arg: option);
          showCursorLockState.value = value;
          if (!showCursorEnabled) {
            await bind.sessionToggleOption(
                sessionId: sessionId, value: showCursorOption);
            showCursorState.value = bind.sessionGetToggleOptionSync(
                sessionId: sessionId, arg: showCursorOption);
          }
        }));
  }
  // follow remote window focus
  if (pi.platform != kPeerPlatformAndroid &&
      !ffi.canvasModel.cursorEmbedded &&
      !pi.isWayland &&
      versionCmp(pi.version, "1.2.4") >= 0 &&
      pi.displays.length > 1 &&
      pi.currentDisplay != kAllDisplayValue &&
      !bind.sessionIsMultiUiSession(sessionId: sessionId)) {
    final option = 'follow-remote-window';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        child: Text(translate('Follow remote window focus')),
        value: value,
        actionId: kShortcutActionToggleFollowRemoteWindow,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          value = bind.sessionGetToggleOptionSync(
              sessionId: sessionId, arg: option);
        }));
  }
  // zoom cursor
  final viewStyle = await bind.sessionGetViewStyle(sessionId: sessionId) ?? '';
  if (!isMobile &&
      pi.platform != kPeerPlatformAndroid &&
      viewStyle != kRemoteViewStyleOriginal) {
    final option = 'zoom-cursor';
    final peerState = PeerBoolOption.find(id, option);
    v.add(TToggleMenu(
      child: Text(translate('Zoom cursor')),
      value: peerState.value,
      actionId: kShortcutActionToggleZoomCursor,
      onChanged: (value) async {
        if (value == null) return;
        await bind.sessionToggleOption(sessionId: sessionId, value: option);
        peerState.value =
            bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
      },
    ));
  }
  return _registerToggleMenuShortcuts(ffi, v,
      ownedActionIds: _kToolbarCursorActionIds);
}

Future<List<TToggleMenu>> toolbarDisplayToggle(
    BuildContext context, String id, FFI ffi) async {
  List<TToggleMenu> v = [];
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final perms = ffiModel.permissions;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;

  // show quality monitor
  final option = 'show-quality-monitor';
  v.add(TToggleMenu(
      value: bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option),
      actionId: kShortcutActionToggleQualityMonitor,
      onChanged: (value) async {
        if (value == null) return;
        await bind.sessionToggleOption(sessionId: sessionId, value: option);
        ffi.qualityMonitorModel.checkShowQualityMonitor(sessionId);
      },
      child: Text(translate('Show quality monitor'))));
  // mute
  if (isDefaultConn && perms['audio'] != false) {
    final option = 'disable-audio';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleMute,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionToggleOption(sessionId: sessionId, value: option);
        },
        child: Text(translate('Mute'))));
  }
  // file copy and paste
  // If the version is less than 1.2.4, file copy and paste is supported on Windows only.
  final isSupportIfPeer_1_2_3 = versionCmp(pi.version, '1.2.4') < 0 &&
      isWindows &&
      pi.platform == kPeerPlatformWindows;
  // If the version is 1.2.4 or later, file copy and paste is supported when kPlatformAdditionsHasFileClipboard is set.
  final isSupportIfPeer_1_2_4 = versionCmp(pi.version, '1.2.4') >= 0 &&
      bind.mainHasFileClipboard() &&
      pi.platformAdditions.containsKey(kPlatformAdditionsHasFileClipboard);
  if (isDefaultConn &&
      ffiModel.keyboard &&
      perms['file'] != false &&
      (isSupportIfPeer_1_2_3 || isSupportIfPeer_1_2_4)) {
    final enabled = !ffiModel.viewOnly;
    final value = bind.sessionGetToggleOptionSync(
        sessionId: sessionId, arg: kOptionEnableFileCopyPaste);
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleEnableFileCopyPaste,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(
                    sessionId: sessionId, value: kOptionEnableFileCopyPaste);
              }
            : null,
        child: Text(translate('Enable file copy and paste'))));
  }
  // disable clipboard
  if (isDefaultConn && ffiModel.keyboard && perms['clipboard'] != false) {
    final enabled = !ffiModel.viewOnly;
    final option = 'disable-clipboard';
    var value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    if (ffiModel.viewOnly) value = true;
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleDisableClipboard,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(sessionId: sessionId, value: option);
              }
            : null,
        child: Text(translate('Disable clipboard'))));
  }
  // lock after session end
  if (isDefaultConn && ffiModel.keyboard && !ffiModel.isPeerAndroid) {
    final enabled = !ffiModel.viewOnly;
    final option = 'lock-after-session-end';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleLockAfterSessionEnd,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                bind.sessionToggleOption(sessionId: sessionId, value: option);
              }
            : null,
        child: Text(translate('Lock after session end'))));
  }

  if (pi.isSupportMultiDisplay &&
      PrivacyModeState.find(id).isEmpty &&
      pi.displaysCount.value > 1 &&
      bind.mainGetUserDefaultOption(key: kKeyShowMonitorsToolbar) == 'Y') {
    final value =
        bind.sessionGetDisplaysAsIndividualWindows(sessionId: ffi.sessionId) ==
            'Y';
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionSetDisplaysAsIndividualWindows(
              sessionId: sessionId, value: value ? 'Y' : 'N');
        },
        child: Text(translate('Show displays as individual windows'))));
  }

  final isMultiScreens = !isWeb && (await getScreenRectList()).length > 1;
  if (pi.isSupportMultiDisplay && isMultiScreens) {
    final value = bind.sessionGetUseAllMyDisplaysForTheRemoteSession(
            sessionId: ffi.sessionId) ==
        'Y';
    v.add(TToggleMenu(
        value: value,
        onChanged: (value) {
          if (value == null) return;
          bind.sessionSetUseAllMyDisplaysForTheRemoteSession(
              sessionId: sessionId, value: value ? 'Y' : 'N');
        },
        child: Text(translate('Use all my displays for the remote session'))));
  }

  // 444
  final codec_format = ffi.qualityMonitorModel.data.codecFormat;
  if (versionCmp(pi.version, "1.2.4") >= 0 &&
      (codec_format == "AV1" || codec_format == "VP9")) {
    final option = 'i444';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleTrueColor,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(sessionId: sessionId, value: option);
          bind.sessionChangePreferCodec(sessionId: sessionId);
        },
        child: Text(translate('True color (4:4:4)'))));
  }

  if (isDefaultConn && isMobile) {
    v.addAll(toolbarKeyboardToggles(ffi));
  }

  // view mode (mobile only, desktop is in keyboard menu)
  if (isDefaultConn && isMobile && versionCmp(pi.version, '1.2.0') >= 0) {
    v.add(TToggleMenu(
        value: ffiModel.viewOnly,
        onChanged: (value) async {
          if (value == null) return;
          await bind.sessionToggleOption(
              sessionId: ffi.sessionId, value: kOptionToggleViewOnly);
          ffiModel.setViewOnly(id, value);
        },
        child: Text(translate('View Mode'))));
  }
  return _registerToggleMenuShortcuts(ffi, v,
      ownedActionIds: _kToolbarDisplayToggleActionIds);
}

var togglePrivacyModeTime = DateTime.now().subtract(const Duration(hours: 1));

List<TToggleMenu> toolbarPrivacyMode(
    RxString privacyModeState, BuildContext context, String id, FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;

  getDefaultMenu(Future<void> Function(SessionID sid, String opt) toggleFunc) {
    final enabled = !ffi.ffiModel.viewOnly;
    return TToggleMenu(
        value: privacyModeState.isNotEmpty,
        onChanged: enabled
            ? (value) {
                if (value == null) return;
                if (ffiModel.pi.currentDisplay != 0 &&
                    ffiModel.pi.currentDisplay != kAllDisplayValue) {
                  msgBox(
                      sessionId,
                      'custom-nook-nocancel-hasclose',
                      'info',
                      'Please switch to Display 1 first',
                      '',
                      ffi.dialogManager);
                  return;
                }
                final option = 'privacy-mode';
                toggleFunc(sessionId, option);
              }
            : null,
        child: Text(translate('Privacy mode')));
  }

  final privacyModeImpls =
      pi.platformAdditions[kPlatformAdditionsSupportedPrivacyModeImpl]
          as List<dynamic>?;
  if (privacyModeImpls == null) {
    return [
      getDefaultMenu((sid, opt) async {
        bind.sessionToggleOption(sessionId: sid, value: opt);
        togglePrivacyModeTime = DateTime.now();
      })
    ];
  }
  if (privacyModeImpls.isEmpty) {
    return [];
  }

  if (privacyModeImpls.length == 1) {
    final implKey = (privacyModeImpls[0] as List<dynamic>)[0] as String;
    return [
      getDefaultMenu((sid, opt) async {
        bind.sessionTogglePrivacyMode(
            sessionId: sid, implKey: implKey, on: privacyModeState.isEmpty);
        togglePrivacyModeTime = DateTime.now();
      })
    ];
  } else {
    return privacyModeImpls.map((e) {
      final implKey = (e as List<dynamic>)[0] as String;
      final implName = (e)[1] as String;
      return TToggleMenu(
          child: Text(translate(implName)),
          value: privacyModeState.value == implKey,
          onChanged: (value) {
            if (value == null) return;
            togglePrivacyModeTime = DateTime.now();
            bind.sessionTogglePrivacyMode(
                sessionId: sessionId, implKey: implKey, on: value);
          });
    }).toList();
  }
}

List<TToggleMenu> toolbarKeyboardToggles(FFI ffi) {
  final ffiModel = ffi.ffiModel;
  final pi = ffiModel.pi;
  final sessionId = ffi.sessionId;
  final isDefaultConn = ffi.connType == ConnType.defaultConn;
  List<TToggleMenu> v = [];

  // swap key
  if (ffiModel.keyboard &&
      ((isMacOS && pi.platform != kPeerPlatformMacOS) ||
          (!isMacOS && pi.platform == kPeerPlatformMacOS))) {
    final option = 'allow_swap_key';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    onChanged(bool? value) {
      if (value == null) return;
      bind.sessionToggleOption(sessionId: sessionId, value: option);
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleSwapCtrlCmd,
        onChanged: enabled ? onChanged : null,
        child: Text(translate('Swap control-command key'))));
  }

  // Relative mouse mode (gaming mode).
  // Only show when server supports MOUSE_TYPE_MOVE_RELATIVE (version >= 1.4.5)
  // Note: This feature is only available in Flutter client. Sciter client does not support this.
  // Web client is not supported yet due to Pointer Lock API integration complexity with Flutter's input system.
  // Wayland is not supported due to cursor warping limitations.
  // Mobile: This option is now in GestureHelp widget, shown only when joystick is visible.
  final isWayland = isDesktop && isLinux && bind.mainCurrentIsWayland();
  if (isDesktop &&
      isDefaultConn &&
      !isWeb &&
      !isWayland &&
      ffiModel.keyboard &&
      !ffiModel.viewOnly &&
      ffi.inputModel.isRelativeMouseModeSupported) {
    v.add(TToggleMenu(
        value: ffi.inputModel.relativeMouseMode.value,
        onChanged: (value) {
          if (value == null) return;
          final previousValue = ffi.inputModel.relativeMouseMode.value;
          final success = ffi.inputModel.setRelativeMouseMode(value);
          if (!success) {
            // Revert the observable toggle to reflect the actual state
            ffi.inputModel.relativeMouseMode.value = previousValue;
          }
        },
        child: Text(translate('Relative mouse mode'))));
  }

  // reverse mouse wheel
  if (ffiModel.keyboard) {
    var optionValue =
        bind.sessionGetReverseMouseWheelSync(sessionId: sessionId) ?? '';
    if (optionValue == '') {
      optionValue = bind.mainGetUserDefaultOption(key: kKeyReverseMouseWheel);
    }
    onChanged(bool? value) async {
      if (value == null) return;
      await bind.sessionSetReverseMouseWheel(
          sessionId: sessionId, value: value ? 'Y' : 'N');
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: optionValue == 'Y',
        onChanged: enabled ? onChanged : null,
        child: Text(translate('Reverse mouse wheel'))));
  }

  // swap left right mouse
  if (ffiModel.keyboard) {
    final option = 'swap-left-right-mouse';
    final value =
        bind.sessionGetToggleOptionSync(sessionId: sessionId, arg: option);
    onChanged(bool? value) {
      if (value == null) return;
      bind.sessionToggleOption(sessionId: sessionId, value: option);
    }

    final enabled = !ffi.ffiModel.viewOnly;
    v.add(TToggleMenu(
        value: value,
        actionId: kShortcutActionToggleSwapLeftRightMouse,
        onChanged: enabled ? onChanged : null,
        child: Text(translate('swap-left-right-mouse'))));
  }
  return _registerToggleMenuShortcuts(ffi, v,
      ownedActionIds: _kToolbarKeyboardToggleActionIds);
}

/// Drive each toolbar helper for its registration side effect, so a shortcut
/// fires from the first keystroke without needing the user to open the
/// matching submenu. Mobile gets `toolbarKeyboardToggles` via
/// `toolbarDisplayToggle`'s `isMobile` branch — calling it explicitly there
/// would double-register.
void registerToolbarShortcuts(BuildContext context, String id, FFI ffi) {
  if (isDesktop) toolbarKeyboardToggles(ffi);
  unawaited(toolbarCursor(context, id, ffi));
  unawaited(toolbarDisplayToggle(context, id, ffi));
  unawaited(toolbarViewStyle(context, id, ffi));
  unawaited(toolbarImageQuality(context, id, ffi));
  unawaited(toolbarCodec(context, id, ffi));
  toolbarPrivacyMode(PrivacyModeState.find(id), context, id, ffi);
}

bool showVirtualDisplayMenu(FFI ffi) {
  if (ffi.ffiModel.pi.platform != kPeerPlatformWindows) {
    return false;
  }
  if (!ffi.ffiModel.pi.isInstalled) {
    return false;
  }
  if (ffi.ffiModel.pi.isRustDeskIdd || ffi.ffiModel.pi.isAmyuniIdd) {
    return true;
  }
  return false;
}

List<Widget> getVirtualDisplayMenuChildren(
    FFI ffi, String id, VoidCallback? clickCallBack) {
  if (!showVirtualDisplayMenu(ffi)) {
    return [];
  }
  final pi = ffi.ffiModel.pi;
  final privacyModeState = PrivacyModeState.find(id);
  if (pi.isRustDeskIdd) {
    final virtualDisplays = ffi.ffiModel.pi.RustDeskVirtualDisplays;
    final children = <Widget>[];
    for (var i = 0; i < kMaxVirtualDisplayCount; i++) {
      children.add(Obx(() => CkbMenuButton(
            value: virtualDisplays.contains(i + 1),
            onChanged: privacyModeState.isNotEmpty
                ? null
                : (bool? value) async {
                    if (value != null) {
                      bind.sessionToggleVirtualDisplay(
                          sessionId: ffi.sessionId, index: i + 1, on: value);
                      clickCallBack?.call();
                    }
                  },
            child: Text('${translate('Virtual display')} ${i + 1}'),
            ffi: ffi,
          )));
    }
    children.add(Divider());
    children.add(Obx(() => MenuButton(
          onPressed: privacyModeState.isNotEmpty
              ? null
              : () {
                  bind.sessionToggleVirtualDisplay(
                      sessionId: ffi.sessionId,
                      index: kAllVirtualDisplay,
                      on: false);
                  clickCallBack?.call();
                },
          ffi: ffi,
          child: Text(translate('Plug out all')),
        )));
    return children;
  }
  if (pi.isAmyuniIdd) {
    final count = ffi.ffiModel.pi.amyuniVirtualDisplayCount;
    final children = <Widget>[
      Obx(() => Row(
            children: [
              TextButton(
                onPressed: privacyModeState.isNotEmpty || count == 0
                    ? null
                    : () {
                        bind.sessionToggleVirtualDisplay(
                            sessionId: ffi.sessionId, index: 0, on: false);
                        clickCallBack?.call();
                      },
                child: Icon(Icons.remove),
              ),
              Text(count.toString()),
              TextButton(
                onPressed: privacyModeState.isNotEmpty || count == 4
                    ? null
                    : () {
                        bind.sessionToggleVirtualDisplay(
                            sessionId: ffi.sessionId, index: 0, on: true);
                        clickCallBack?.call();
                      },
                child: Icon(Icons.add),
              ),
            ],
          )),
      Divider(),
      Obx(() => MenuButton(
            onPressed: privacyModeState.isNotEmpty || count == 0
                ? null
                : () {
                    bind.sessionToggleVirtualDisplay(
                        sessionId: ffi.sessionId,
                        index: kAllVirtualDisplay,
                        on: false);
                    clickCallBack?.call();
                  },
            ffi: ffi,
            child: Text(translate('Plug out all')),
          )),
    ];
    return children;
  }
  return [];
}
