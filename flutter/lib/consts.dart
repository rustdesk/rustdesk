import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';
import 'package:flutter_hbb/models/state_model.dart';
import 'package:get/get.dart';

const int kMaxVirtualDisplayCount = 4;
const int kAllVirtualDisplay = -1;

const double kDesktopRemoteTabBarHeight = 28.0;
const int kInvalidWindowId = -1;
const int kMainWindowId = 0;

const kAllDisplayValue = -1;

const kKeyLegacyMode = 'legacy';
const kKeyMapMode = 'map';
const kKeyTranslateMode = 'translate';

const String kPlatformAdditionsIsWayland = "is_wayland";
const String kPlatformAdditionsHeadless = "headless";
const String kPlatformAdditionsIsInstalled = "is_installed";
const String kPlatformAdditionsIddImpl = "idd_impl";
const String kPlatformAdditionsRustDeskVirtualDisplays =
    "rustdesk_virtual_displays";
const String kPlatformAdditionsAmyuniVirtualDisplays =
    "amyuni_virtual_displays";
const String kPlatformAdditionsHasFileClipboard = "has_file_clipboard";
const String kPlatformAdditionsSupportedPrivacyModeImpl =
    "supported_privacy_mode_impl";

const String kPeerPlatformWindows = "Windows";
const String kPeerPlatformLinux = "Linux";
const String kPeerPlatformMacOS = "Mac OS";
const String kPeerPlatformAndroid = "Android";
const String kPeerPlatformWebDesktop = "WebDesktop";

const double kScrollbarThickness = 12.0;

/// [kAppTypeMain] used by 'Desktop Main Page' , 'Mobile (Client and Server)', "Install Page"
const String kAppTypeMain = "main";

/// [kAppTypeConnectionManager] only for 'Desktop CM Page'
const String kAppTypeConnectionManager = "cm";

const String kAppTypeDesktopRemote = "remote";
const String kAppTypeDesktopFileTransfer = "file transfer";
const String kAppTypeDesktopViewCamera = "view camera";
const String kAppTypeDesktopPortForward = "port forward";
const String kAppTypeDesktopTerminal = "terminal";

const String kWindowMainWindowOnTop = "main_window_on_top";
const String kWindowGetWindowInfo = "get_window_info";
const String kWindowGetScreenList = "get_screen_list";
// This method is not used, maybe it can be removed.
const String kWindowDisableGrabKeyboard = "disable_grab_keyboard";
const String kWindowActionRebuild = "rebuild";
const String kWindowEventHide = "hide";
const String kWindowEventShow = "show";
const String kWindowConnect = "connect";

const String kWindowEventNewRemoteDesktop = "new_remote_desktop";
const String kWindowEventNewFileTransfer = "new_file_transfer";
const String kWindowEventNewViewCamera = "new_view_camera";
const String kWindowEventNewPortForward = "new_port_forward";
const String kWindowEventNewTerminal = "new_terminal";
const String kWindowEventRestoreTerminalSessions = "restore_terminal_sessions";
const String kWindowEventActiveSession = "active_session";
const String kWindowEventActiveDisplaySession = "active_display_session";
const String kWindowEventGetRemoteList = "get_remote_list";
const String kWindowEventGetSessionIdList = "get_session_id_list";
const String kWindowEventRemoteWindowCoords = "remote_window_coords";
const String kWindowEventSetFullscreen = "set_fullscreen";

const String kWindowEventMoveTabToNewWindow = "move_tab_to_new_window";
const String kWindowEventGetCachedSessionData = "get_cached_session_data";
const String kWindowEventOpenMonitorSession = "open_monitor_session";

