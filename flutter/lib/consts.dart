import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_hbb/common.dart';

const double kDesktopRemoteTabBarHeight = 28.0;
const int kMainWindowId = 0;

const String kPeerPlatformWindows = "Windows";
const String kPeerPlatformLinux = "Linux";
const String kPeerPlatformMacOS = "Mac OS";
const String kPeerPlatformAndroid = "Android";

/// [kAppTypeMain] used by 'Desktop Main Page' , 'Mobile (Client and Server)', "Install Page"
const String kAppTypeMain = "main";
const String kAppTypeConnectionManager = "cm";
const String kAppTypeDesktopRemote = "remote";
const String kAppTypeDesktopFileTransfer = "file transfer";
const String kAppTypeDesktopPortForward = "port forward";

const String kWindowMainWindowOnTop = "main_window_on_top";
const String kWindowGetWindowInfo = "get_window_info";
const String kWindowActionRebuild = "rebuild";
const String kWindowEventHide = "hide";
const String kWindowEventShow = "show";
const String kWindowConnect = "connect";

const String kUniLinksPrefix = "rustdesk://";

const String kTabLabelHomePage = "Home";
const String kTabLabelSettingPage = "Settings";

const String kWindowPrefix = "wm_";
const int kWindowMainId = 0;

// the executable name of the portable version
const String kEnvPortableExecutable = "RUSTDESK_APPNAME";

const Color kColorWarn = Color.fromARGB(255, 245, 133, 59);

const int kMobileDefaultDisplayWidth = 720;
const int kMobileDefaultDisplayHeight = 1280;

const int kDesktopDefaultDisplayWidth = 1080;
const int kDesktopDefaultDisplayHeight = 720;

const int kMobileMaxDisplayWidth = 720;
const int kMobileMaxDisplayHeight = 1280;

const int kDesktopMaxDisplayWidth = 1920;
const int kDesktopMaxDisplayHeight = 1080;

const Size kConnectionManagerWindowSize = Size(300, 400);
// Tabbar transition duration, now we remove the duration
const Duration kTabTransitionDuration = Duration.zero;
const double kEmptyMarginTop = 50;
const double kDesktopIconButtonSplashRadius = 20;

/// [kMinCursorSize] indicates min cursor (w, h)
const int kMinCursorSize = 12;

/// [kDefaultScrollAmountMultiplier] indicates how many rows can be scrolled after a minimum scroll action of mouse
const kDefaultScrollAmountMultiplier = 5.0;
const kDefaultScrollDuration = Duration(milliseconds: 50);
const kDefaultMouseWheelThrottleDuration = Duration(milliseconds: 50);
const kFullScreenEdgeSize = 0.0;
var kWindowEdgeSize = Platform.isWindows ? 1.0 : 5.0;
const kWindowBorderWidth = 1.0;
const kDesktopMenuPadding = EdgeInsets.only(left: 12.0, right: 3.0);

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

/// [kRemoteImageQualityBest] Best image quality.
const kRemoteImageQualityBest = 'best';

/// [kRemoteImageQualityBalanced] Balanced image quality, mid performance.
const kRemoteImageQualityBalanced = 'balanced';

/// [kRemoteImageQualityLow] Low image quality, better performance.
const kRemoteImageQualityLow = 'low';

/// [kRemoteImageQualityCustom] Custom image quality.
const kRemoteImageQualityCustom = 'custom';

const kIgnoreDpi = true;

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
