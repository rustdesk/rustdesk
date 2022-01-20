/* eslint-disable */
import Long from "long";
import _m0 from "protobufjs/minimal";

export const protobufPackage = "hbb";

export enum ControlKey {
  Unknown = 0,
  Alt = 1,
  Backspace = 2,
  CapsLock = 3,
  Control = 4,
  Delete = 5,
  DownArrow = 6,
  End = 7,
  Escape = 8,
  F1 = 9,
  F10 = 10,
  F11 = 11,
  F12 = 12,
  F2 = 13,
  F3 = 14,
  F4 = 15,
  F5 = 16,
  F6 = 17,
  F7 = 18,
  F8 = 19,
  F9 = 20,
  Home = 21,
  LeftArrow = 22,
  /** Meta - / meta key (also known as "windows"; "super"; and "command") */
  Meta = 23,
  /** Option - / option key on macOS (alt key on Linux and Windows) */
  Option = 24,
  PageDown = 25,
  PageUp = 26,
  Return = 27,
  RightArrow = 28,
  Shift = 29,
  Space = 30,
  Tab = 31,
  UpArrow = 32,
  Numpad0 = 33,
  Numpad1 = 34,
  Numpad2 = 35,
  Numpad3 = 36,
  Numpad4 = 37,
  Numpad5 = 38,
  Numpad6 = 39,
  Numpad7 = 40,
  Numpad8 = 41,
  Numpad9 = 42,
  Cancel = 43,
  Clear = 44,
  /** Menu - deprecated, use Alt instead */
  Menu = 45,
  Pause = 46,
  Kana = 47,
  Hangul = 48,
  Junja = 49,
  Final = 50,
  Hanja = 51,
  Kanji = 52,
  Convert = 53,
  Select = 54,
  Print = 55,
  Execute = 56,
  Snapshot = 57,
  Insert = 58,
  Help = 59,
  Sleep = 60,
  Separator = 61,
  Scroll = 62,
  NumLock = 63,
  RWin = 64,
  Apps = 65,
  Multiply = 66,
  Add = 67,
  Subtract = 68,
  Decimal = 69,
  Divide = 70,
  Equals = 71,
  NumpadEnter = 72,
  RShift = 73,
  RControl = 74,
  RAlt = 75,
  CtrlAltDel = 100,
  LockScreen = 101,
  UNRECOGNIZED = -1,
}

export function controlKeyFromJSON(object: any): ControlKey {
  switch (object) {
    case 0:
    case "Unknown":
      return ControlKey.Unknown;
    case 1:
    case "Alt":
      return ControlKey.Alt;
    case 2:
    case "Backspace":
      return ControlKey.Backspace;
    case 3:
    case "CapsLock":
      return ControlKey.CapsLock;
    case 4:
    case "Control":
      return ControlKey.Control;
    case 5:
    case "Delete":
      return ControlKey.Delete;
    case 6:
    case "DownArrow":
      return ControlKey.DownArrow;
    case 7:
    case "End":
      return ControlKey.End;
    case 8:
    case "Escape":
      return ControlKey.Escape;
    case 9:
    case "F1":
      return ControlKey.F1;
    case 10:
    case "F10":
      return ControlKey.F10;
    case 11:
    case "F11":
      return ControlKey.F11;
    case 12:
    case "F12":
      return ControlKey.F12;
    case 13:
    case "F2":
      return ControlKey.F2;
    case 14:
    case "F3":
      return ControlKey.F3;
    case 15:
    case "F4":
      return ControlKey.F4;
    case 16:
    case "F5":
      return ControlKey.F5;
    case 17:
    case "F6":
      return ControlKey.F6;
    case 18:
    case "F7":
      return ControlKey.F7;
    case 19:
    case "F8":
      return ControlKey.F8;
    case 20:
    case "F9":
      return ControlKey.F9;
    case 21:
    case "Home":
      return ControlKey.Home;
    case 22:
    case "LeftArrow":
      return ControlKey.LeftArrow;
    case 23:
    case "Meta":
      return ControlKey.Meta;
    case 24:
    case "Option":
      return ControlKey.Option;
    case 25:
    case "PageDown":
      return ControlKey.PageDown;
    case 26:
    case "PageUp":
      return ControlKey.PageUp;
    case 27:
    case "Return":
      return ControlKey.Return;
    case 28:
    case "RightArrow":
      return ControlKey.RightArrow;
    case 29:
    case "Shift":
      return ControlKey.Shift;
    case 30:
    case "Space":
      return ControlKey.Space;
    case 31:
    case "Tab":
      return ControlKey.Tab;
    case 32:
    case "UpArrow":
      return ControlKey.UpArrow;
    case 33:
    case "Numpad0":
      return ControlKey.Numpad0;
    case 34:
    case "Numpad1":
      return ControlKey.Numpad1;
    case 35:
    case "Numpad2":
      return ControlKey.Numpad2;
    case 36:
    case "Numpad3":
      return ControlKey.Numpad3;
    case 37:
    case "Numpad4":
      return ControlKey.Numpad4;
    case 38:
    case "Numpad5":
      return ControlKey.Numpad5;
    case 39:
    case "Numpad6":
      return ControlKey.Numpad6;
    case 40:
    case "Numpad7":
      return ControlKey.Numpad7;
    case 41:
    case "Numpad8":
      return ControlKey.Numpad8;
    case 42:
    case "Numpad9":
      return ControlKey.Numpad9;
    case 43:
    case "Cancel":
      return ControlKey.Cancel;
    case 44:
    case "Clear":
      return ControlKey.Clear;
    case 45:
    case "Menu":
      return ControlKey.Menu;
    case 46:
    case "Pause":
      return ControlKey.Pause;
    case 47:
    case "Kana":
      return ControlKey.Kana;
    case 48:
    case "Hangul":
      return ControlKey.Hangul;
    case 49:
    case "Junja":
      return ControlKey.Junja;
    case 50:
    case "Final":
      return ControlKey.Final;
    case 51:
    case "Hanja":
      return ControlKey.Hanja;
    case 52:
    case "Kanji":
      return ControlKey.Kanji;
    case 53:
    case "Convert":
      return ControlKey.Convert;
    case 54:
    case "Select":
      return ControlKey.Select;
    case 55:
    case "Print":
      return ControlKey.Print;
    case 56:
    case "Execute":
      return ControlKey.Execute;
    case 57:
    case "Snapshot":
      return ControlKey.Snapshot;
    case 58:
    case "Insert":
      return ControlKey.Insert;
    case 59:
    case "Help":
      return ControlKey.Help;
    case 60:
    case "Sleep":
      return ControlKey.Sleep;
    case 61:
    case "Separator":
      return ControlKey.Separator;
    case 62:
    case "Scroll":
      return ControlKey.Scroll;
    case 63:
    case "NumLock":
      return ControlKey.NumLock;
    case 64:
    case "RWin":
      return ControlKey.RWin;
    case 65:
    case "Apps":
      return ControlKey.Apps;
    case 66:
    case "Multiply":
      return ControlKey.Multiply;
    case 67:
    case "Add":
      return ControlKey.Add;
    case 68:
    case "Subtract":
      return ControlKey.Subtract;
    case 69:
    case "Decimal":
      return ControlKey.Decimal;
    case 70:
    case "Divide":
      return ControlKey.Divide;
    case 71:
    case "Equals":
      return ControlKey.Equals;
    case 72:
    case "NumpadEnter":
      return ControlKey.NumpadEnter;
    case 73:
    case "RShift":
      return ControlKey.RShift;
    case 74:
    case "RControl":
      return ControlKey.RControl;
    case 75:
    case "RAlt":
      return ControlKey.RAlt;
    case 100:
    case "CtrlAltDel":
      return ControlKey.CtrlAltDel;
    case 101:
    case "LockScreen":
      return ControlKey.LockScreen;
    case -1:
    case "UNRECOGNIZED":
    default:
      return ControlKey.UNRECOGNIZED;
  }
}

export function controlKeyToJSON(object: ControlKey): string {
  switch (object) {
    case ControlKey.Unknown:
      return "Unknown";
    case ControlKey.Alt:
      return "Alt";
    case ControlKey.Backspace:
      return "Backspace";
    case ControlKey.CapsLock:
      return "CapsLock";
    case ControlKey.Control:
      return "Control";
    case ControlKey.Delete:
      return "Delete";
    case ControlKey.DownArrow:
      return "DownArrow";
    case ControlKey.End:
      return "End";
    case ControlKey.Escape:
      return "Escape";
    case ControlKey.F1:
      return "F1";
    case ControlKey.F10:
      return "F10";
    case ControlKey.F11:
      return "F11";
    case ControlKey.F12:
      return "F12";
    case ControlKey.F2:
      return "F2";
    case ControlKey.F3:
      return "F3";
    case ControlKey.F4:
      return "F4";
    case ControlKey.F5:
      return "F5";
    case ControlKey.F6:
      return "F6";
    case ControlKey.F7:
      return "F7";
    case ControlKey.F8:
      return "F8";
    case ControlKey.F9:
      return "F9";
    case ControlKey.Home:
      return "Home";
    case ControlKey.LeftArrow:
      return "LeftArrow";
    case ControlKey.Meta:
      return "Meta";
    case ControlKey.Option:
      return "Option";
    case ControlKey.PageDown:
      return "PageDown";
    case ControlKey.PageUp:
      return "PageUp";
    case ControlKey.Return:
      return "Return";
    case ControlKey.RightArrow:
      return "RightArrow";
    case ControlKey.Shift:
      return "Shift";
    case ControlKey.Space:
      return "Space";
    case ControlKey.Tab:
      return "Tab";
    case ControlKey.UpArrow:
      return "UpArrow";
    case ControlKey.Numpad0:
      return "Numpad0";
    case ControlKey.Numpad1:
      return "Numpad1";
    case ControlKey.Numpad2:
      return "Numpad2";
    case ControlKey.Numpad3:
      return "Numpad3";
    case ControlKey.Numpad4:
      return "Numpad4";
    case ControlKey.Numpad5:
      return "Numpad5";
    case ControlKey.Numpad6:
      return "Numpad6";
    case ControlKey.Numpad7:
      return "Numpad7";
    case ControlKey.Numpad8:
      return "Numpad8";
    case ControlKey.Numpad9:
      return "Numpad9";
    case ControlKey.Cancel:
      return "Cancel";
    case ControlKey.Clear:
      return "Clear";
    case ControlKey.Menu:
      return "Menu";
    case ControlKey.Pause:
      return "Pause";
    case ControlKey.Kana:
      return "Kana";
    case ControlKey.Hangul:
      return "Hangul";
    case ControlKey.Junja:
      return "Junja";
    case ControlKey.Final:
      return "Final";
    case ControlKey.Hanja:
      return "Hanja";
    case ControlKey.Kanji:
      return "Kanji";
    case ControlKey.Convert:
      return "Convert";
    case ControlKey.Select:
      return "Select";
    case ControlKey.Print:
      return "Print";
    case ControlKey.Execute:
      return "Execute";
    case ControlKey.Snapshot:
      return "Snapshot";
    case ControlKey.Insert:
      return "Insert";
    case ControlKey.Help:
      return "Help";
    case ControlKey.Sleep:
      return "Sleep";
    case ControlKey.Separator:
      return "Separator";
    case ControlKey.Scroll:
      return "Scroll";
    case ControlKey.NumLock:
      return "NumLock";
    case ControlKey.RWin:
      return "RWin";
    case ControlKey.Apps:
      return "Apps";
    case ControlKey.Multiply:
      return "Multiply";
    case ControlKey.Add:
      return "Add";
    case ControlKey.Subtract:
      return "Subtract";
    case ControlKey.Decimal:
      return "Decimal";
    case ControlKey.Divide:
      return "Divide";
    case ControlKey.Equals:
      return "Equals";
    case ControlKey.NumpadEnter:
      return "NumpadEnter";
    case ControlKey.RShift:
      return "RShift";
    case ControlKey.RControl:
      return "RControl";
    case ControlKey.RAlt:
      return "RAlt";
    case ControlKey.CtrlAltDel:
      return "CtrlAltDel";
    case ControlKey.LockScreen:
      return "LockScreen";
    default:
      return "UNKNOWN";
  }
}

export enum FileType {
  UnknownFileType = 0,
  Dir = 1,
  DirLink = 2,
  DirDrive = 3,
  File = 4,
  FileLink = 5,
  UNRECOGNIZED = -1,
}

export function fileTypeFromJSON(object: any): FileType {
  switch (object) {
    case 0:
    case "UnknownFileType":
      return FileType.UnknownFileType;
    case 1:
    case "Dir":
      return FileType.Dir;
    case 2:
    case "DirLink":
      return FileType.DirLink;
    case 3:
    case "DirDrive":
      return FileType.DirDrive;
    case 4:
    case "File":
      return FileType.File;
    case 5:
    case "FileLink":
      return FileType.FileLink;
    case -1:
    case "UNRECOGNIZED":
    default:
      return FileType.UNRECOGNIZED;
  }
}

export function fileTypeToJSON(object: FileType): string {
  switch (object) {
    case FileType.UnknownFileType:
      return "UnknownFileType";
    case FileType.Dir:
      return "Dir";
    case FileType.DirLink:
      return "DirLink";
    case FileType.DirDrive:
      return "DirDrive";
    case FileType.File:
      return "File";
    case FileType.FileLink:
      return "FileLink";
    default:
      return "UNKNOWN";
  }
}