const String kOptionViewStyle = "view_style";
const String kOptionScrollStyle = "scroll_style";
const String kOptionImageQuality = "image_quality";
const String kOptionOpenNewConnInTabs = "enable-open-new-connections-in-tabs";
const String kOptionTextureRender = "use-texture-render";
const String kOptionD3DRender = "allow-d3d-render";
const String kOptionOpenInTabs = "allow-open-in-tabs";
const String kOptionOpenInWindows = "allow-open-in-windows";
const String kOptionForceAlwaysRelay = "force-always-relay";
const String kOptionViewOnly = "view_only";
const String kOptionEnableLanDiscovery = "enable-lan-discovery";
const String kOptionWhitelist = "whitelist";
const String kOptionEnableAbr = "enable-abr";
const String kOptionEnableRecordSession = "enable-record-session";
const String kOptionDirectServer = "direct-server";
const String kOptionDirectAccessPort = "direct-access-port";
const String kOptionAllowAutoDisconnect = "allow-auto-disconnect";
const String kOptionAutoDisconnectTimeout = "auto-disconnect-timeout";
const String kOptionEnableHwcodec = "enable-hwcodec";
const String kOptionAllowAutoRecordIncoming = "allow-auto-record-incoming";
const String kOptionAllowAutoRecordOutgoing = "allow-auto-record-outgoing";
const String kOptionVideoSaveDirectory = "video-save-directory";
const String kOptionAccessMode = "access-mode";
const String kOptionEnableKeyboard = "enable-keyboard";
// "Settings -> Security -> Permissions"
const String kOptionEnableRemotePrinter = "enable-remote-printer";
const String kOptionEnableClipboard = "enable-clipboard";
const String kOptionEnableFileTransfer = "enable-file-transfer";
const String kOptionEnableAudio = "enable-audio";
const String kOptionEnableCamera = "enable-camera";
const String kOptionEnableTerminal = "enable-terminal";
const String kOptionTerminalPersistent = "terminal-persistent";
const String kOptionEnableTunnel = "enable-tunnel";
const String kOptionEnableRemoteRestart = "enable-remote-restart";
const String kOptionEnableBlockInput = "enable-block-input";
const String kOptionAllowRemoteConfigModification =
    "allow-remote-config-modification";
const String kOptionVerificationMethod = "verification-method";
const String kOptionApproveMode = "approve-mode";
const String kOptionAllowNumericOneTimePassword =
    "allow-numeric-one-time-password";
const String kOptionCollapseToolbar = "collapse_toolbar";
const String kOptionShowRemoteCursor = "show_remote_cursor";
const String kOptionFollowRemoteCursor = "follow_remote_cursor";
const String kOptionFollowRemoteWindow = "follow_remote_window";
const String kOptionZoomCursor = "zoom-cursor";
const String kOptionShowQualityMonitor = "show_quality_monitor";
const String kOptionDisableAudio = "disable_audio";
const String kOptionEnableFileCopyPaste = "enable-file-copy-paste";
// "Settings -> Display -> Other default options"
const String kOptionDisableClipboard = "disable_clipboard";
const String kOptionLockAfterSessionEnd = "lock_after_session_end";
const String kOptionPrivacyMode = "privacy_mode";
const String kOptionTouchMode = "touch-mode";
const String kOptionI444 = "i444";
const String kOptionSwapLeftRightMouse = "swap-left-right-mouse";
const String kOptionCodecPreference = "codec-preference";
const String kOptionRemoteMenubarDragLeft = "remote-menubar-drag-left";
const String kOptionRemoteMenubarDragRight = "remote-menubar-drag-right";
const String kOptionHideAbTagsPanel = "hideAbTagsPanel";
const String kOptionRemoteMenubarState = "remoteMenubarState";
const String kOptionPeerSorting = "peer-sorting";
const String kOptionPeerTabIndex = "peer-tab-index";
const String kOptionPeerTabOrder = "peer-tab-order";
const String kOptionPeerTabVisible = "peer-tab-visible";
const String kOptionPeerCardUiType = "peer-card-ui-type";
const String kOptionCurrentAbName = "current-ab-name";
const String kOptionEnableConfirmClosingTabs = "enable-confirm-closing-tabs";
const String kOptionAllowAlwaysSoftwareRender = "allow-always-software-render";
const String kOptionEnableCheckUpdate = "enable-check-update";
const String kOptionAllowAutoUpdate = "allow-auto-update";
const String kOptionAllowLinuxHeadless = "allow-linux-headless";
const String kOptionAllowRemoveWallpaper = "allow-remove-wallpaper";
const String kOptionStopService = "stop-service";
const String kOptionDirectxCapture = "enable-directx-capture";
const String kOptionAllowRemoteCmModification = "allow-remote-cm-modification";
const String kOptionEnableUdpPunch = "enable-udp-punch";
const String kOptionEnableIpv6Punch = "enable-ipv6-punch";
const String kOptionEnableTrustedDevices = "enable-trusted-devices";