export enum ImageQuality {
  NotSet = 0,
  Low = 2,
  Balanced = 3,
  Best = 4,
  UNRECOGNIZED = -1,
}

export function imageQualityFromJSON(object: any): ImageQuality {
  switch (object) {
    case 0:
    case "NotSet":
      return ImageQuality.NotSet;
    case 2:
    case "Low":
      return ImageQuality.Low;
    case 3:
    case "Balanced":
      return ImageQuality.Balanced;
    case 4:
    case "Best":
      return ImageQuality.Best;
    case -1:
    case "UNRECOGNIZED":
    default:
      return ImageQuality.UNRECOGNIZED;
  }
}

export function imageQualityToJSON(object: ImageQuality): string {
  switch (object) {
    case ImageQuality.NotSet:
      return "NotSet";
    case ImageQuality.Low:
      return "Low";
    case ImageQuality.Balanced:
      return "Balanced";
    case ImageQuality.Best:
      return "Best";
    default:
      return "UNKNOWN";
  }
}

export interface VP9 {
  data: Uint8Array;
  key: boolean;
  pts: number;
}

export interface VP9s {
  frames: VP9[];
}

export interface RGB {
  compress: boolean;
}

/** planes data send directly in binary for better use arraybuffer on web */
export interface YUV {
  compress: boolean;
  stride: number;
}

export interface VideoFrame {
  vp9s: VP9s | undefined;
  rgb: RGB | undefined;
  yuv: YUV | undefined;
}

export interface DisplayInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  name: string;
  online: boolean;
}

export interface PortForward {
  host: string;
  port: number;
}

export interface FileTransfer {
  dir: string;
  showHidden: boolean;
}

export interface LoginRequest {
  username: string;
  password: Uint8Array;
  myId: string;
  myName: string;
  option: OptionMessage | undefined;
  fileTransfer: FileTransfer | undefined;
  portForward: PortForward | undefined;
}

export interface ChatMessage {
  text: string;
}

export interface PeerInfo {
  username: string;
  hostname: string;
  platform: string;
  displays: DisplayInfo[];
  currentDisplay: number;
  sasEnabled: boolean;
  version: string;
}

export interface LoginResponse {
  error: string | undefined;
  peerInfo: PeerInfo | undefined;
}

export interface MouseEvent {
  mask: number;
  x: number;
  y: number;
  modifiers: ControlKey[];
}

export interface KeyEvent {
  down: boolean;
  press: boolean;
  controlKey: ControlKey | undefined;
  chr: number | undefined;
  unicode: number | undefined;
  seq: string | undefined;
  modifiers: ControlKey[];
}

export interface CursorData {
  id: number;
  hotx: number;
  hoty: number;
  width: number;
  height: number;
  colors: Uint8Array;
}

export interface CursorPosition {
  x: number;
  y: number;
}

export interface Hash {
  salt: string;
  challenge: string;
}

export interface Clipboard {
  compress: boolean;
  content: Uint8Array;
}

export interface FileEntry {
  entryType: FileType;
  name: string;
  isHidden: boolean;
  size: number;
  modifiedTime: number;
}

export interface FileDirectory {
  id: number;
  path: string;
  entries: FileEntry[];
}

export interface ReadDir {
  path: string;
  includeHidden: boolean;
}

export interface ReadAllFiles {
  id: number;
  path: string;
  includeHidden: boolean;
}

export interface FileAction {
  readDir: ReadDir | undefined;
  send: FileTransferSendRequest | undefined;
  receive: FileTransferReceiveRequest | undefined;
  create: FileDirCreate | undefined;
  removeDir: FileRemoveDir | undefined;
  removeFile: FileRemoveFile | undefined;
  allFiles: ReadAllFiles | undefined;
  cancel: FileTransferCancel | undefined;
}

export interface FileTransferCancel {
  id: number;
}

export interface FileResponse {
  dir: FileDirectory | undefined;
  block: FileTransferBlock | undefined;
  error: FileTransferError | undefined;
  done: FileTransferDone | undefined;
}

export interface FileTransferBlock {
  id: number;
  fileNum: number;
  data: Uint8Array;
  compressed: boolean;
}

export interface FileTransferError {
  id: number;
  error: string;
  fileNum: number;
}

export interface FileTransferSendRequest {
  id: number;
  path: string;
  includeHidden: boolean;
}

export interface FileTransferDone {
  id: number;
  fileNum: number;
}

export interface FileTransferReceiveRequest {
  id: number;
  /** path written to */
  path: string;
  files: FileEntry[];
}

export interface FileRemoveDir {
  id: number;
  path: string;
  recursive: boolean;
}

export interface FileRemoveFile {
  id: number;
  path: string;
  fileNum: number;
}

export interface FileDirCreate {
  id: number;
  path: string;
}