// network options
const String kOptionAllowWebSocket = "allow-websocket";

// buildin opitons
const String kOptionHideServerSetting = "hide-server-settings";
const String kOptionHideProxySetting = "hide-proxy-settings";
const String kOptionHideWebSocketSetting = "hide-websocket-settings";
const String kOptionHideRemotePrinterSetting = "hide-remote-printer-settings";
const String kOptionHideSecuritySetting = "hide-security-settings";
const String kOptionHideNetworkSetting = "hide-network-settings";
const String kOptionRemovePresetPasswordWarning =
    "remove-preset-password-warning";
const kHideUsernameOnCard = "hide-username-on-card";
const String kOptionHideHelpCards = "hide-help-cards";

const String kOptionToggleViewOnly = "view-only";
const String kOptionToggleShowMyCursor = "show-my-cursor";

const String kOptionDisableFloatingWindow = "disable-floating-window";

const String kOptionKeepScreenOn = "keep-screen-on";

const String kOptionShowMobileAction = "showMobileActions";

const String kUrlActionClose = "close";

const String kTabLabelHomePage = "Home";
const String kTabLabelSettingPage = "Settings";

const String kWindowPrefix = "wm_";
const int kWindowMainId = 0;

const String kPointerEventKindTouch = "touch";
const String kPointerEventKindMouse = "mouse";

const String kMouseEventTypeDefault = "";
const String kMouseEventTypePanStart = "pan_start";
const String kMouseEventTypePanUpdate = "pan_update";
const String kMouseEventTypePanEnd = "pan_end";
const String kMouseEventTypeDown = "down";
const String kMouseEventTypeUp = "up";

const String kKeyFlutterKey = "flutter_key";

const String kKeyShowDisplaysAsIndividualWindows =
    'displays_as_individual_windows';
const String kKeyUseAllMyDisplaysForTheRemoteSession =
    'use_all_my_displays_for_the_remote_session';
const String kKeyShowMonitorsToolbar = 'show_monitors_toolbar';
const String kKeyReverseMouseWheel = "reverse_mouse_wheel";

const String kMsgboxTextWaitingForImage = 'Connected, waiting for image...';

// the executable name of the portable version
const String kEnvPortableExecutable = "RUSTDESK_APPNAME";

const Color kColorWarn = Color.fromARGB(255, 245, 133, 59);
const Color kColorCanvas = Colors.black;

const int kMobileDefaultDisplayWidth = 720;
const int kMobileDefaultDisplayHeight = 1280;

const int kDesktopDefaultDisplayWidth = 1080;
const int kDesktopDefaultDisplayHeight = 720;

const int kMobileMaxDisplaySize = 1280;
const int kDesktopMaxDisplaySize = 3840;

const double kDesktopFileTransferRowHeight = 30.0;
const double kDesktopFileTransferHeaderHeight = 25.0;

const double kMinFps = 5;
const double kDefaultFps = 30;
const double kMaxFps = 120;

const double kMinQuality = 10;
const double kDefaultQuality = 50;
const double kMaxQuality = 100;
const double kMaxMoreQuality = 2000;

// trackpad speed
const String kKeyTrackpadSpeed = 'trackpad-speed';
const int kMinTrackpadSpeed = 10;
const int kDefaultTrackpadSpeed = 100;
const int kMaxTrackpadSpeed = 1000;

// incomming (should be incoming) is kept, because change it will break the previous setting.
const String kKeyPrinterIncomingJobAction = 'printer-incomming-job-action';
const String kValuePrinterIncomingJobDismiss = 'dismiss';
const String kValuePrinterIncomingJobDefault = '';
const String kValuePrinterIncomingJobSelected = 'selected';
const String kKeyPrinterSelected = 'printer-selected-name';
const String kKeyPrinterSave = 'allow-printer-dialog-save';
const String kKeyPrinterAllowAutoPrint = 'allow-printer-auto-print';

double kNewWindowOffset = isWindows
    ? 56.0
    : isLinux
        ? 50.0
        : isMacOS
            ? 30.0
            : 50.0;

EdgeInsets get kDragToResizeAreaPadding => !kUseCompatibleUiMode && isLinux
    ? stateGlobal.fullscreen.isTrue || stateGlobal.isMaximized.value
        ? EdgeInsets.zero
        : EdgeInsets.all(5.0)
    : EdgeInsets.zero;
// https://en.wikipedia.org/wiki/Non-breaking_space
const int $nbsp = 0x00A0;

extension StringExtension on String {
  String get nonBreaking => replaceAll(' ', String.fromCharCode($nbsp));
}

const Size kConnectionManagerWindowSizeClosedChat = Size(300, 490);
const Size kConnectionManagerWindowSizeOpenChat = Size(700, 490);
// Tabbar transition duration, now we remove the duration
const Duration kTabTransitionDuration = Duration.zero;
const double kEmptyMarginTop = 50;
const double kDesktopIconButtonSplashRadius = 20;

/// [kMinCursorSize] indicates min cursor (w, h)
const int kMinCursorSize = 12;

const kFullScreenEdgeSize = 0.0;
const kMaximizeEdgeSize = 0.0;
// Do not use kWindowResizeEdgeSize directly. Use `windowResizeEdgeSize` in `common.dart` instead.
const kWindowResizeEdgeSize = 5.0;
final kWindowBorderWidth = isWindows ? 0.0 : 1.0;
const kDesktopMenuPadding = EdgeInsets.only(left: 12.0, right: 3.0);
const kFrameBorderRadius = 12.0;
const kFrameClipRRectBorderRadius = 12.0;
const kFrameBoxShadowBlurRadius = 32.0;
const kFrameBoxShadowOffsetFocused = 4.0;
const kFrameBoxShadowOffsetUnfocused = 2.0;

const kInvalidValueStr = 'InvalidValueStr';

// Config key shared by flutter and other ui.
const kCommConfKeyTheme = 'theme';
const kCommConfKeyLang = 'lang';

const kMobilePageConstraints = BoxConstraints(maxWidth: 600);

/// [kMouseControlDistance] indicates the distance that self-side move to get control of mouse.
const kMouseControlDistance = 12;

/// [kMouseControlTimeoutMSec] indicates the timeout (in milliseconds) that self-side can get control of mouse.
const kMouseControlTimeoutMSec = 1000;

/// [kRemoteViewStyleOriginal] Show remote image without scaling.
const kRemoteViewStyleOriginal = 'original';

/// [kRemoteViewStyleAdaptive] Show remote image scaling by ratio factor.
const kRemoteViewStyleAdaptive = 'adaptive';

/// [kRemoteScrollStyleAuto] Scroll image auto by position.
const kRemoteScrollStyleAuto = 'scrollauto';

/// [kRemoteScrollStyleBar] Scroll image with scroll bar.
const kRemoteScrollStyleBar = 'scrollbar';

/// [kScrollModeDefault] Mouse or touchpad, the default scroll mode.
const kScrollModeDefault = 'default';

/// [kScrollModeReverse] Mouse or touchpad, the reverse scroll mode.
const kScrollModeReverse = 'reverse';

/// [kRemoteImageQualityBest] Best image quality.
const kRemoteImageQualityBest = 'best';

/// [kRemoteImageQualityBalanced] Balanced image quality, mid performance.
const kRemoteImageQualityBalanced = 'balanced';

/// [kRemoteImageQualityLow] Low image quality, better performance.
const kRemoteImageQualityLow = 'low';

/// [kRemoteImageQualityCustom] Custom image quality.
const kRemoteImageQualityCustom = 'custom';

const kIgnoreDpi = true;

const Set<PointerDeviceKind> kTouchBasedDeviceKinds = {
  PointerDeviceKind.touch,
  PointerDeviceKind.stylus,
  PointerDeviceKind.invertedStylus,
};