export interface SwitchDisplay {
  display: number;
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PermissionInfo {
  permission: PermissionInfo_Permission;
  enabled: boolean;
}

export enum PermissionInfo_Permission {
  Unknown = 0,
  Keyboard = 1,
  Clipboard = 2,
  Audio = 3,
  UNRECOGNIZED = -1,
}

export function permissionInfo_PermissionFromJSON(
  object: any
): PermissionInfo_Permission {
  switch (object) {
    case 0:
    case "Unknown":
      return PermissionInfo_Permission.Unknown;
    case 1:
    case "Keyboard":
      return PermissionInfo_Permission.Keyboard;
    case 2:
    case "Clipboard":
      return PermissionInfo_Permission.Clipboard;
    case 3:
    case "Audio":
      return PermissionInfo_Permission.Audio;
    case -1:
    case "UNRECOGNIZED":
    default:
      return PermissionInfo_Permission.UNRECOGNIZED;
  }
}

export function permissionInfo_PermissionToJSON(
  object: PermissionInfo_Permission
): string {
  switch (object) {
    case PermissionInfo_Permission.Unknown:
      return "Unknown";
    case PermissionInfo_Permission.Keyboard:
      return "Keyboard";
    case PermissionInfo_Permission.Clipboard:
      return "Clipboard";
    case PermissionInfo_Permission.Audio:
      return "Audio";
    default:
      return "UNKNOWN";
  }
}

export interface OptionMessage {
  imageQuality: ImageQuality;
  lockAfterSessionEnd: OptionMessage_BoolOption;
  showRemoteCursor: OptionMessage_BoolOption;
  privacyMode: OptionMessage_BoolOption;
  blockInput: OptionMessage_BoolOption;
  customImageQuality: number;
  disableAudio: OptionMessage_BoolOption;
  disableClipboard: OptionMessage_BoolOption;
}

export enum OptionMessage_BoolOption {
  NotSet = 0,
  No = 1,
  Yes = 2,
  UNRECOGNIZED = -1,
}

export function optionMessage_BoolOptionFromJSON(
  object: any
): OptionMessage_BoolOption {
  switch (object) {
    case 0:
    case "NotSet":
      return OptionMessage_BoolOption.NotSet;
    case 1:
    case "No":
      return OptionMessage_BoolOption.No;
    case 2:
    case "Yes":
      return OptionMessage_BoolOption.Yes;
    case -1:
    case "UNRECOGNIZED":
    default:
      return OptionMessage_BoolOption.UNRECOGNIZED;
  }
}

export function optionMessage_BoolOptionToJSON(
  object: OptionMessage_BoolOption
): string {
  switch (object) {
    case OptionMessage_BoolOption.NotSet:
      return "NotSet";
    case OptionMessage_BoolOption.No:
      return "No";
    case OptionMessage_BoolOption.Yes:
      return "Yes";
    default:
      return "UNKNOWN";
  }
}

export interface OptionResponse {
  opt: OptionMessage | undefined;
  error: string;
}

export interface TestDelay {
  time: number;
  fromClient: boolean;
}

export interface PublicKey {
  asymmetricValue: Uint8Array;
  symmetricValue: Uint8Array;
}

export interface SignedId {
  id: Uint8Array;
}

export interface AudioFormat {
  sampleRate: number;
  channels: number;
}

export interface AudioFrame {
  data: Uint8Array;
}

export interface Misc {
  chatMessage: ChatMessage | undefined;
  switchDisplay: SwitchDisplay | undefined;
  permissionInfo: PermissionInfo | undefined;
  option: OptionMessage | undefined;
  audioFormat: AudioFormat | undefined;
  closeReason: string | undefined;
  refreshVideo: boolean | undefined;
  optionResponse: OptionResponse | undefined;
}

export interface Message {
  signedId: SignedId | undefined;
  publicKey: PublicKey | undefined;
  testDelay: TestDelay | undefined;
  videoFrame: VideoFrame | undefined;
  loginRequest: LoginRequest | undefined;
  loginResponse: LoginResponse | undefined;
  hash: Hash | undefined;
  mouseEvent: MouseEvent | undefined;
  audioFrame: AudioFrame | undefined;
  cursorData: CursorData | undefined;
  cursorPosition: CursorPosition | undefined;
  cursorId: number | undefined;
  keyEvent: KeyEvent | undefined;
  clipboard: Clipboard | undefined;
  fileAction: FileAction | undefined;
  fileResponse: FileResponse | undefined;
  misc: Misc | undefined;
}

function createBaseVP9(): VP9 {
  return { data: new Uint8Array(), key: false, pts: 0 };
}

export const VP9 = {
  encode(message: VP9, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.data.length !== 0) {
      writer.uint32(10).bytes(message.data);
    }
    if (message.key === true) {
      writer.uint32(16).bool(message.key);
    }
    if (message.pts !== 0) {
      writer.uint32(24).int64(message.pts);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): VP9 {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseVP9();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.data = reader.bytes();
          break;
        case 2:
          message.key = reader.bool();
          break;
        case 3:
          message.pts = longToNumber(reader.int64() as Long);
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): VP9 {
    return {
      data: isSet(object.data)
        ? bytesFromBase64(object.data)
        : new Uint8Array(),
      key: isSet(object.key) ? Boolean(object.key) : false,
      pts: isSet(object.pts) ? Number(object.pts) : 0,
    };
  },

  toJSON(message: VP9): unknown {
    const obj: any = {};
    message.data !== undefined &&
      (obj.data = base64FromBytes(
        message.data !== undefined ? message.data : new Uint8Array()
      ));
    message.key !== undefined && (obj.key = message.key);
    message.pts !== undefined && (obj.pts = Math.round(message.pts));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<VP9>, I>>(object: I): VP9 {
    const message = createBaseVP9();
    message.data = object.data ?? new Uint8Array();
    message.key = object.key ?? false;
    message.pts = object.pts ?? 0;
    return message;
  },
};

function createBaseVP9s(): VP9s {
  return { frames: [] };
}

export const VP9s = {
  encode(message: VP9s, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    for (const v of message.frames) {
      VP9.encode(v!, writer.uint32(10).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): VP9s {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseVP9s();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.frames.push(VP9.decode(reader, reader.uint32()));
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): VP9s {
    return {
      frames: Array.isArray(object?.frames)
        ? object.frames.map((e: any) => VP9.fromJSON(e))
        : [],
    };
  },

  toJSON(message: VP9s): unknown {
    const obj: any = {};
    if (message.frames) {
      obj.frames = message.frames.map((e) => (e ? VP9.toJSON(e) : undefined));
    } else {
      obj.frames = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<VP9s>, I>>(object: I): VP9s {
    const message = createBaseVP9s();
    message.frames = object.frames?.map((e) => VP9.fromPartial(e)) || [];
    return message;
  },
};

function createBaseRGB(): RGB {
  return { compress: false };
}

export const RGB = {
  encode(message: RGB, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.compress === true) {
      writer.uint32(8).bool(message.compress);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): RGB {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseRGB();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.compress = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): RGB {
    return {
      compress: isSet(object.compress) ? Boolean(object.compress) : false,
    };
  },

  toJSON(message: RGB): unknown {
    const obj: any = {};
    message.compress !== undefined && (obj.compress = message.compress);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<RGB>, I>>(object: I): RGB {
    const message = createBaseRGB();
    message.compress = object.compress ?? false;
    return message;
  },
};

function createBaseYUV(): YUV {
  return { compress: false, stride: 0 };
}

export const YUV = {
  encode(message: YUV, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.compress === true) {
      writer.uint32(8).bool(message.compress);
    }
    if (message.stride !== 0) {
      writer.uint32(16).int32(message.stride);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): YUV {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseYUV();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.compress = reader.bool();
          break;
        case 2:
          message.stride = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): YUV {
    return {
      compress: isSet(object.compress) ? Boolean(object.compress) : false,
      stride: isSet(object.stride) ? Number(object.stride) : 0,
    };
  },

  toJSON(message: YUV): unknown {
    const obj: any = {};
    message.compress !== undefined && (obj.compress = message.compress);
    message.stride !== undefined && (obj.stride = Math.round(message.stride));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<YUV>, I>>(object: I): YUV {
    const message = createBaseYUV();
    message.compress = object.compress ?? false;
    message.stride = object.stride ?? 0;
    return message;
  },
};

function createBaseVideoFrame(): VideoFrame {
  return { vp9s: undefined, rgb: undefined, yuv: undefined };
}

export const VideoFrame = {
  encode(
    message: VideoFrame,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.vp9s !== undefined) {
      VP9s.encode(message.vp9s, writer.uint32(50).fork()).ldelim();
    }
    if (message.rgb !== undefined) {
      RGB.encode(message.rgb, writer.uint32(58).fork()).ldelim();
    }
    if (message.yuv !== undefined) {
      YUV.encode(message.yuv, writer.uint32(66).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): VideoFrame {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseVideoFrame();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 6:
          message.vp9s = VP9s.decode(reader, reader.uint32());
          break;
        case 7:
          message.rgb = RGB.decode(reader, reader.uint32());
          break;
        case 8:
          message.yuv = YUV.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): VideoFrame {
    return {
      vp9s: isSet(object.vp9s) ? VP9s.fromJSON(object.vp9s) : undefined,
      rgb: isSet(object.rgb) ? RGB.fromJSON(object.rgb) : undefined,
      yuv: isSet(object.yuv) ? YUV.fromJSON(object.yuv) : undefined,
    };
  },

  toJSON(message: VideoFrame): unknown {
    const obj: any = {};
    message.vp9s !== undefined &&
      (obj.vp9s = message.vp9s ? VP9s.toJSON(message.vp9s) : undefined);
    message.rgb !== undefined &&
      (obj.rgb = message.rgb ? RGB.toJSON(message.rgb) : undefined);
    message.yuv !== undefined &&
      (obj.yuv = message.yuv ? YUV.toJSON(message.yuv) : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<VideoFrame>, I>>(
    object: I
  ): VideoFrame {
    const message = createBaseVideoFrame();
    message.vp9s =
      object.vp9s !== undefined && object.vp9s !== null
        ? VP9s.fromPartial(object.vp9s)
        : undefined;
    message.rgb =
      object.rgb !== undefined && object.rgb !== null
        ? RGB.fromPartial(object.rgb)
        : undefined;
    message.yuv =
      object.yuv !== undefined && object.yuv !== null
        ? YUV.fromPartial(object.yuv)
        : undefined;
    return message;
  },
};

function createBaseDisplayInfo(): DisplayInfo {
  return { x: 0, y: 0, width: 0, height: 0, name: "", online: false };
}

export const DisplayInfo = {
  encode(
    message: DisplayInfo,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.x !== 0) {
      writer.uint32(8).sint32(message.x);
    }
    if (message.y !== 0) {
      writer.uint32(16).sint32(message.y);
    }
    if (message.width !== 0) {
      writer.uint32(24).int32(message.width);
    }
    if (message.height !== 0) {
      writer.uint32(32).int32(message.height);
    }
    if (message.name !== "") {
      writer.uint32(42).string(message.name);
    }
    if (message.online === true) {
      writer.uint32(48).bool(message.online);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): DisplayInfo {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseDisplayInfo();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.x = reader.sint32();
          break;
        case 2:
          message.y = reader.sint32();
          break;
        case 3:
          message.width = reader.int32();
          break;
        case 4:
          message.height = reader.int32();
          break;
        case 5:
          message.name = reader.string();
          break;
        case 6:
          message.online = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): DisplayInfo {
    return {
      x: isSet(object.x) ? Number(object.x) : 0,
      y: isSet(object.y) ? Number(object.y) : 0,
      width: isSet(object.width) ? Number(object.width) : 0,
      height: isSet(object.height) ? Number(object.height) : 0,
      name: isSet(object.name) ? String(object.name) : "",
      online: isSet(object.online) ? Boolean(object.online) : false,
    };
  },

  toJSON(message: DisplayInfo): unknown {
    const obj: any = {};
    message.x !== undefined && (obj.x = Math.round(message.x));
    message.y !== undefined && (obj.y = Math.round(message.y));
    message.width !== undefined && (obj.width = Math.round(message.width));
    message.height !== undefined && (obj.height = Math.round(message.height));
    message.name !== undefined && (obj.name = message.name);
    message.online !== undefined && (obj.online = message.online);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<DisplayInfo>, I>>(
    object: I
  ): DisplayInfo {
    const message = createBaseDisplayInfo();
    message.x = object.x ?? 0;
    message.y = object.y ?? 0;
    message.width = object.width ?? 0;
    message.height = object.height ?? 0;
    message.name = object.name ?? "";
    message.online = object.online ?? false;
    return message;
  },
};

function createBasePortForward(): PortForward {
  return { host: "", port: 0 };
}

export const PortForward = {
  encode(
    message: PortForward,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.host !== "") {
      writer.uint32(10).string(message.host);
    }
    if (message.port !== 0) {
      writer.uint32(16).int32(message.port);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PortForward {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePortForward();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.host = reader.string();
          break;
        case 2:
          message.port = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PortForward {
    return {
      host: isSet(object.host) ? String(object.host) : "",
      port: isSet(object.port) ? Number(object.port) : 0,
    };
  },

  toJSON(message: PortForward): unknown {
    const obj: any = {};
    message.host !== undefined && (obj.host = message.host);
    message.port !== undefined && (obj.port = Math.round(message.port));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PortForward>, I>>(
    object: I
  ): PortForward {
    const message = createBasePortForward();
    message.host = object.host ?? "";
    message.port = object.port ?? 0;
    return message;
  },
};

function createBaseFileTransfer(): FileTransfer {
  return { dir: "", showHidden: false };
}

export const FileTransfer = {
  encode(
    message: FileTransfer,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.dir !== "") {
      writer.uint32(10).string(message.dir);
    }
    if (message.showHidden === true) {
      writer.uint32(16).bool(message.showHidden);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileTransfer {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransfer();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.dir = reader.string();
          break;
        case 2:
          message.showHidden = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransfer {
    return {
      dir: isSet(object.dir) ? String(object.dir) : "",
      showHidden: isSet(object.showHidden) ? Boolean(object.showHidden) : false,
    };
  },

  toJSON(message: FileTransfer): unknown {
    const obj: any = {};
    message.dir !== undefined && (obj.dir = message.dir);
    message.showHidden !== undefined && (obj.showHidden = message.showHidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransfer>, I>>(
    object: I
  ): FileTransfer {
    const message = createBaseFileTransfer();
    message.dir = object.dir ?? "";
    message.showHidden = object.showHidden ?? false;
    return message;
  },
};

function createBaseLoginRequest(): LoginRequest {
  return {
    username: "",
    password: new Uint8Array(),
    myId: "",
    myName: "",
    option: undefined,
    fileTransfer: undefined,
    portForward: undefined,
  };
}

export const LoginRequest = {
  encode(
    message: LoginRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.username !== "") {
      writer.uint32(10).string(message.username);
    }
    if (message.password.length !== 0) {
      writer.uint32(18).bytes(message.password);
    }
    if (message.myId !== "") {
      writer.uint32(34).string(message.myId);
    }
    if (message.myName !== "") {
      writer.uint32(42).string(message.myName);
    }
    if (message.option !== undefined) {
      OptionMessage.encode(message.option, writer.uint32(50).fork()).ldelim();
    }
    if (message.fileTransfer !== undefined) {
      FileTransfer.encode(
        message.fileTransfer,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.portForward !== undefined) {
      PortForward.encode(
        message.portForward,
        writer.uint32(66).fork()
      ).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): LoginRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseLoginRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.username = reader.string();
          break;
        case 2:
          message.password = reader.bytes();
          break;
        case 4:
          message.myId = reader.string();
          break;
        case 5:
          message.myName = reader.string();
          break;
        case 6:
          message.option = OptionMessage.decode(reader, reader.uint32());
          break;
        case 7:
          message.fileTransfer = FileTransfer.decode(reader, reader.uint32());
          break;
        case 8:
          message.portForward = PortForward.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): LoginRequest {
    return {
      username: isSet(object.username) ? String(object.username) : "",
      password: isSet(object.password)
        ? bytesFromBase64(object.password)
        : new Uint8Array(),
      myId: isSet(object.myId) ? String(object.myId) : "",
      myName: isSet(object.myName) ? String(object.myName) : "",
      option: isSet(object.option)
        ? OptionMessage.fromJSON(object.option)
        : undefined,
      fileTransfer: isSet(object.fileTransfer)
        ? FileTransfer.fromJSON(object.fileTransfer)
        : undefined,
      portForward: isSet(object.portForward)
        ? PortForward.fromJSON(object.portForward)
        : undefined,
    };
  },

  toJSON(message: LoginRequest): unknown {
    const obj: any = {};
    message.username !== undefined && (obj.username = message.username);
    message.password !== undefined &&
      (obj.password = base64FromBytes(
        message.password !== undefined ? message.password : new Uint8Array()
      ));
    message.myId !== undefined && (obj.myId = message.myId);
    message.myName !== undefined && (obj.myName = message.myName);
    message.option !== undefined &&
      (obj.option = message.option
        ? OptionMessage.toJSON(message.option)
        : undefined);
    message.fileTransfer !== undefined &&
      (obj.fileTransfer = message.fileTransfer
        ? FileTransfer.toJSON(message.fileTransfer)
        : undefined);
    message.portForward !== undefined &&
      (obj.portForward = message.portForward
        ? PortForward.toJSON(message.portForward)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LoginRequest>, I>>(
    object: I
  ): LoginRequest {
    const message = createBaseLoginRequest();
    message.username = object.username ?? "";
    message.password = object.password ?? new Uint8Array();
    message.myId = object.myId ?? "";
    message.myName = object.myName ?? "";
    message.option =
      object.option !== undefined && object.option !== null
        ? OptionMessage.fromPartial(object.option)
        : undefined;
    message.fileTransfer =
      object.fileTransfer !== undefined && object.fileTransfer !== null
        ? FileTransfer.fromPartial(object.fileTransfer)
        : undefined;
    message.portForward =
      object.portForward !== undefined && object.portForward !== null
        ? PortForward.fromPartial(object.portForward)
        : undefined;
    return message;
  },
};

function createBaseChatMessage(): ChatMessage {
  return { text: "" };
}

export const ChatMessage = {
  encode(
    message: ChatMessage,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.text !== "") {
      writer.uint32(10).string(message.text);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): ChatMessage {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseChatMessage();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.text = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): ChatMessage {
    return {
      text: isSet(object.text) ? String(object.text) : "",
    };
  },

  toJSON(message: ChatMessage): unknown {
    const obj: any = {};
    message.text !== undefined && (obj.text = message.text);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ChatMessage>, I>>(
    object: I
  ): ChatMessage {
    const message = createBaseChatMessage();
    message.text = object.text ?? "";
    return message;
  },
};

function createBasePeerInfo(): PeerInfo {
  return {
    username: "",
    hostname: "",
    platform: "",
    displays: [],
    currentDisplay: 0,
    sasEnabled: false,
    version: "",
  };
}

export const PeerInfo = {
  encode(
    message: PeerInfo,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.username !== "") {
      writer.uint32(10).string(message.username);
    }
    if (message.hostname !== "") {
      writer.uint32(18).string(message.hostname);
    }
    if (message.platform !== "") {
      writer.uint32(26).string(message.platform);
    }
    for (const v of message.displays) {
      DisplayInfo.encode(v!, writer.uint32(34).fork()).ldelim();
    }
    if (message.currentDisplay !== 0) {
      writer.uint32(40).int32(message.currentDisplay);
    }
    if (message.sasEnabled === true) {
      writer.uint32(48).bool(message.sasEnabled);
    }
    if (message.version !== "") {
      writer.uint32(58).string(message.version);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PeerInfo {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePeerInfo();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.username = reader.string();
          break;
        case 2:
          message.hostname = reader.string();
          break;
        case 3:
          message.platform = reader.string();
          break;
        case 4:
          message.displays.push(DisplayInfo.decode(reader, reader.uint32()));
          break;
        case 5:
          message.currentDisplay = reader.int32();
          break;
        case 6:
          message.sasEnabled = reader.bool();
          break;
        case 7:
          message.version = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PeerInfo {
    return {
      username: isSet(object.username) ? String(object.username) : "",
      hostname: isSet(object.hostname) ? String(object.hostname) : "",
      platform: isSet(object.platform) ? String(object.platform) : "",
      displays: Array.isArray(object?.displays)
        ? object.displays.map((e: any) => DisplayInfo.fromJSON(e))
        : [],
      currentDisplay: isSet(object.currentDisplay)
        ? Number(object.currentDisplay)
        : 0,
      sasEnabled: isSet(object.sasEnabled) ? Boolean(object.sasEnabled) : false,
      version: isSet(object.version) ? String(object.version) : "",
    };
  },

  toJSON(message: PeerInfo): unknown {
    const obj: any = {};
    message.username !== undefined && (obj.username = message.username);
    message.hostname !== undefined && (obj.hostname = message.hostname);
    message.platform !== undefined && (obj.platform = message.platform);
    if (message.displays) {
      obj.displays = message.displays.map((e) =>
        e ? DisplayInfo.toJSON(e) : undefined
      );
    } else {
      obj.displays = [];
    }
    message.currentDisplay !== undefined &&
      (obj.currentDisplay = Math.round(message.currentDisplay));
    message.sasEnabled !== undefined && (obj.sasEnabled = message.sasEnabled);
    message.version !== undefined && (obj.version = message.version);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PeerInfo>, I>>(object: I): PeerInfo {
    const message = createBasePeerInfo();
    message.username = object.username ?? "";
    message.hostname = object.hostname ?? "";
    message.platform = object.platform ?? "";
    message.displays =
      object.displays?.map((e) => DisplayInfo.fromPartial(e)) || [];
    message.currentDisplay = object.currentDisplay ?? 0;
    message.sasEnabled = object.sasEnabled ?? false;
    message.version = object.version ?? "";
    return message;
  },
};

function createBaseLoginResponse(): LoginResponse {
  return { error: undefined, peerInfo: undefined };
}

export const LoginResponse = {
  encode(
    message: LoginResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.error !== undefined) {
      writer.uint32(10).string(message.error);
    }
    if (message.peerInfo !== undefined) {
      PeerInfo.encode(message.peerInfo, writer.uint32(18).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): LoginResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseLoginResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.error = reader.string();
          break;
        case 2:
          message.peerInfo = PeerInfo.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): LoginResponse {
    return {
      error: isSet(object.error) ? String(object.error) : undefined,
      peerInfo: isSet(object.peerInfo)
        ? PeerInfo.fromJSON(object.peerInfo)
        : undefined,
    };
  },

  toJSON(message: LoginResponse): unknown {
    const obj: any = {};
    message.error !== undefined && (obj.error = message.error);
    message.peerInfo !== undefined &&
      (obj.peerInfo = message.peerInfo
        ? PeerInfo.toJSON(message.peerInfo)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LoginResponse>, I>>(
    object: I
  ): LoginResponse {
    const message = createBaseLoginResponse();
    message.error = object.error ?? undefined;
    message.peerInfo =
      object.peerInfo !== undefined && object.peerInfo !== null
        ? PeerInfo.fromPartial(object.peerInfo)
        : undefined;
    return message;
  },
};

function createBaseMouseEvent(): MouseEvent {
  return { mask: 0, x: 0, y: 0, modifiers: [] };
}

export const MouseEvent = {
  encode(
    message: MouseEvent,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.mask !== 0) {
      writer.uint32(8).int32(message.mask);
    }
    if (message.x !== 0) {
      writer.uint32(16).sint32(message.x);
    }
    if (message.y !== 0) {
      writer.uint32(24).sint32(message.y);
    }
    writer.uint32(34).fork();
    for (const v of message.modifiers) {
      writer.int32(v);
    }
    writer.ldelim();
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): MouseEvent {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseMouseEvent();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.mask = reader.int32();
          break;
        case 2:
          message.x = reader.sint32();
          break;
        case 3:
          message.y = reader.sint32();
          break;
        case 4:
          if ((tag & 7) === 2) {
            const end2 = reader.uint32() + reader.pos;
            while (reader.pos < end2) {
              message.modifiers.push(reader.int32() as any);
            }
          } else {
            message.modifiers.push(reader.int32() as any);
          }
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): MouseEvent {
    return {
      mask: isSet(object.mask) ? Number(object.mask) : 0,
      x: isSet(object.x) ? Number(object.x) : 0,
      y: isSet(object.y) ? Number(object.y) : 0,
      modifiers: Array.isArray(object?.modifiers)
        ? object.modifiers.map((e: any) => controlKeyFromJSON(e))
        : [],
    };
  },

  toJSON(message: MouseEvent): unknown {
    const obj: any = {};
    message.mask !== undefined && (obj.mask = Math.round(message.mask));
    message.x !== undefined && (obj.x = Math.round(message.x));
    message.y !== undefined && (obj.y = Math.round(message.y));
    if (message.modifiers) {
      obj.modifiers = message.modifiers.map((e) => controlKeyToJSON(e));
    } else {
      obj.modifiers = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<MouseEvent>, I>>(
    object: I
  ): MouseEvent {
    const message = createBaseMouseEvent();
    message.mask = object.mask ?? 0;
    message.x = object.x ?? 0;
    message.y = object.y ?? 0;
    message.modifiers = object.modifiers?.map((e) => e) || [];
    return message;
  },
};

function createBaseKeyEvent(): KeyEvent {
  return {
    down: false,
    press: false,
    controlKey: undefined,
    chr: undefined,
    unicode: undefined,
    seq: undefined,
    modifiers: [],
  };
}

export const KeyEvent = {
  encode(
    message: KeyEvent,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.down === true) {
      writer.uint32(8).bool(message.down);
    }
    if (message.press === true) {
      writer.uint32(16).bool(message.press);
    }
    if (message.controlKey !== undefined) {
      writer.uint32(24).int32(message.controlKey);
    }
    if (message.chr !== undefined) {
      writer.uint32(32).uint32(message.chr);
    }
    if (message.unicode !== undefined) {
      writer.uint32(40).uint32(message.unicode);
    }
    if (message.seq !== undefined) {
      writer.uint32(50).string(message.seq);
    }
    writer.uint32(66).fork();
    for (const v of message.modifiers) {
      writer.int32(v);
    }
    writer.ldelim();
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): KeyEvent {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseKeyEvent();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.down = reader.bool();
          break;
        case 2:
          message.press = reader.bool();
          break;
        case 3:
          message.controlKey = reader.int32() as any;
          break;
        case 4:
          message.chr = reader.uint32();
          break;
        case 5:
          message.unicode = reader.uint32();
          break;
        case 6:
          message.seq = reader.string();
          break;
        case 8:
          if ((tag & 7) === 2) {
            const end2 = reader.uint32() + reader.pos;
            while (reader.pos < end2) {
              message.modifiers.push(reader.int32() as any);
            }
          } else {
            message.modifiers.push(reader.int32() as any);
          }
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): KeyEvent {
    return {
      down: isSet(object.down) ? Boolean(object.down) : false,
      press: isSet(object.press) ? Boolean(object.press) : false,
      controlKey: isSet(object.controlKey)
        ? controlKeyFromJSON(object.controlKey)
        : undefined,
      chr: isSet(object.chr) ? Number(object.chr) : undefined,
      unicode: isSet(object.unicode) ? Number(object.unicode) : undefined,
      seq: isSet(object.seq) ? String(object.seq) : undefined,
      modifiers: Array.isArray(object?.modifiers)
        ? object.modifiers.map((e: any) => controlKeyFromJSON(e))
        : [],
    };
  },

  toJSON(message: KeyEvent): unknown {
    const obj: any = {};
    message.down !== undefined && (obj.down = message.down);
    message.press !== undefined && (obj.press = message.press);
    message.controlKey !== undefined &&
      (obj.controlKey =
        message.controlKey !== undefined
          ? controlKeyToJSON(message.controlKey)
          : undefined);
    message.chr !== undefined && (obj.chr = Math.round(message.chr));
    message.unicode !== undefined &&
      (obj.unicode = Math.round(message.unicode));
    message.seq !== undefined && (obj.seq = message.seq);
    if (message.modifiers) {
      obj.modifiers = message.modifiers.map((e) => controlKeyToJSON(e));
    } else {
      obj.modifiers = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<KeyEvent>, I>>(object: I): KeyEvent {
    const message = createBaseKeyEvent();
    message.down = object.down ?? false;
    message.press = object.press ?? false;
    message.controlKey = object.controlKey ?? undefined;
    message.chr = object.chr ?? undefined;
    message.unicode = object.unicode ?? undefined;
    message.seq = object.seq ?? undefined;
    message.modifiers = object.modifiers?.map((e) => e) || [];
    return message;
  },
};

function createBaseCursorData(): CursorData {
  return {
    id: 0,
    hotx: 0,
    hoty: 0,
    width: 0,
    height: 0,
    colors: new Uint8Array(),
  };
}

export const CursorData = {
  encode(
    message: CursorData,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).uint64(message.id);
    }
    if (message.hotx !== 0) {
      writer.uint32(16).sint32(message.hotx);
    }
    if (message.hoty !== 0) {
      writer.uint32(24).sint32(message.hoty);
    }
    if (message.width !== 0) {
      writer.uint32(32).int32(message.width);
    }
    if (message.height !== 0) {
      writer.uint32(40).int32(message.height);
    }
    if (message.colors.length !== 0) {
      writer.uint32(50).bytes(message.colors);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): CursorData {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCursorData();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = longToNumber(reader.uint64() as Long);
          break;
        case 2:
          message.hotx = reader.sint32();
          break;
        case 3:
          message.hoty = reader.sint32();
          break;
        case 4:
          message.width = reader.int32();
          break;
        case 5:
          message.height = reader.int32();
          break;
        case 6:
          message.colors = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CursorData {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      hotx: isSet(object.hotx) ? Number(object.hotx) : 0,
      hoty: isSet(object.hoty) ? Number(object.hoty) : 0,
      width: isSet(object.width) ? Number(object.width) : 0,
      height: isSet(object.height) ? Number(object.height) : 0,
      colors: isSet(object.colors)
        ? bytesFromBase64(object.colors)
        : new Uint8Array(),
    };
  },

  toJSON(message: CursorData): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.hotx !== undefined && (obj.hotx = Math.round(message.hotx));
    message.hoty !== undefined && (obj.hoty = Math.round(message.hoty));
    message.width !== undefined && (obj.width = Math.round(message.width));
    message.height !== undefined && (obj.height = Math.round(message.height));
    message.colors !== undefined &&
      (obj.colors = base64FromBytes(
        message.colors !== undefined ? message.colors : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CursorData>, I>>(
    object: I
  ): CursorData {
    const message = createBaseCursorData();
    message.id = object.id ?? 0;
    message.hotx = object.hotx ?? 0;
    message.hoty = object.hoty ?? 0;
    message.width = object.width ?? 0;
    message.height = object.height ?? 0;
    message.colors = object.colors ?? new Uint8Array();
    return message;
  },
};

function createBaseCursorPosition(): CursorPosition {
  return { x: 0, y: 0 };
}

export const CursorPosition = {
  encode(
    message: CursorPosition,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.x !== 0) {
      writer.uint32(8).sint32(message.x);
    }
    if (message.y !== 0) {
      writer.uint32(16).sint32(message.y);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): CursorPosition {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCursorPosition();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.x = reader.sint32();
          break;
        case 2:
          message.y = reader.sint32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CursorPosition {
    return {
      x: isSet(object.x) ? Number(object.x) : 0,
      y: isSet(object.y) ? Number(object.y) : 0,
    };
  },

  toJSON(message: CursorPosition): unknown {
    const obj: any = {};
    message.x !== undefined && (obj.x = Math.round(message.x));
    message.y !== undefined && (obj.y = Math.round(message.y));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CursorPosition>, I>>(
    object: I
  ): CursorPosition {
    const message = createBaseCursorPosition();
    message.x = object.x ?? 0;
    message.y = object.y ?? 0;
    return message;
  },
};

function createBaseHash(): Hash {
  return { salt: "", challenge: "" };
}

export const Hash = {
  encode(message: Hash, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.salt !== "") {
      writer.uint32(10).string(message.salt);
    }
    if (message.challenge !== "") {
      writer.uint32(18).string(message.challenge);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Hash {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseHash();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.salt = reader.string();
          break;
        case 2:
          message.challenge = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): Hash {
    return {
      salt: isSet(object.salt) ? String(object.salt) : "",
      challenge: isSet(object.challenge) ? String(object.challenge) : "",
    };
  },

  toJSON(message: Hash): unknown {
    const obj: any = {};
    message.salt !== undefined && (obj.salt = message.salt);
    message.challenge !== undefined && (obj.challenge = message.challenge);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Hash>, I>>(object: I): Hash {
    const message = createBaseHash();
    message.salt = object.salt ?? "";
    message.challenge = object.challenge ?? "";
    return message;
  },
};

function createBaseClipboard(): Clipboard {
  return { compress: false, content: new Uint8Array() };
}

export const Clipboard = {
  encode(
    message: Clipboard,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.compress === true) {
      writer.uint32(8).bool(message.compress);
    }
    if (message.content.length !== 0) {
      writer.uint32(18).bytes(message.content);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Clipboard {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseClipboard();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.compress = reader.bool();
          break;
        case 2:
          message.content = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): Clipboard {
    return {
      compress: isSet(object.compress) ? Boolean(object.compress) : false,
      content: isSet(object.content)
        ? bytesFromBase64(object.content)
        : new Uint8Array(),
    };
  },

  toJSON(message: Clipboard): unknown {
    const obj: any = {};
    message.compress !== undefined && (obj.compress = message.compress);
    message.content !== undefined &&
      (obj.content = base64FromBytes(
        message.content !== undefined ? message.content : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Clipboard>, I>>(
    object: I
  ): Clipboard {
    const message = createBaseClipboard();
    message.compress = object.compress ?? false;
    message.content = object.content ?? new Uint8Array();
    return message;
  },
};

function createBaseFileEntry(): FileEntry {
  return { entryType: 0, name: "", isHidden: false, size: 0, modifiedTime: 0 };
}

export const FileEntry = {
  encode(
    message: FileEntry,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.entryType !== 0) {
      writer.uint32(8).int32(message.entryType);
    }
    if (message.name !== "") {
      writer.uint32(18).string(message.name);
    }
    if (message.isHidden === true) {
      writer.uint32(24).bool(message.isHidden);
    }
    if (message.size !== 0) {
      writer.uint32(32).uint64(message.size);
    }
    if (message.modifiedTime !== 0) {
      writer.uint32(40).uint64(message.modifiedTime);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileEntry {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileEntry();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.entryType = reader.int32() as any;
          break;
        case 2:
          message.name = reader.string();
          break;
        case 3:
          message.isHidden = reader.bool();
          break;
        case 4:
          message.size = longToNumber(reader.uint64() as Long);
          break;
        case 5:
          message.modifiedTime = longToNumber(reader.uint64() as Long);
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileEntry {
    return {
      entryType: isSet(object.entryType)
        ? fileTypeFromJSON(object.entryType)
        : 0,
      name: isSet(object.name) ? String(object.name) : "",
      isHidden: isSet(object.isHidden) ? Boolean(object.isHidden) : false,
      size: isSet(object.size) ? Number(object.size) : 0,
      modifiedTime: isSet(object.modifiedTime)
        ? Number(object.modifiedTime)
        : 0,
    };
  },

  toJSON(message: FileEntry): unknown {
    const obj: any = {};
    message.entryType !== undefined &&
      (obj.entryType = fileTypeToJSON(message.entryType));
    message.name !== undefined && (obj.name = message.name);
    message.isHidden !== undefined && (obj.isHidden = message.isHidden);
    message.size !== undefined && (obj.size = Math.round(message.size));
    message.modifiedTime !== undefined &&
      (obj.modifiedTime = Math.round(message.modifiedTime));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileEntry>, I>>(
    object: I
  ): FileEntry {
    const message = createBaseFileEntry();
    message.entryType = object.entryType ?? 0;
    message.name = object.name ?? "";
    message.isHidden = object.isHidden ?? false;
    message.size = object.size ?? 0;
    message.modifiedTime = object.modifiedTime ?? 0;
    return message;
  },
};

function createBaseFileDirectory(): FileDirectory {
  return { id: 0, path: "", entries: [] };
}

export const FileDirectory = {
  encode(
    message: FileDirectory,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    for (const v of message.entries) {
      FileEntry.encode(v!, writer.uint32(26).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileDirectory {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileDirectory();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.entries.push(FileEntry.decode(reader, reader.uint32()));
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileDirectory {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      entries: Array.isArray(object?.entries)
        ? object.entries.map((e: any) => FileEntry.fromJSON(e))
        : [],
    };
  },

  toJSON(message: FileDirectory): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    if (message.entries) {
      obj.entries = message.entries.map((e) =>
        e ? FileEntry.toJSON(e) : undefined
      );
    } else {
      obj.entries = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileDirectory>, I>>(
    object: I
  ): FileDirectory {
    const message = createBaseFileDirectory();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.entries =
      object.entries?.map((e) => FileEntry.fromPartial(e)) || [];
    return message;
  },
};

function createBaseReadDir(): ReadDir {
  return { path: "", includeHidden: false };
}

export const ReadDir = {
  encode(
    message: ReadDir,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.path !== "") {
      writer.uint32(10).string(message.path);
    }
    if (message.includeHidden === true) {
      writer.uint32(16).bool(message.includeHidden);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): ReadDir {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseReadDir();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.path = reader.string();
          break;
        case 2:
          message.includeHidden = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): ReadDir {
    return {
      path: isSet(object.path) ? String(object.path) : "",
      includeHidden: isSet(object.includeHidden)
        ? Boolean(object.includeHidden)
        : false,
    };
  },

  toJSON(message: ReadDir): unknown {
    const obj: any = {};
    message.path !== undefined && (obj.path = message.path);
    message.includeHidden !== undefined &&
      (obj.includeHidden = message.includeHidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ReadDir>, I>>(object: I): ReadDir {
    const message = createBaseReadDir();
    message.path = object.path ?? "";
    message.includeHidden = object.includeHidden ?? false;
    return message;
  },
};

function createBaseReadAllFiles(): ReadAllFiles {
  return { id: 0, path: "", includeHidden: false };
}

export const ReadAllFiles = {
  encode(
    message: ReadAllFiles,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    if (message.includeHidden === true) {
      writer.uint32(24).bool(message.includeHidden);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): ReadAllFiles {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseReadAllFiles();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.includeHidden = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): ReadAllFiles {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      includeHidden: isSet(object.includeHidden)
        ? Boolean(object.includeHidden)
        : false,
    };
  },

  toJSON(message: ReadAllFiles): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.includeHidden !== undefined &&
      (obj.includeHidden = message.includeHidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ReadAllFiles>, I>>(
    object: I
  ): ReadAllFiles {
    const message = createBaseReadAllFiles();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.includeHidden = object.includeHidden ?? false;
    return message;
  },
};

function createBaseFileAction(): FileAction {
  return {
    readDir: undefined,
    send: undefined,
    receive: undefined,
    create: undefined,
    removeDir: undefined,
    removeFile: undefined,
    allFiles: undefined,
    cancel: undefined,
  };
}

export const FileAction = {
  encode(
    message: FileAction,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.readDir !== undefined) {
      ReadDir.encode(message.readDir, writer.uint32(10).fork()).ldelim();
    }
    if (message.send !== undefined) {
      FileTransferSendRequest.encode(
        message.send,
        writer.uint32(18).fork()
      ).ldelim();
    }
    if (message.receive !== undefined) {
      FileTransferReceiveRequest.encode(
        message.receive,
        writer.uint32(26).fork()
      ).ldelim();
    }
    if (message.create !== undefined) {
      FileDirCreate.encode(message.create, writer.uint32(34).fork()).ldelim();
    }
    if (message.removeDir !== undefined) {
      FileRemoveDir.encode(
        message.removeDir,
        writer.uint32(42).fork()
      ).ldelim();
    }
    if (message.removeFile !== undefined) {
      FileRemoveFile.encode(
        message.removeFile,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.allFiles !== undefined) {
      ReadAllFiles.encode(message.allFiles, writer.uint32(58).fork()).ldelim();
    }
    if (message.cancel !== undefined) {
      FileTransferCancel.encode(
        message.cancel,
        writer.uint32(66).fork()
      ).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileAction {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileAction();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.readDir = ReadDir.decode(reader, reader.uint32());
          break;
        case 2:
          message.send = FileTransferSendRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 3:
          message.receive = FileTransferReceiveRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 4:
          message.create = FileDirCreate.decode(reader, reader.uint32());
          break;
        case 5:
          message.removeDir = FileRemoveDir.decode(reader, reader.uint32());
          break;
        case 6:
          message.removeFile = FileRemoveFile.decode(reader, reader.uint32());
          break;
        case 7:
          message.allFiles = ReadAllFiles.decode(reader, reader.uint32());
          break;
        case 8:
          message.cancel = FileTransferCancel.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileAction {
    return {
      readDir: isSet(object.readDir)
        ? ReadDir.fromJSON(object.readDir)
        : undefined,
      send: isSet(object.send)
        ? FileTransferSendRequest.fromJSON(object.send)
        : undefined,
      receive: isSet(object.receive)
        ? FileTransferReceiveRequest.fromJSON(object.receive)
        : undefined,
      create: isSet(object.create)
        ? FileDirCreate.fromJSON(object.create)
        : undefined,
      removeDir: isSet(object.removeDir)
        ? FileRemoveDir.fromJSON(object.removeDir)
        : undefined,
      removeFile: isSet(object.removeFile)
        ? FileRemoveFile.fromJSON(object.removeFile)
        : undefined,
      allFiles: isSet(object.allFiles)
        ? ReadAllFiles.fromJSON(object.allFiles)
        : undefined,
      cancel: isSet(object.cancel)
        ? FileTransferCancel.fromJSON(object.cancel)
        : undefined,
    };
  },

  toJSON(message: FileAction): unknown {
    const obj: any = {};
    message.readDir !== undefined &&
      (obj.readDir = message.readDir
        ? ReadDir.toJSON(message.readDir)
        : undefined);
    message.send !== undefined &&
      (obj.send = message.send
        ? FileTransferSendRequest.toJSON(message.send)
        : undefined);
    message.receive !== undefined &&
      (obj.receive = message.receive
        ? FileTransferReceiveRequest.toJSON(message.receive)
        : undefined);
    message.create !== undefined &&
      (obj.create = message.create
        ? FileDirCreate.toJSON(message.create)
        : undefined);
    message.removeDir !== undefined &&
      (obj.removeDir = message.removeDir
        ? FileRemoveDir.toJSON(message.removeDir)
        : undefined);
    message.removeFile !== undefined &&
      (obj.removeFile = message.removeFile
        ? FileRemoveFile.toJSON(message.removeFile)
        : undefined);
    message.allFiles !== undefined &&
      (obj.allFiles = message.allFiles
        ? ReadAllFiles.toJSON(message.allFiles)
        : undefined);
    message.cancel !== undefined &&
      (obj.cancel = message.cancel
        ? FileTransferCancel.toJSON(message.cancel)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileAction>, I>>(
    object: I
  ): FileAction {
    const message = createBaseFileAction();
    message.readDir =
      object.readDir !== undefined && object.readDir !== null
        ? ReadDir.fromPartial(object.readDir)
        : undefined;
    message.send =
      object.send !== undefined && object.send !== null
        ? FileTransferSendRequest.fromPartial(object.send)
        : undefined;
    message.receive =
      object.receive !== undefined && object.receive !== null
        ? FileTransferReceiveRequest.fromPartial(object.receive)
        : undefined;
    message.create =
      object.create !== undefined && object.create !== null
        ? FileDirCreate.fromPartial(object.create)
        : undefined;
    message.removeDir =
      object.removeDir !== undefined && object.removeDir !== null
        ? FileRemoveDir.fromPartial(object.removeDir)
        : undefined;
    message.removeFile =
      object.removeFile !== undefined && object.removeFile !== null
        ? FileRemoveFile.fromPartial(object.removeFile)
        : undefined;
    message.allFiles =
      object.allFiles !== undefined && object.allFiles !== null
        ? ReadAllFiles.fromPartial(object.allFiles)
        : undefined;
    message.cancel =
      object.cancel !== undefined && object.cancel !== null
        ? FileTransferCancel.fromPartial(object.cancel)
        : undefined;
    return message;
  },
};

function createBaseFileTransferCancel(): FileTransferCancel {
  return { id: 0 };
}

export const FileTransferCancel = {
  encode(
    message: FileTransferCancel,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileTransferCancel {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferCancel();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferCancel {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
    };
  },

  toJSON(message: FileTransferCancel): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferCancel>, I>>(
    object: I
  ): FileTransferCancel {
    const message = createBaseFileTransferCancel();
    message.id = object.id ?? 0;
    return message;
  },
};

function createBaseFileResponse(): FileResponse {
  return {
    dir: undefined,
    block: undefined,
    error: undefined,
    done: undefined,
  };
}

export const FileResponse = {
  encode(
    message: FileResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.dir !== undefined) {
      FileDirectory.encode(message.dir, writer.uint32(10).fork()).ldelim();
    }
    if (message.block !== undefined) {
      FileTransferBlock.encode(
        message.block,
        writer.uint32(18).fork()
      ).ldelim();
    }
    if (message.error !== undefined) {
      FileTransferError.encode(
        message.error,
        writer.uint32(26).fork()
      ).ldelim();
    }
    if (message.done !== undefined) {
      FileTransferDone.encode(message.done, writer.uint32(34).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.dir = FileDirectory.decode(reader, reader.uint32());
          break;
        case 2:
          message.block = FileTransferBlock.decode(reader, reader.uint32());
          break;
        case 3:
          message.error = FileTransferError.decode(reader, reader.uint32());
          break;
        case 4:
          message.done = FileTransferDone.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileResponse {
    return {
      dir: isSet(object.dir) ? FileDirectory.fromJSON(object.dir) : undefined,
      block: isSet(object.block)
        ? FileTransferBlock.fromJSON(object.block)
        : undefined,
      error: isSet(object.error)
        ? FileTransferError.fromJSON(object.error)
        : undefined,
      done: isSet(object.done)
        ? FileTransferDone.fromJSON(object.done)
        : undefined,
    };
  },

  toJSON(message: FileResponse): unknown {
    const obj: any = {};
    message.dir !== undefined &&
      (obj.dir = message.dir ? FileDirectory.toJSON(message.dir) : undefined);
    message.block !== undefined &&
      (obj.block = message.block
        ? FileTransferBlock.toJSON(message.block)
        : undefined);
    message.error !== undefined &&
      (obj.error = message.error
        ? FileTransferError.toJSON(message.error)
        : undefined);
    message.done !== undefined &&
      (obj.done = message.done
        ? FileTransferDone.toJSON(message.done)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileResponse>, I>>(
    object: I
  ): FileResponse {
    const message = createBaseFileResponse();
    message.dir =
      object.dir !== undefined && object.dir !== null
        ? FileDirectory.fromPartial(object.dir)
        : undefined;
    message.block =
      object.block !== undefined && object.block !== null
        ? FileTransferBlock.fromPartial(object.block)
        : undefined;
    message.error =
      object.error !== undefined && object.error !== null
        ? FileTransferError.fromPartial(object.error)
        : undefined;
    message.done =
      object.done !== undefined && object.done !== null
        ? FileTransferDone.fromPartial(object.done)
        : undefined;
    return message;
  },
};

function createBaseFileTransferBlock(): FileTransferBlock {
  return { id: 0, fileNum: 0, data: new Uint8Array(), compressed: false };
}

export const FileTransferBlock = {
  encode(
    message: FileTransferBlock,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.fileNum !== 0) {
      writer.uint32(16).sint32(message.fileNum);
    }
    if (message.data.length !== 0) {
      writer.uint32(26).bytes(message.data);
    }
    if (message.compressed === true) {
      writer.uint32(32).bool(message.compressed);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileTransferBlock {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferBlock();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.fileNum = reader.sint32();
          break;
        case 3:
          message.data = reader.bytes();
          break;
        case 4:
          message.compressed = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferBlock {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      fileNum: isSet(object.fileNum) ? Number(object.fileNum) : 0,
      data: isSet(object.data)
        ? bytesFromBase64(object.data)
        : new Uint8Array(),
      compressed: isSet(object.compressed) ? Boolean(object.compressed) : false,
    };
  },

  toJSON(message: FileTransferBlock): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.fileNum !== undefined &&
      (obj.fileNum = Math.round(message.fileNum));
    message.data !== undefined &&
      (obj.data = base64FromBytes(
        message.data !== undefined ? message.data : new Uint8Array()
      ));
    message.compressed !== undefined && (obj.compressed = message.compressed);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferBlock>, I>>(
    object: I
  ): FileTransferBlock {
    const message = createBaseFileTransferBlock();
    message.id = object.id ?? 0;
    message.fileNum = object.fileNum ?? 0;
    message.data = object.data ?? new Uint8Array();
    message.compressed = object.compressed ?? false;
    return message;
  },
};

function createBaseFileTransferError(): FileTransferError {
  return { id: 0, error: "", fileNum: 0 };
}

export const FileTransferError = {
  encode(
    message: FileTransferError,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.error !== "") {
      writer.uint32(18).string(message.error);
    }
    if (message.fileNum !== 0) {
      writer.uint32(24).sint32(message.fileNum);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileTransferError {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferError();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.error = reader.string();
          break;
        case 3:
          message.fileNum = reader.sint32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferError {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      error: isSet(object.error) ? String(object.error) : "",
      fileNum: isSet(object.fileNum) ? Number(object.fileNum) : 0,
    };
  },

  toJSON(message: FileTransferError): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.error !== undefined && (obj.error = message.error);
    message.fileNum !== undefined &&
      (obj.fileNum = Math.round(message.fileNum));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferError>, I>>(
    object: I
  ): FileTransferError {
    const message = createBaseFileTransferError();
    message.id = object.id ?? 0;
    message.error = object.error ?? "";
    message.fileNum = object.fileNum ?? 0;
    return message;
  },
};

function createBaseFileTransferSendRequest(): FileTransferSendRequest {
  return { id: 0, path: "", includeHidden: false };
}

export const FileTransferSendRequest = {
  encode(
    message: FileTransferSendRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    if (message.includeHidden === true) {
      writer.uint32(24).bool(message.includeHidden);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): FileTransferSendRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferSendRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.includeHidden = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferSendRequest {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      includeHidden: isSet(object.includeHidden)
        ? Boolean(object.includeHidden)
        : false,
    };
  },

  toJSON(message: FileTransferSendRequest): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.includeHidden !== undefined &&
      (obj.includeHidden = message.includeHidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferSendRequest>, I>>(
    object: I
  ): FileTransferSendRequest {
    const message = createBaseFileTransferSendRequest();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.includeHidden = object.includeHidden ?? false;
    return message;
  },
};

function createBaseFileTransferDone(): FileTransferDone {
  return { id: 0, fileNum: 0 };
}

export const FileTransferDone = {
  encode(
    message: FileTransferDone,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.fileNum !== 0) {
      writer.uint32(16).sint32(message.fileNum);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileTransferDone {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferDone();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.fileNum = reader.sint32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferDone {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      fileNum: isSet(object.fileNum) ? Number(object.fileNum) : 0,
    };
  },

  toJSON(message: FileTransferDone): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.fileNum !== undefined &&
      (obj.fileNum = Math.round(message.fileNum));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferDone>, I>>(
    object: I
  ): FileTransferDone {
    const message = createBaseFileTransferDone();
    message.id = object.id ?? 0;
    message.fileNum = object.fileNum ?? 0;
    return message;
  },
};

function createBaseFileTransferReceiveRequest(): FileTransferReceiveRequest {
  return { id: 0, path: "", files: [] };
}

export const FileTransferReceiveRequest = {
  encode(
    message: FileTransferReceiveRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    for (const v of message.files) {
      FileEntry.encode(v!, writer.uint32(26).fork()).ldelim();
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): FileTransferReceiveRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileTransferReceiveRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.files.push(FileEntry.decode(reader, reader.uint32()));
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileTransferReceiveRequest {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      files: Array.isArray(object?.files)
        ? object.files.map((e: any) => FileEntry.fromJSON(e))
        : [],
    };
  },

  toJSON(message: FileTransferReceiveRequest): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    if (message.files) {
      obj.files = message.files.map((e) =>
        e ? FileEntry.toJSON(e) : undefined
      );
    } else {
      obj.files = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferReceiveRequest>, I>>(
    object: I
  ): FileTransferReceiveRequest {
    const message = createBaseFileTransferReceiveRequest();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.files = object.files?.map((e) => FileEntry.fromPartial(e)) || [];
    return message;
  },
};

function createBaseFileRemoveDir(): FileRemoveDir {
  return { id: 0, path: "", recursive: false };
}

export const FileRemoveDir = {
  encode(
    message: FileRemoveDir,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    if (message.recursive === true) {
      writer.uint32(24).bool(message.recursive);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileRemoveDir {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileRemoveDir();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.recursive = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileRemoveDir {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      recursive: isSet(object.recursive) ? Boolean(object.recursive) : false,
    };
  },

  toJSON(message: FileRemoveDir): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.recursive !== undefined && (obj.recursive = message.recursive);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileRemoveDir>, I>>(
    object: I
  ): FileRemoveDir {
    const message = createBaseFileRemoveDir();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.recursive = object.recursive ?? false;
    return message;
  },
};

function createBaseFileRemoveFile(): FileRemoveFile {
  return { id: 0, path: "", fileNum: 0 };
}

export const FileRemoveFile = {
  encode(
    message: FileRemoveFile,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    if (message.fileNum !== 0) {
      writer.uint32(24).sint32(message.fileNum);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileRemoveFile {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileRemoveFile();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        case 3:
          message.fileNum = reader.sint32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileRemoveFile {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
      fileNum: isSet(object.fileNum) ? Number(object.fileNum) : 0,
    };
  },

  toJSON(message: FileRemoveFile): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.fileNum !== undefined &&
      (obj.fileNum = Math.round(message.fileNum));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileRemoveFile>, I>>(
    object: I
  ): FileRemoveFile {
    const message = createBaseFileRemoveFile();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.fileNum = object.fileNum ?? 0;
    return message;
  },
};

function createBaseFileDirCreate(): FileDirCreate {
  return { id: 0, path: "" };
}

export const FileDirCreate = {
  encode(
    message: FileDirCreate,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.path !== "") {
      writer.uint32(18).string(message.path);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): FileDirCreate {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseFileDirCreate();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.int32();
          break;
        case 2:
          message.path = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): FileDirCreate {
    return {
      id: isSet(object.id) ? Number(object.id) : 0,
      path: isSet(object.path) ? String(object.path) : "",
    };
  },

  toJSON(message: FileDirCreate): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileDirCreate>, I>>(
    object: I
  ): FileDirCreate {
    const message = createBaseFileDirCreate();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    return message;
  },
};

function createBaseSwitchDisplay(): SwitchDisplay {
  return { display: 0, x: 0, y: 0, width: 0, height: 0 };
}

export const SwitchDisplay = {
  encode(
    message: SwitchDisplay,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.display !== 0) {
      writer.uint32(8).int32(message.display);
    }
    if (message.x !== 0) {
      writer.uint32(16).sint32(message.x);
    }
    if (message.y !== 0) {
      writer.uint32(24).sint32(message.y);
    }
    if (message.width !== 0) {
      writer.uint32(32).int32(message.width);
    }
    if (message.height !== 0) {
      writer.uint32(40).int32(message.height);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): SwitchDisplay {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseSwitchDisplay();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.display = reader.int32();
          break;
        case 2:
          message.x = reader.sint32();
          break;
        case 3:
          message.y = reader.sint32();
          break;
        case 4:
          message.width = reader.int32();
          break;
        case 5:
          message.height = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): SwitchDisplay {
    return {
      display: isSet(object.display) ? Number(object.display) : 0,
      x: isSet(object.x) ? Number(object.x) : 0,
      y: isSet(object.y) ? Number(object.y) : 0,
      width: isSet(object.width) ? Number(object.width) : 0,
      height: isSet(object.height) ? Number(object.height) : 0,
    };
  },

  toJSON(message: SwitchDisplay): unknown {
    const obj: any = {};
    message.display !== undefined &&
      (obj.display = Math.round(message.display));
    message.x !== undefined && (obj.x = Math.round(message.x));
    message.y !== undefined && (obj.y = Math.round(message.y));
    message.width !== undefined && (obj.width = Math.round(message.width));
    message.height !== undefined && (obj.height = Math.round(message.height));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<SwitchDisplay>, I>>(
    object: I
  ): SwitchDisplay {
    const message = createBaseSwitchDisplay();
    message.display = object.display ?? 0;
    message.x = object.x ?? 0;
    message.y = object.y ?? 0;
    message.width = object.width ?? 0;
    message.height = object.height ?? 0;
    return message;
  },
};

function createBasePermissionInfo(): PermissionInfo {
  return { permission: 0, enabled: false };
}

export const PermissionInfo = {
  encode(
    message: PermissionInfo,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.permission !== 0) {
      writer.uint32(8).int32(message.permission);
    }
    if (message.enabled === true) {
      writer.uint32(16).bool(message.enabled);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PermissionInfo {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePermissionInfo();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.permission = reader.int32() as any;
          break;
        case 2:
          message.enabled = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PermissionInfo {
    return {
      permission: isSet(object.permission)
        ? permissionInfo_PermissionFromJSON(object.permission)
        : 0,
      enabled: isSet(object.enabled) ? Boolean(object.enabled) : false,
    };
  },

  toJSON(message: PermissionInfo): unknown {
    const obj: any = {};
    message.permission !== undefined &&
      (obj.permission = permissionInfo_PermissionToJSON(message.permission));
    message.enabled !== undefined && (obj.enabled = message.enabled);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PermissionInfo>, I>>(
    object: I
  ): PermissionInfo {
    const message = createBasePermissionInfo();
    message.permission = object.permission ?? 0;
    message.enabled = object.enabled ?? false;
    return message;
  },
};

function createBaseOptionMessage(): OptionMessage {
  return {
    imageQuality: 0,
    lockAfterSessionEnd: 0,
    showRemoteCursor: 0,
    privacyMode: 0,
    blockInput: 0,
    customImageQuality: 0,
    disableAudio: 0,
    disableClipboard: 0,
  };
}

export const OptionMessage = {
  encode(
    message: OptionMessage,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.imageQuality !== 0) {
      writer.uint32(8).int32(message.imageQuality);
    }
    if (message.lockAfterSessionEnd !== 0) {
      writer.uint32(16).int32(message.lockAfterSessionEnd);
    }
    if (message.showRemoteCursor !== 0) {
      writer.uint32(24).int32(message.showRemoteCursor);
    }
    if (message.privacyMode !== 0) {
      writer.uint32(32).int32(message.privacyMode);
    }
    if (message.blockInput !== 0) {
      writer.uint32(40).int32(message.blockInput);
    }
    if (message.customImageQuality !== 0) {
      writer.uint32(48).int32(message.customImageQuality);
    }
    if (message.disableAudio !== 0) {
      writer.uint32(56).int32(message.disableAudio);
    }
    if (message.disableClipboard !== 0) {
      writer.uint32(64).int32(message.disableClipboard);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): OptionMessage {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseOptionMessage();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.imageQuality = reader.int32() as any;
          break;
        case 2:
          message.lockAfterSessionEnd = reader.int32() as any;
          break;
        case 3:
          message.showRemoteCursor = reader.int32() as any;
          break;
        case 4:
          message.privacyMode = reader.int32() as any;
          break;
        case 5:
          message.blockInput = reader.int32() as any;
          break;
        case 6:
          message.customImageQuality = reader.int32();
          break;
        case 7:
          message.disableAudio = reader.int32() as any;
          break;
        case 8:
          message.disableClipboard = reader.int32() as any;
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): OptionMessage {
    return {
      imageQuality: isSet(object.imageQuality)
        ? imageQualityFromJSON(object.imageQuality)
        : 0,
      lockAfterSessionEnd: isSet(object.lockAfterSessionEnd)
        ? optionMessage_BoolOptionFromJSON(object.lockAfterSessionEnd)
        : 0,
      showRemoteCursor: isSet(object.showRemoteCursor)
        ? optionMessage_BoolOptionFromJSON(object.showRemoteCursor)
        : 0,
      privacyMode: isSet(object.privacyMode)
        ? optionMessage_BoolOptionFromJSON(object.privacyMode)
        : 0,
      blockInput: isSet(object.blockInput)
        ? optionMessage_BoolOptionFromJSON(object.blockInput)
        : 0,
      customImageQuality: isSet(object.customImageQuality)
        ? Number(object.customImageQuality)
        : 0,
      disableAudio: isSet(object.disableAudio)
        ? optionMessage_BoolOptionFromJSON(object.disableAudio)
        : 0,
      disableClipboard: isSet(object.disableClipboard)
        ? optionMessage_BoolOptionFromJSON(object.disableClipboard)
        : 0,
    };
  },

  toJSON(message: OptionMessage): unknown {
    const obj: any = {};
    message.imageQuality !== undefined &&
      (obj.imageQuality = imageQualityToJSON(message.imageQuality));
    message.lockAfterSessionEnd !== undefined &&
      (obj.lockAfterSessionEnd = optionMessage_BoolOptionToJSON(
        message.lockAfterSessionEnd
      ));
    message.showRemoteCursor !== undefined &&
      (obj.showRemoteCursor = optionMessage_BoolOptionToJSON(
        message.showRemoteCursor
      ));
    message.privacyMode !== undefined &&
      (obj.privacyMode = optionMessage_BoolOptionToJSON(message.privacyMode));
    message.blockInput !== undefined &&
      (obj.blockInput = optionMessage_BoolOptionToJSON(message.blockInput));
    message.customImageQuality !== undefined &&
      (obj.customImageQuality = Math.round(message.customImageQuality));
    message.disableAudio !== undefined &&
      (obj.disableAudio = optionMessage_BoolOptionToJSON(message.disableAudio));
    message.disableClipboard !== undefined &&
      (obj.disableClipboard = optionMessage_BoolOptionToJSON(
        message.disableClipboard
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<OptionMessage>, I>>(
    object: I
  ): OptionMessage {
    const message = createBaseOptionMessage();
    message.imageQuality = object.imageQuality ?? 0;
    message.lockAfterSessionEnd = object.lockAfterSessionEnd ?? 0;
    message.showRemoteCursor = object.showRemoteCursor ?? 0;
    message.privacyMode = object.privacyMode ?? 0;
    message.blockInput = object.blockInput ?? 0;
    message.customImageQuality = object.customImageQuality ?? 0;
    message.disableAudio = object.disableAudio ?? 0;
    message.disableClipboard = object.disableClipboard ?? 0;
    return message;
  },
};

function createBaseOptionResponse(): OptionResponse {
  return { opt: undefined, error: "" };
}

export const OptionResponse = {
  encode(
    message: OptionResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.opt !== undefined) {
      OptionMessage.encode(message.opt, writer.uint32(10).fork()).ldelim();
    }
    if (message.error !== "") {
      writer.uint32(18).string(message.error);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): OptionResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseOptionResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.opt = OptionMessage.decode(reader, reader.uint32());
          break;
        case 2:
          message.error = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): OptionResponse {
    return {
      opt: isSet(object.opt) ? OptionMessage.fromJSON(object.opt) : undefined,
      error: isSet(object.error) ? String(object.error) : "",
    };
  },

  toJSON(message: OptionResponse): unknown {
    const obj: any = {};
    message.opt !== undefined &&
      (obj.opt = message.opt ? OptionMessage.toJSON(message.opt) : undefined);
    message.error !== undefined && (obj.error = message.error);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<OptionResponse>, I>>(
    object: I
  ): OptionResponse {
    const message = createBaseOptionResponse();
    message.opt =
      object.opt !== undefined && object.opt !== null
        ? OptionMessage.fromPartial(object.opt)
        : undefined;
    message.error = object.error ?? "";
    return message;
  },
};

function createBaseTestDelay(): TestDelay {
  return { time: 0, fromClient: false };
}

export const TestDelay = {
  encode(
    message: TestDelay,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.time !== 0) {
      writer.uint32(8).int64(message.time);
    }
    if (message.fromClient === true) {
      writer.uint32(16).bool(message.fromClient);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): TestDelay {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseTestDelay();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.time = longToNumber(reader.int64() as Long);
          break;
        case 2:
          message.fromClient = reader.bool();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): TestDelay {
    return {
      time: isSet(object.time) ? Number(object.time) : 0,
      fromClient: isSet(object.fromClient) ? Boolean(object.fromClient) : false,
    };
  },

  toJSON(message: TestDelay): unknown {
    const obj: any = {};
    message.time !== undefined && (obj.time = Math.round(message.time));
    message.fromClient !== undefined && (obj.fromClient = message.fromClient);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<TestDelay>, I>>(
    object: I
  ): TestDelay {
    const message = createBaseTestDelay();
    message.time = object.time ?? 0;
    message.fromClient = object.fromClient ?? false;
    return message;
  },
};

function createBasePublicKey(): PublicKey {
  return {
    asymmetricValue: new Uint8Array(),
    symmetricValue: new Uint8Array(),
  };
}

export const PublicKey = {
  encode(
    message: PublicKey,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.asymmetricValue.length !== 0) {
      writer.uint32(10).bytes(message.asymmetricValue);
    }
    if (message.symmetricValue.length !== 0) {
      writer.uint32(18).bytes(message.symmetricValue);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): PublicKey {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBasePublicKey();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.asymmetricValue = reader.bytes();
          break;
        case 2:
          message.symmetricValue = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): PublicKey {
    return {
      asymmetricValue: isSet(object.asymmetricValue)
        ? bytesFromBase64(object.asymmetricValue)
        : new Uint8Array(),
      symmetricValue: isSet(object.symmetricValue)
        ? bytesFromBase64(object.symmetricValue)
        : new Uint8Array(),
    };
  },

  toJSON(message: PublicKey): unknown {
    const obj: any = {};
    message.asymmetricValue !== undefined &&
      (obj.asymmetricValue = base64FromBytes(
        message.asymmetricValue !== undefined
          ? message.asymmetricValue
          : new Uint8Array()
      ));
    message.symmetricValue !== undefined &&
      (obj.symmetricValue = base64FromBytes(
        message.symmetricValue !== undefined
          ? message.symmetricValue
          : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PublicKey>, I>>(
    object: I
  ): PublicKey {
    const message = createBasePublicKey();
    message.asymmetricValue = object.asymmetricValue ?? new Uint8Array();
    message.symmetricValue = object.symmetricValue ?? new Uint8Array();
    return message;
  },
};

function createBaseSignedId(): SignedId {
  return { id: new Uint8Array() };
}

export const SignedId = {
  encode(
    message: SignedId,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id.length !== 0) {
      writer.uint32(10).bytes(message.id);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): SignedId {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseSignedId();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): SignedId {
    return {
      id: isSet(object.id) ? bytesFromBase64(object.id) : new Uint8Array(),
    };
  },

  toJSON(message: SignedId): unknown {
    const obj: any = {};
    message.id !== undefined &&
      (obj.id = base64FromBytes(
        message.id !== undefined ? message.id : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<SignedId>, I>>(object: I): SignedId {
    const message = createBaseSignedId();
    message.id = object.id ?? new Uint8Array();
    return message;
  },
};

function createBaseAudioFormat(): AudioFormat {
  return { sampleRate: 0, channels: 0 };
}

export const AudioFormat = {
  encode(
    message: AudioFormat,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.sampleRate !== 0) {
      writer.uint32(8).uint32(message.sampleRate);
    }
    if (message.channels !== 0) {
      writer.uint32(16).uint32(message.channels);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): AudioFormat {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseAudioFormat();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.sampleRate = reader.uint32();
          break;
        case 2:
          message.channels = reader.uint32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): AudioFormat {
    return {
      sampleRate: isSet(object.sampleRate) ? Number(object.sampleRate) : 0,
      channels: isSet(object.channels) ? Number(object.channels) : 0,
    };
  },

  toJSON(message: AudioFormat): unknown {
    const obj: any = {};
    message.sampleRate !== undefined &&
      (obj.sampleRate = Math.round(message.sampleRate));
    message.channels !== undefined &&
      (obj.channels = Math.round(message.channels));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<AudioFormat>, I>>(
    object: I
  ): AudioFormat {
    const message = createBaseAudioFormat();
    message.sampleRate = object.sampleRate ?? 0;
    message.channels = object.channels ?? 0;
    return message;
  },
};

function createBaseAudioFrame(): AudioFrame {
  return { data: new Uint8Array() };
}

export const AudioFrame = {
  encode(
    message: AudioFrame,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.data.length !== 0) {
      writer.uint32(10).bytes(message.data);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): AudioFrame {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseAudioFrame();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.data = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): AudioFrame {
    return {
      data: isSet(object.data)
        ? bytesFromBase64(object.data)
        : new Uint8Array(),
    };
  },

  toJSON(message: AudioFrame): unknown {
    const obj: any = {};
    message.data !== undefined &&
      (obj.data = base64FromBytes(
        message.data !== undefined ? message.data : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<AudioFrame>, I>>(
    object: I
  ): AudioFrame {
    const message = createBaseAudioFrame();
    message.data = object.data ?? new Uint8Array();
    return message;
  },
};

function createBaseMisc(): Misc {
  return {
    chatMessage: undefined,
    switchDisplay: undefined,
    permissionInfo: undefined,
    option: undefined,
    audioFormat: undefined,
    closeReason: undefined,
    refreshVideo: undefined,
    optionResponse: undefined,
  };
}

export const Misc = {
  encode(message: Misc, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.chatMessage !== undefined) {
      ChatMessage.encode(
        message.chatMessage,
        writer.uint32(34).fork()
      ).ldelim();
    }
    if (message.switchDisplay !== undefined) {
      SwitchDisplay.encode(
        message.switchDisplay,
        writer.uint32(42).fork()
      ).ldelim();
    }
    if (message.permissionInfo !== undefined) {
      PermissionInfo.encode(
        message.permissionInfo,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.option !== undefined) {
      OptionMessage.encode(message.option, writer.uint32(58).fork()).ldelim();
    }
    if (message.audioFormat !== undefined) {
      AudioFormat.encode(
        message.audioFormat,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.closeReason !== undefined) {
      writer.uint32(74).string(message.closeReason);
    }
    if (message.refreshVideo !== undefined) {
      writer.uint32(80).bool(message.refreshVideo);
    }
    if (message.optionResponse !== undefined) {
      OptionResponse.encode(
        message.optionResponse,
        writer.uint32(90).fork()
      ).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Misc {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseMisc();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 4:
          message.chatMessage = ChatMessage.decode(reader, reader.uint32());
          break;
        case 5:
          message.switchDisplay = SwitchDisplay.decode(reader, reader.uint32());
          break;
        case 6:
          message.permissionInfo = PermissionInfo.decode(
            reader,
            reader.uint32()
          );
          break;
        case 7:
          message.option = OptionMessage.decode(reader, reader.uint32());
          break;
        case 8:
          message.audioFormat = AudioFormat.decode(reader, reader.uint32());
          break;
        case 9:
          message.closeReason = reader.string();
          break;
        case 10:
          message.refreshVideo = reader.bool();
          break;
        case 11:
          message.optionResponse = OptionResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): Misc {
    return {
      chatMessage: isSet(object.chatMessage)
        ? ChatMessage.fromJSON(object.chatMessage)
        : undefined,
      switchDisplay: isSet(object.switchDisplay)
        ? SwitchDisplay.fromJSON(object.switchDisplay)
        : undefined,
      permissionInfo: isSet(object.permissionInfo)
        ? PermissionInfo.fromJSON(object.permissionInfo)
        : undefined,
      option: isSet(object.option)
        ? OptionMessage.fromJSON(object.option)
        : undefined,
      audioFormat: isSet(object.audioFormat)
        ? AudioFormat.fromJSON(object.audioFormat)
        : undefined,
      closeReason: isSet(object.closeReason)
        ? String(object.closeReason)
        : undefined,
      refreshVideo: isSet(object.refreshVideo)
        ? Boolean(object.refreshVideo)
        : undefined,
      optionResponse: isSet(object.optionResponse)
        ? OptionResponse.fromJSON(object.optionResponse)
        : undefined,
    };
  },

  toJSON(message: Misc): unknown {
    const obj: any = {};
    message.chatMessage !== undefined &&
      (obj.chatMessage = message.chatMessage
        ? ChatMessage.toJSON(message.chatMessage)
        : undefined);
    message.switchDisplay !== undefined &&
      (obj.switchDisplay = message.switchDisplay
        ? SwitchDisplay.toJSON(message.switchDisplay)
        : undefined);
    message.permissionInfo !== undefined &&
      (obj.permissionInfo = message.permissionInfo
        ? PermissionInfo.toJSON(message.permissionInfo)
        : undefined);
    message.option !== undefined &&
      (obj.option = message.option
        ? OptionMessage.toJSON(message.option)
        : undefined);
    message.audioFormat !== undefined &&
      (obj.audioFormat = message.audioFormat
        ? AudioFormat.toJSON(message.audioFormat)
        : undefined);
    message.closeReason !== undefined &&
      (obj.closeReason = message.closeReason);
    message.refreshVideo !== undefined &&
      (obj.refreshVideo = message.refreshVideo);
    message.optionResponse !== undefined &&
      (obj.optionResponse = message.optionResponse
        ? OptionResponse.toJSON(message.optionResponse)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Misc>, I>>(object: I): Misc {
    const message = createBaseMisc();
    message.chatMessage =
      object.chatMessage !== undefined && object.chatMessage !== null
        ? ChatMessage.fromPartial(object.chatMessage)
        : undefined;
    message.switchDisplay =
      object.switchDisplay !== undefined && object.switchDisplay !== null
        ? SwitchDisplay.fromPartial(object.switchDisplay)
        : undefined;
    message.permissionInfo =
      object.permissionInfo !== undefined && object.permissionInfo !== null
        ? PermissionInfo.fromPartial(object.permissionInfo)
        : undefined;
    message.option =
      object.option !== undefined && object.option !== null
        ? OptionMessage.fromPartial(object.option)
        : undefined;
    message.audioFormat =
      object.audioFormat !== undefined && object.audioFormat !== null
        ? AudioFormat.fromPartial(object.audioFormat)
        : undefined;
    message.closeReason = object.closeReason ?? undefined;
    message.refreshVideo = object.refreshVideo ?? undefined;
    message.optionResponse =
      object.optionResponse !== undefined && object.optionResponse !== null
        ? OptionResponse.fromPartial(object.optionResponse)
        : undefined;
    return message;
  },
};

function createBaseMessage(): Message {
  return {
    signedId: undefined,
    publicKey: undefined,
    testDelay: undefined,
    videoFrame: undefined,
    loginRequest: undefined,
    loginResponse: undefined,
    hash: undefined,
    mouseEvent: undefined,
    audioFrame: undefined,
    cursorData: undefined,
    cursorPosition: undefined,
    cursorId: undefined,
    keyEvent: undefined,
    clipboard: undefined,
    fileAction: undefined,
    fileResponse: undefined,
    misc: undefined,
  };
}

export const Message = {
  encode(
    message: Message,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.signedId !== undefined) {
      SignedId.encode(message.signedId, writer.uint32(26).fork()).ldelim();
    }
    if (message.publicKey !== undefined) {
      PublicKey.encode(message.publicKey, writer.uint32(34).fork()).ldelim();
    }
    if (message.testDelay !== undefined) {
      TestDelay.encode(message.testDelay, writer.uint32(42).fork()).ldelim();
    }
    if (message.videoFrame !== undefined) {
      VideoFrame.encode(message.videoFrame, writer.uint32(50).fork()).ldelim();
    }
    if (message.loginRequest !== undefined) {
      LoginRequest.encode(
        message.loginRequest,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.loginResponse !== undefined) {
      LoginResponse.encode(
        message.loginResponse,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.hash !== undefined) {
      Hash.encode(message.hash, writer.uint32(74).fork()).ldelim();
    }
    if (message.mouseEvent !== undefined) {
      MouseEvent.encode(message.mouseEvent, writer.uint32(82).fork()).ldelim();
    }
    if (message.audioFrame !== undefined) {
      AudioFrame.encode(message.audioFrame, writer.uint32(90).fork()).ldelim();
    }
    if (message.cursorData !== undefined) {
      CursorData.encode(message.cursorData, writer.uint32(98).fork()).ldelim();
    }
    if (message.cursorPosition !== undefined) {
      CursorPosition.encode(
        message.cursorPosition,
        writer.uint32(106).fork()
      ).ldelim();
    }
    if (message.cursorId !== undefined) {
      writer.uint32(112).uint64(message.cursorId);
    }
    if (message.keyEvent !== undefined) {
      KeyEvent.encode(message.keyEvent, writer.uint32(122).fork()).ldelim();
    }
    if (message.clipboard !== undefined) {
      Clipboard.encode(message.clipboard, writer.uint32(130).fork()).ldelim();
    }
    if (message.fileAction !== undefined) {
      FileAction.encode(message.fileAction, writer.uint32(138).fork()).ldelim();
    }
    if (message.fileResponse !== undefined) {
      FileResponse.encode(
        message.fileResponse,
        writer.uint32(146).fork()
      ).ldelim();
    }
    if (message.misc !== undefined) {
      Misc.encode(message.misc, writer.uint32(154).fork()).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Message {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseMessage();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 3:
          message.signedId = SignedId.decode(reader, reader.uint32());
          break;
        case 4:
          message.publicKey = PublicKey.decode(reader, reader.uint32());
          break;
        case 5:
          message.testDelay = TestDelay.decode(reader, reader.uint32());
          break;
        case 6:
          message.videoFrame = VideoFrame.decode(reader, reader.uint32());
          break;
        case 7:
          message.loginRequest = LoginRequest.decode(reader, reader.uint32());
          break;
        case 8:
          message.loginResponse = LoginResponse.decode(reader, reader.uint32());
          break;
        case 9:
          message.hash = Hash.decode(reader, reader.uint32());
          break;
        case 10:
          message.mouseEvent = MouseEvent.decode(reader, reader.uint32());
          break;
        case 11:
          message.audioFrame = AudioFrame.decode(reader, reader.uint32());
          break;
        case 12:
          message.cursorData = CursorData.decode(reader, reader.uint32());
          break;
        case 13:
          message.cursorPosition = CursorPosition.decode(
            reader,
            reader.uint32()
          );
          break;
        case 14:
          message.cursorId = longToNumber(reader.uint64() as Long);
          break;
        case 15:
          message.keyEvent = KeyEvent.decode(reader, reader.uint32());
          break;
        case 16:
          message.clipboard = Clipboard.decode(reader, reader.uint32());
          break;
        case 17:
          message.fileAction = FileAction.decode(reader, reader.uint32());
          break;
        case 18:
          message.fileResponse = FileResponse.decode(reader, reader.uint32());
          break;
        case 19:
          message.misc = Misc.decode(reader, reader.uint32());
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): Message {
    return {
      signedId: isSet(object.signedId)
        ? SignedId.fromJSON(object.signedId)
        : undefined,
      publicKey: isSet(object.publicKey)
        ? PublicKey.fromJSON(object.publicKey)
        : undefined,
      testDelay: isSet(object.testDelay)
        ? TestDelay.fromJSON(object.testDelay)
        : undefined,
      videoFrame: isSet(object.videoFrame)
        ? VideoFrame.fromJSON(object.videoFrame)
        : undefined,
      loginRequest: isSet(object.loginRequest)
        ? LoginRequest.fromJSON(object.loginRequest)
        : undefined,
      loginResponse: isSet(object.loginResponse)
        ? LoginResponse.fromJSON(object.loginResponse)
        : undefined,
      hash: isSet(object.hash) ? Hash.fromJSON(object.hash) : undefined,
      mouseEvent: isSet(object.mouseEvent)
        ? MouseEvent.fromJSON(object.mouseEvent)
        : undefined,
      audioFrame: isSet(object.audioFrame)
        ? AudioFrame.fromJSON(object.audioFrame)
        : undefined,
      cursorData: isSet(object.cursorData)
        ? CursorData.fromJSON(object.cursorData)
        : undefined,
      cursorPosition: isSet(object.cursorPosition)
        ? CursorPosition.fromJSON(object.cursorPosition)
        : undefined,
      cursorId: isSet(object.cursorId) ? Number(object.cursorId) : undefined,
      keyEvent: isSet(object.keyEvent)
        ? KeyEvent.fromJSON(object.keyEvent)
        : undefined,
      clipboard: isSet(object.clipboard)
        ? Clipboard.fromJSON(object.clipboard)
        : undefined,
      fileAction: isSet(object.fileAction)
        ? FileAction.fromJSON(object.fileAction)
        : undefined,
      fileResponse: isSet(object.fileResponse)
        ? FileResponse.fromJSON(object.fileResponse)
        : undefined,
      misc: isSet(object.misc) ? Misc.fromJSON(object.misc) : undefined,
    };
  },

  toJSON(message: Message): unknown {
    const obj: any = {};
    message.signedId !== undefined &&
      (obj.signedId = message.signedId
        ? SignedId.toJSON(message.signedId)
        : undefined);
    message.publicKey !== undefined &&
      (obj.publicKey = message.publicKey
        ? PublicKey.toJSON(message.publicKey)
        : undefined);
    message.testDelay !== undefined &&
      (obj.testDelay = message.testDelay
        ? TestDelay.toJSON(message.testDelay)
        : undefined);
    message.videoFrame !== undefined &&
      (obj.videoFrame = message.videoFrame
        ? VideoFrame.toJSON(message.videoFrame)
        : undefined);
    message.loginRequest !== undefined &&
      (obj.loginRequest = message.loginRequest
        ? LoginRequest.toJSON(message.loginRequest)
        : undefined);
    message.loginResponse !== undefined &&
      (obj.loginResponse = message.loginResponse
        ? LoginResponse.toJSON(message.loginResponse)
        : undefined);
    message.hash !== undefined &&
      (obj.hash = message.hash ? Hash.toJSON(message.hash) : undefined);
    message.mouseEvent !== undefined &&
      (obj.mouseEvent = message.mouseEvent
        ? MouseEvent.toJSON(message.mouseEvent)
        : undefined);
    message.audioFrame !== undefined &&
      (obj.audioFrame = message.audioFrame
        ? AudioFrame.toJSON(message.audioFrame)
        : undefined);
    message.cursorData !== undefined &&
      (obj.cursorData = message.cursorData
        ? CursorData.toJSON(message.cursorData)
        : undefined);
    message.cursorPosition !== undefined &&
      (obj.cursorPosition = message.cursorPosition
        ? CursorPosition.toJSON(message.cursorPosition)
        : undefined);
    message.cursorId !== undefined &&
      (obj.cursorId = Math.round(message.cursorId));
    message.keyEvent !== undefined &&
      (obj.keyEvent = message.keyEvent
        ? KeyEvent.toJSON(message.keyEvent)
        : undefined);
    message.clipboard !== undefined &&
      (obj.clipboard = message.clipboard
        ? Clipboard.toJSON(message.clipboard)
        : undefined);
    message.fileAction !== undefined &&
      (obj.fileAction = message.fileAction
        ? FileAction.toJSON(message.fileAction)
        : undefined);
    message.fileResponse !== undefined &&
      (obj.fileResponse = message.fileResponse
        ? FileResponse.toJSON(message.fileResponse)
        : undefined);
    message.misc !== undefined &&
      (obj.misc = message.misc ? Misc.toJSON(message.misc) : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Message>, I>>(object: I): Message {
    const message = createBaseMessage();
    message.signedId =
      object.signedId !== undefined && object.signedId !== null
        ? SignedId.fromPartial(object.signedId)
        : undefined;
    message.publicKey =
      object.publicKey !== undefined && object.publicKey !== null
        ? PublicKey.fromPartial(object.publicKey)
        : undefined;
    message.testDelay =
      object.testDelay !== undefined && object.testDelay !== null
        ? TestDelay.fromPartial(object.testDelay)
        : undefined;
    message.videoFrame =
      object.videoFrame !== undefined && object.videoFrame !== null
        ? VideoFrame.fromPartial(object.videoFrame)
        : undefined;
    message.loginRequest =
      object.loginRequest !== undefined && object.loginRequest !== null
        ? LoginRequest.fromPartial(object.loginRequest)
        : undefined;
    message.loginResponse =
      object.loginResponse !== undefined && object.loginResponse !== null
        ? LoginResponse.fromPartial(object.loginResponse)
        : undefined;
    message.hash =
      object.hash !== undefined && object.hash !== null
        ? Hash.fromPartial(object.hash)
        : undefined;
    message.mouseEvent =
      object.mouseEvent !== undefined && object.mouseEvent !== null
        ? MouseEvent.fromPartial(object.mouseEvent)
        : undefined;
    message.audioFrame =
      object.audioFrame !== undefined && object.audioFrame !== null
        ? AudioFrame.fromPartial(object.audioFrame)
        : undefined;
    message.cursorData =
      object.cursorData !== undefined && object.cursorData !== null
        ? CursorData.fromPartial(object.cursorData)
        : undefined;
    message.cursorPosition =
      object.cursorPosition !== undefined && object.cursorPosition !== null
        ? CursorPosition.fromPartial(object.cursorPosition)
        : undefined;
    message.cursorId = object.cursorId ?? undefined;
    message.keyEvent =
      object.keyEvent !== undefined && object.keyEvent !== null
        ? KeyEvent.fromPartial(object.keyEvent)
        : undefined;
    message.clipboard =
      object.clipboard !== undefined && object.clipboard !== null
        ? Clipboard.fromPartial(object.clipboard)
        : undefined;
    message.fileAction =
      object.fileAction !== undefined && object.fileAction !== null
        ? FileAction.fromPartial(object.fileAction)
        : undefined;
    message.fileResponse =
      object.fileResponse !== undefined && object.fileResponse !== null
        ? FileResponse.fromPartial(object.fileResponse)
        : undefined;
    message.misc =
      object.misc !== undefined && object.misc !== null
        ? Misc.fromPartial(object.misc)
        : undefined;
    return message;
  },
};

declare var self: any | undefined;
declare var window: any | undefined;
declare var global: any | undefined;
var globalThis: any = (() => {
  if (typeof globalThis !== "undefined") return globalThis;
  if (typeof self !== "undefined") return self;
  if (typeof window !== "undefined") return window;
  if (typeof global !== "undefined") return global;
  throw "Unable to locate global object";
})();

const atob: (b64: string) => string =
  globalThis.atob ||
  ((b64) => globalThis.Buffer.from(b64, "base64").toString("binary"));
function bytesFromBase64(b64: string): Uint8Array {
  const bin = atob(b64);
  const arr = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; ++i) {
    arr[i] = bin.charCodeAt(i);
  }
  return arr;
}

const btoa: (bin: string) => string =
  globalThis.btoa ||
  ((bin) => globalThis.Buffer.from(bin, "binary").toString("base64"));
function base64FromBytes(arr: Uint8Array): string {
  const bin: string[] = [];
  for (const byte of arr) {
    bin.push(String.fromCharCode(byte));
  }
  return btoa(bin.join(""));
}

type Builtin =
  | Date
  | Function
  | Uint8Array
  | string
  | number
  | boolean
  | undefined;

export type DeepPartial<T> = T extends Builtin
  ? T
  : T extends Array<infer U>
  ? Array<DeepPartial<U>>
  : T extends ReadonlyArray<infer U>
  ? ReadonlyArray<DeepPartial<U>>
  : T extends {}
  ? { [K in keyof T]?: DeepPartial<T[K]> }
  : Partial<T>;

type KeysOfUnion<T> = T extends T ? keyof T : never;
export type Exact<P, I extends P> = P extends Builtin
  ? P
  : P & { [K in keyof P]: Exact<P[K], I[K]> } & Record<
        Exclude<keyof I, KeysOfUnion<P>>,
        never
      >;

function longToNumber(long: Long): number {
  if (long.gt(Number.MAX_SAFE_INTEGER)) {
    throw new globalThis.Error("Value is larger than Number.MAX_SAFE_INTEGER");
  }
  return long.toNumber();
}

if (_m0.util.Long !== Long) {
  _m0.util.Long = Long as any;
  _m0.configure();
}

function isSet(value: any): boolean {
  return value !== null && value !== undefined;
}