// ================================ mobile ================================

// Magic numbers, maybe need to avoid it or use a better way to get them.
const kMobileDelaySoftKeyboard = Duration(milliseconds: 30);
const kMobileDelaySoftKeyboardFocus = Duration(milliseconds: 30);

/// Android constants
const kActionApplicationDetailsSettings =
    "android.settings.APPLICATION_DETAILS_SETTINGS";
const kActionAccessibilitySettings = "android.settings.ACCESSIBILITY_SETTINGS";

const kRecordAudio = "android.permission.RECORD_AUDIO";
const kManageExternalStorage = "android.permission.MANAGE_EXTERNAL_STORAGE";
const kRequestIgnoreBatteryOptimizations =
    "android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS";
const kSystemAlertWindow = "android.permission.SYSTEM_ALERT_WINDOW";
const kAndroid13Notification = "android.permission.POST_NOTIFICATIONS";

/// Android channel invoke type key
class AndroidChannel {
  static final kStartAction = "start_action";
  static final kGetStartOnBootOpt = "get_start_on_boot_opt";
  static final kSetStartOnBootOpt = "set_start_on_boot_opt";
  static final kSyncAppDirConfigPath = "sync_app_dir";
}

/// flutter/packages/flutter/lib/src/services/keyboard_key.dart -> _keyLabels
/// see [LogicalKeyboardKey.keyLabel]
const Map<int, String> logicalKeyMap = <int, String>{
  0x00000000020: 'VK_SPACE',
  0x00000000022: 'VK_QUOTE',
  0x0000000002c: 'VK_COMMA',
  0x0000000002d: 'VK_MINUS',
  0x0000000002f: 'VK_SLASH',
  0x00000000030: 'VK_0',
  0x00000000031: 'VK_1',
  0x00000000032: 'VK_2',
  0x00000000033: 'VK_3',
  0x00000000034: 'VK_4',
  0x00000000035: 'VK_5',
  0x00000000036: 'VK_6',
  0x00000000037: 'VK_7',
  0x00000000038: 'VK_8',
  0x00000000039: 'VK_9',
  0x0000000003b: 'VK_SEMICOLON',
  0x0000000003d: 'VK_PLUS', // it is =
  0x0000000005b: 'VK_LBRACKET',
  0x0000000005c: 'VK_BACKSLASH',
  0x0000000005d: 'VK_RBRACKET',
  0x00000000061: 'VK_A',
  0x00000000062: 'VK_B',
  0x00000000063: 'VK_C',
  0x00000000064: 'VK_D',
  0x00000000065: 'VK_E',
  0x00000000066: 'VK_F',
  0x00000000067: 'VK_G',
  0x00000000068: 'VK_H',
  0x00000000069: 'VK_I',
  0x0000000006a: 'VK_J',
  0x0000000006b: 'VK_K',
  0x0000000006c: 'VK_L',
  0x0000000006d: 'VK_M',
  0x0000000006e: 'VK_N',
  0x0000000006f: 'VK_O',
  0x00000000070: 'VK_P',
  0x00000000071: 'VK_Q',
  0x00000000072: 'VK_R',
  0x00000000073: 'VK_S',
  0x00000000074: 'VK_T',
  0x00000000075: 'VK_U',
  0x00000000076: 'VK_V',
  0x00000000077: 'VK_W',
  0x00000000078: 'VK_X',
  0x00000000079: 'VK_Y',
  0x0000000007a: 'VK_Z',
  0x00100000008: 'VK_BACK',
  0x00100000009: 'VK_TAB',
  0x0010000000d: 'VK_ENTER',
  0x0010000001b: 'VK_ESCAPE',
  0x0010000007f: 'VK_DELETE',
  0x00100000104: 'VK_CAPITAL',
  0x00100000301: 'VK_DOWN',
  0x00100000302: 'VK_LEFT',
  0x00100000303: 'VK_RIGHT',
  0x00100000304: 'VK_UP',
  0x00100000305: 'VK_END',
  0x00100000306: 'VK_HOME',
  0x00100000307: 'VK_NEXT',
  0x00100000308: 'VK_PRIOR',
  0x00100000401: 'VK_CLEAR',
  0x00100000407: 'VK_INSERT',
  0x00100000504: 'VK_CANCEL',
  0x00100000506: 'VK_EXECUTE',
  0x00100000508: 'VK_HELP',
  0x00100000509: 'VK_PAUSE',
  0x0010000050c: 'VK_SELECT',
  0x00100000608: 'VK_PRINT',
  0x00100000705: 'VK_CONVERT',
  0x00100000706: 'VK_FINAL',
  0x00100000711: 'VK_HANGUL',
  0x00100000712: 'VK_HANJA',
  0x00100000713: 'VK_JUNJA',
  0x00100000718: 'VK_KANA',
  0x00100000719: 'VK_KANJI',
  0x00100000801: 'VK_F1',
  0x00100000802: 'VK_F2',
  0x00100000803: 'VK_F3',
  0x00100000804: 'VK_F4',
  0x00100000805: 'VK_F5',
  0x00100000806: 'VK_F6',
  0x00100000807: 'VK_F7',
  0x00100000808: 'VK_F8',
  0x00100000809: 'VK_F9',
  0x0010000080a: 'VK_F10',
  0x0010000080b: 'VK_F11',
  0x0010000080c: 'VK_F12',
  0x00100000d2b: 'Apps',
  0x00200000002: 'VK_SLEEP',
  0x00200000100: 'VK_CONTROL',
  0x00200000101: 'RControl',
  0x00200000102: 'VK_SHIFT',
  0x00200000103: 'RShift',
  0x00200000104: 'VK_MENU',
  0x00200000105: 'RAlt',
  0x002000001f0: 'VK_CONTROL',
  0x002000001f2: 'VK_SHIFT',
  0x002000001f4: 'VK_MENU',
  0x002000001f6: 'Meta',
  0x0020000022a: 'VK_MULTIPLY',
  0x0020000022b: 'VK_ADD',
  0x0020000022d: 'VK_SUBTRACT',
  0x0020000022e: 'VK_DECIMAL',
  0x0020000022f: 'VK_DIVIDE',
  0x00200000230: 'VK_NUMPAD0',
  0x00200000231: 'VK_NUMPAD1',
  0x00200000232: 'VK_NUMPAD2',
  0x00200000233: 'VK_NUMPAD3',
  0x00200000234: 'VK_NUMPAD4',
  0x00200000235: 'VK_NUMPAD5',
  0x00200000236: 'VK_NUMPAD6',
  0x00200000237: 'VK_NUMPAD7',
  0x00200000238: 'VK_NUMPAD8',
  0x00200000239: 'VK_NUMPAD9',
};

/// flutter/packages/flutter/lib/src/services/keyboard_key.dart -> _debugName
/// see [PhysicalKeyboardKey.debugName] -> _debugName
const Map<int, String> physicalKeyMap = <int, String>{
  0x00010082: 'VK_SLEEP',
  0x00070004: 'VK_A',
  0x00070005: 'VK_B',
  0x00070006: 'VK_C',
  0x00070007: 'VK_D',
  0x00070008: 'VK_E',
  0x00070009: 'VK_F',
  0x0007000a: 'VK_G',
  0x0007000b: 'VK_H',
  0x0007000c: 'VK_I',
  0x0007000d: 'VK_J',
  0x0007000e: 'VK_K',
  0x0007000f: 'VK_L',
  0x00070010: 'VK_M',
  0x00070011: 'VK_N',
  0x00070012: 'VK_O',
  0x00070013: 'VK_P',
  0x00070014: 'VK_Q',
  0x00070015: 'VK_R',
  0x00070016: 'VK_S',
  0x00070017: 'VK_T',
  0x00070018: 'VK_U',
  0x00070019: 'VK_V',
  0x0007001a: 'VK_W',
  0x0007001b: 'VK_X',
  0x0007001c: 'VK_Y',
  0x0007001d: 'VK_Z',
  0x0007001e: 'VK_1',
  0x0007001f: 'VK_2',
  0x00070020: 'VK_3',
  0x00070021: 'VK_4',
  0x00070022: 'VK_5',
  0x00070023: 'VK_6',
  0x00070024: 'VK_7',
  0x00070025: 'VK_8',
  0x00070026: 'VK_9',
  0x00070027: 'VK_0',
  0x00070028: 'VK_ENTER',
  0x00070029: 'VK_ESCAPE',
  0x0007002a: 'VK_BACK',
  0x0007002b: 'VK_TAB',
  0x0007002c: 'VK_SPACE',
  0x0007002d: 'VK_MINUS',
  0x0007002e: 'VK_PLUS', // it is =
  0x0007002f: 'VK_LBRACKET',
  0x00070030: 'VK_RBRACKET',
  0x00070033: 'VK_SEMICOLON',
  0x00070034: 'VK_QUOTE',
  0x00070036: 'VK_COMMA',
  0x00070038: 'VK_SLASH',
  0x00070039: 'VK_CAPITAL',
  0x0007003a: 'VK_F1',
  0x0007003b: 'VK_F2',
  0x0007003c: 'VK_F3',
  0x0007003d: 'VK_F4',
  0x0007003e: 'VK_F5',
  0x0007003f: 'VK_F6',
  0x00070040: 'VK_F7',
  0x00070041: 'VK_F8',
  0x00070042: 'VK_F9',
  0x00070043: 'VK_F10',
  0x00070044: 'VK_F11',
  0x00070045: 'VK_F12',
  0x00070049: 'VK_INSERT',
  0x0007004a: 'VK_HOME',
  0x0007004b: 'VK_PRIOR', // Page Up
  0x0007004c: 'VK_DELETE',
  0x0007004d: 'VK_END',
  0x0007004e: 'VK_NEXT', // Page Down
  0x0007004f: 'VK_RIGHT',
  0x00070050: 'VK_LEFT',
  0x00070051: 'VK_DOWN',
  0x00070052: 'VK_UP',
  0x00070053: 'Num Lock', // TODO rust not impl
  0x00070054: 'VK_DIVIDE', // numpad
  0x00070055: 'VK_MULTIPLY',
  0x00070056: 'VK_SUBTRACT',
  0x00070057: 'VK_ADD',
  0x00070058: 'VK_ENTER', // num enter
  0x00070059: 'VK_NUMPAD1',
  0x0007005a: 'VK_NUMPAD2',
  0x0007005b: 'VK_NUMPAD3',
  0x0007005c: 'VK_NUMPAD4',
  0x0007005d: 'VK_NUMPAD5',
  0x0007005e: 'VK_NUMPAD6',
  0x0007005f: 'VK_NUMPAD7',
  0x00070060: 'VK_NUMPAD8',
  0x00070061: 'VK_NUMPAD9',
  0x00070062: 'VK_NUMPAD0',
  0x00070063: 'VK_DECIMAL',
  0x00070075: 'VK_HELP',
  0x00070077: 'VK_SELECT',
  0x00070088: 'VK_KANA',
  0x0007008a: 'VK_CONVERT',
  0x000700e0: 'VK_CONTROL',
  0x000700e1: 'VK_SHIFT',
  0x000700e2: 'VK_MENU',
  0x000700e3: 'Meta',
  0x000700e4: 'RControl',
  0x000700e5: 'RShift',
  0x000700e6: 'RAlt',
  0x000700e7: 'RWin',
  0x000c00b1: 'VK_PAUSE',
  0x000c00cd: 'VK_PAUSE',
  0x000c019e: 'LOCK_SCREEN',
  0x000c0208: 'VK_PRINT',
};

/// The windows targets in the publish time order.
enum WindowsTarget {
  naw, // not a windows target
  xp,
  vista,
  w7,
  w8,
  w8_1,
  w10,
  w11
}

/// A convenient method to transform a build number to the corresponding windows version.
extension WindowsTargetExt on int {
  WindowsTarget get windowsVersion => getWindowsTarget(this);
}

const kCheckSoftwareUpdateFinish = 'check_software_update_finish';
