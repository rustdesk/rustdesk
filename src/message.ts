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
  Dir = 0,
  DirLink = 2,
  DirDrive = 3,
  File = 4,
  FileLink = 5,
  UNRECOGNIZED = -1,
}

export function fileTypeFromJSON(object: any): FileType {
  switch (object) {
    case 0:
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

export interface IdPk {
  id: string;
  pk: Uint8Array;
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
  show_hidden: boolean;
}

export interface LoginRequest {
  username: string;
  password: Uint8Array;
  my_id: string;
  my_name: string;
  option: OptionMessage | undefined;
  file_transfer: FileTransfer | undefined;
  port_forward: PortForward | undefined;
  video_ack_required: boolean;
}

export interface ChatMessage {
  text: string;
}

export interface PeerInfo {
  username: string;
  hostname: string;
  platform: string;
  displays: DisplayInfo[];
  current_display: number;
  sas_enabled: boolean;
  version: string;
  conn_id: number;
  home_dir: string;
}

export interface LoginResponse {
  error: string | undefined;
  peer_info: PeerInfo | undefined;
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
  control_key: ControlKey | undefined;
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
  entry_type: FileType;
  name: string;
  is_hidden: boolean;
  size: number;
  modified_time: number;
}

export interface FileDirectory {
  id: number;
  path: string;
  entries: FileEntry[];
}

export interface ReadDir {
  path: string;
  include_hidden: boolean;
}

export interface ReadAllFiles {
  id: number;
  path: string;
  include_hidden: boolean;
}

export interface FileAction {
  read_dir: ReadDir | undefined;
  send: FileTransferSendRequest | undefined;
  receive: FileTransferReceiveRequest | undefined;
  create: FileDirCreate | undefined;
  remove_dir: FileRemoveDir | undefined;
  remove_file: FileRemoveFile | undefined;
  all_files: ReadAllFiles | undefined;
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
  file_num: number;
  data: Uint8Array;
  compressed: boolean;
}

export interface FileTransferError {
  id: number;
  error: string;
  file_num: number;
}

export interface FileTransferSendRequest {
  id: number;
  path: string;
  include_hidden: boolean;
}

export interface FileTransferDone {
  id: number;
  file_num: number;
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
  file_num: number;
}

export interface FileDirCreate {
  id: number;
  path: string;
}

/** main logic from freeRDP */
export interface CliprdrMonitorReady {
  conn_id: number;
}

export interface CliprdrFormat {
  conn_id: number;
  id: number;
  format: string;
}

export interface CliprdrServerFormatList {
  conn_id: number;
  formats: CliprdrFormat[];
}

export interface CliprdrServerFormatListResponse {
  conn_id: number;
  msg_flags: number;
}

export interface CliprdrServerFormatDataRequest {
  conn_id: number;
  requested_format_id: number;
}

export interface CliprdrServerFormatDataResponse {
  conn_id: number;
  msg_flags: number;
  format_data: Uint8Array;
}

export interface CliprdrFileContentsRequest {
  conn_id: number;
  stream_id: number;
  list_index: number;
  dw_flags: number;
  n_position_low: number;
  n_position_high: number;
  cb_requested: number;
  have_clip_data_id: boolean;
  clip_data_id: number;
}

export interface CliprdrFileContentsResponse {
  conn_id: number;
  msg_flags: number;
  stream_id: number;
  requested_data: Uint8Array;
}

export interface Cliprdr {
  ready: CliprdrMonitorReady | undefined;
  format_list: CliprdrServerFormatList | undefined;
  format_list_response: CliprdrServerFormatListResponse | undefined;
  format_data_request: CliprdrServerFormatDataRequest | undefined;
  format_data_response: CliprdrServerFormatDataResponse | undefined;
  file_contents_request: CliprdrFileContentsRequest | undefined;
  file_contents_response: CliprdrFileContentsResponse | undefined;
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
  Keyboard = 0,
  Clipboard = 2,
  Audio = 3,
  File = 4,
  UNRECOGNIZED = -1,
}

export function permissionInfo_PermissionFromJSON(
  object: any
): PermissionInfo_Permission {
  switch (object) {
    case 0:
    case "Keyboard":
      return PermissionInfo_Permission.Keyboard;
    case 2:
    case "Clipboard":
      return PermissionInfo_Permission.Clipboard;
    case 3:
    case "Audio":
      return PermissionInfo_Permission.Audio;
    case 4:
    case "File":
      return PermissionInfo_Permission.File;
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
    case PermissionInfo_Permission.Keyboard:
      return "Keyboard";
    case PermissionInfo_Permission.Clipboard:
      return "Clipboard";
    case PermissionInfo_Permission.Audio:
      return "Audio";
    case PermissionInfo_Permission.File:
      return "File";
    default:
      return "UNKNOWN";
  }
}

export interface OptionMessage {
  image_quality: ImageQuality;
  lock_after_session_end: OptionMessage_BoolOption;
  show_remote_cursor: OptionMessage_BoolOption;
  privacy_mode: OptionMessage_BoolOption;
  block_input: OptionMessage_BoolOption;
  custom_image_quality: number;
  disable_audio: OptionMessage_BoolOption;
  disable_clipboard: OptionMessage_BoolOption;
  enable_file_transfer: OptionMessage_BoolOption;
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
  from_client: boolean;
}

export interface PublicKey {
  asymmetric_value: Uint8Array;
  symmetric_value: Uint8Array;
}

export interface SignedId {
  id: Uint8Array;
}

export interface AudioFormat {
  sample_rate: number;
  channels: number;
}

export interface AudioFrame {
  data: Uint8Array;
}

export interface Misc {
  chat_message: ChatMessage | undefined;
  switch_display: SwitchDisplay | undefined;
  permission_info: PermissionInfo | undefined;
  option: OptionMessage | undefined;
  audio_format: AudioFormat | undefined;
  close_reason: string | undefined;
  refresh_video: boolean | undefined;
  option_response: OptionResponse | undefined;
  video_received: boolean | undefined;
}

export interface Message {
  signed_id: SignedId | undefined;
  public_key: PublicKey | undefined;
  test_delay: TestDelay | undefined;
  video_frame: VideoFrame | undefined;
  login_request: LoginRequest | undefined;
  login_response: LoginResponse | undefined;
  hash: Hash | undefined;
  mouse_event: MouseEvent | undefined;
  audio_frame: AudioFrame | undefined;
  cursor_data: CursorData | undefined;
  cursor_position: CursorPosition | undefined;
  cursor_id: number | undefined;
  key_event: KeyEvent | undefined;
  clipboard: Clipboard | undefined;
  file_action: FileAction | undefined;
  file_response: FileResponse | undefined;
  misc: Misc | undefined;
  cliprdr: Cliprdr | undefined;
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

function createBaseIdPk(): IdPk {
  return { id: "", pk: new Uint8Array() };
}

export const IdPk = {
  encode(message: IdPk, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.id !== "") {
      writer.uint32(10).string(message.id);
    }
    if (message.pk.length !== 0) {
      writer.uint32(18).bytes(message.pk);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): IdPk {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseIdPk();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.id = reader.string();
          break;
        case 2:
          message.pk = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): IdPk {
    return {
      id: isSet(object.id) ? String(object.id) : "",
      pk: isSet(object.pk) ? bytesFromBase64(object.pk) : new Uint8Array(),
    };
  },

  toJSON(message: IdPk): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = message.id);
    message.pk !== undefined &&
      (obj.pk = base64FromBytes(
        message.pk !== undefined ? message.pk : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<IdPk>, I>>(object: I): IdPk {
    const message = createBaseIdPk();
    message.id = object.id ?? "";
    message.pk = object.pk ?? new Uint8Array();
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
  return { dir: "", show_hidden: false };
}

export const FileTransfer = {
  encode(
    message: FileTransfer,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.dir !== "") {
      writer.uint32(10).string(message.dir);
    }
    if (message.show_hidden === true) {
      writer.uint32(16).bool(message.show_hidden);
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
          message.show_hidden = reader.bool();
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
      show_hidden: isSet(object.show_hidden)
        ? Boolean(object.show_hidden)
        : false,
    };
  },

  toJSON(message: FileTransfer): unknown {
    const obj: any = {};
    message.dir !== undefined && (obj.dir = message.dir);
    message.show_hidden !== undefined &&
      (obj.show_hidden = message.show_hidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransfer>, I>>(
    object: I
  ): FileTransfer {
    const message = createBaseFileTransfer();
    message.dir = object.dir ?? "";
    message.show_hidden = object.show_hidden ?? false;
    return message;
  },
};

function createBaseLoginRequest(): LoginRequest {
  return {
    username: "",
    password: new Uint8Array(),
    my_id: "",
    my_name: "",
    option: undefined,
    file_transfer: undefined,
    port_forward: undefined,
    video_ack_required: false,
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
    if (message.my_id !== "") {
      writer.uint32(34).string(message.my_id);
    }
    if (message.my_name !== "") {
      writer.uint32(42).string(message.my_name);
    }
    if (message.option !== undefined) {
      OptionMessage.encode(message.option, writer.uint32(50).fork()).ldelim();
    }
    if (message.file_transfer !== undefined) {
      FileTransfer.encode(
        message.file_transfer,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.port_forward !== undefined) {
      PortForward.encode(
        message.port_forward,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.video_ack_required === true) {
      writer.uint32(72).bool(message.video_ack_required);
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
          message.my_id = reader.string();
          break;
        case 5:
          message.my_name = reader.string();
          break;
        case 6:
          message.option = OptionMessage.decode(reader, reader.uint32());
          break;
        case 7:
          message.file_transfer = FileTransfer.decode(reader, reader.uint32());
          break;
        case 8:
          message.port_forward = PortForward.decode(reader, reader.uint32());
          break;
        case 9:
          message.video_ack_required = reader.bool();
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
      my_id: isSet(object.my_id) ? String(object.my_id) : "",
      my_name: isSet(object.my_name) ? String(object.my_name) : "",
      option: isSet(object.option)
        ? OptionMessage.fromJSON(object.option)
        : undefined,
      file_transfer: isSet(object.file_transfer)
        ? FileTransfer.fromJSON(object.file_transfer)
        : undefined,
      port_forward: isSet(object.port_forward)
        ? PortForward.fromJSON(object.port_forward)
        : undefined,
      video_ack_required: isSet(object.video_ack_required)
        ? Boolean(object.video_ack_required)
        : false,
    };
  },

  toJSON(message: LoginRequest): unknown {
    const obj: any = {};
    message.username !== undefined && (obj.username = message.username);
    message.password !== undefined &&
      (obj.password = base64FromBytes(
        message.password !== undefined ? message.password : new Uint8Array()
      ));
    message.my_id !== undefined && (obj.my_id = message.my_id);
    message.my_name !== undefined && (obj.my_name = message.my_name);
    message.option !== undefined &&
      (obj.option = message.option
        ? OptionMessage.toJSON(message.option)
        : undefined);
    message.file_transfer !== undefined &&
      (obj.file_transfer = message.file_transfer
        ? FileTransfer.toJSON(message.file_transfer)
        : undefined);
    message.port_forward !== undefined &&
      (obj.port_forward = message.port_forward
        ? PortForward.toJSON(message.port_forward)
        : undefined);
    message.video_ack_required !== undefined &&
      (obj.video_ack_required = message.video_ack_required);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LoginRequest>, I>>(
    object: I
  ): LoginRequest {
    const message = createBaseLoginRequest();
    message.username = object.username ?? "";
    message.password = object.password ?? new Uint8Array();
    message.my_id = object.my_id ?? "";
    message.my_name = object.my_name ?? "";
    message.option =
      object.option !== undefined && object.option !== null
        ? OptionMessage.fromPartial(object.option)
        : undefined;
    message.file_transfer =
      object.file_transfer !== undefined && object.file_transfer !== null
        ? FileTransfer.fromPartial(object.file_transfer)
        : undefined;
    message.port_forward =
      object.port_forward !== undefined && object.port_forward !== null
        ? PortForward.fromPartial(object.port_forward)
        : undefined;
    message.video_ack_required = object.video_ack_required ?? false;
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
    current_display: 0,
    sas_enabled: false,
    version: "",
    conn_id: 0,
    home_dir: "",
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
    if (message.current_display !== 0) {
      writer.uint32(40).int32(message.current_display);
    }
    if (message.sas_enabled === true) {
      writer.uint32(48).bool(message.sas_enabled);
    }
    if (message.version !== "") {
      writer.uint32(58).string(message.version);
    }
    if (message.conn_id !== 0) {
      writer.uint32(64).int32(message.conn_id);
    }
    if (message.home_dir !== "") {
      writer.uint32(74).string(message.home_dir);
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
          message.current_display = reader.int32();
          break;
        case 6:
          message.sas_enabled = reader.bool();
          break;
        case 7:
          message.version = reader.string();
          break;
        case 8:
          message.conn_id = reader.int32();
          break;
        case 9:
          message.home_dir = reader.string();
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
      current_display: isSet(object.current_display)
        ? Number(object.current_display)
        : 0,
      sas_enabled: isSet(object.sas_enabled)
        ? Boolean(object.sas_enabled)
        : false,
      version: isSet(object.version) ? String(object.version) : "",
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      home_dir: isSet(object.home_dir) ? String(object.home_dir) : "",
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
    message.current_display !== undefined &&
      (obj.current_display = Math.round(message.current_display));
    message.sas_enabled !== undefined &&
      (obj.sas_enabled = message.sas_enabled);
    message.version !== undefined && (obj.version = message.version);
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.home_dir !== undefined && (obj.home_dir = message.home_dir);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PeerInfo>, I>>(object: I): PeerInfo {
    const message = createBasePeerInfo();
    message.username = object.username ?? "";
    message.hostname = object.hostname ?? "";
    message.platform = object.platform ?? "";
    message.displays =
      object.displays?.map((e) => DisplayInfo.fromPartial(e)) || [];
    message.current_display = object.current_display ?? 0;
    message.sas_enabled = object.sas_enabled ?? false;
    message.version = object.version ?? "";
    message.conn_id = object.conn_id ?? 0;
    message.home_dir = object.home_dir ?? "";
    return message;
  },
};

function createBaseLoginResponse(): LoginResponse {
  return { error: undefined, peer_info: undefined };
}

export const LoginResponse = {
  encode(
    message: LoginResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.error !== undefined) {
      writer.uint32(10).string(message.error);
    }
    if (message.peer_info !== undefined) {
      PeerInfo.encode(message.peer_info, writer.uint32(18).fork()).ldelim();
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
          message.peer_info = PeerInfo.decode(reader, reader.uint32());
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
      peer_info: isSet(object.peer_info)
        ? PeerInfo.fromJSON(object.peer_info)
        : undefined,
    };
  },

  toJSON(message: LoginResponse): unknown {
    const obj: any = {};
    message.error !== undefined && (obj.error = message.error);
    message.peer_info !== undefined &&
      (obj.peer_info = message.peer_info
        ? PeerInfo.toJSON(message.peer_info)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<LoginResponse>, I>>(
    object: I
  ): LoginResponse {
    const message = createBaseLoginResponse();
    message.error = object.error ?? undefined;
    message.peer_info =
      object.peer_info !== undefined && object.peer_info !== null
        ? PeerInfo.fromPartial(object.peer_info)
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
    control_key: undefined,
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
    if (message.control_key !== undefined) {
      writer.uint32(24).int32(message.control_key);
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
          message.control_key = reader.int32() as any;
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
      control_key: isSet(object.control_key)
        ? controlKeyFromJSON(object.control_key)
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
    message.control_key !== undefined &&
      (obj.control_key =
        message.control_key !== undefined
          ? controlKeyToJSON(message.control_key)
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
    message.control_key = object.control_key ?? undefined;
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
  return {
    entry_type: 0,
    name: "",
    is_hidden: false,
    size: 0,
    modified_time: 0,
  };
}

export const FileEntry = {
  encode(
    message: FileEntry,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.entry_type !== 0) {
      writer.uint32(8).int32(message.entry_type);
    }
    if (message.name !== "") {
      writer.uint32(18).string(message.name);
    }
    if (message.is_hidden === true) {
      writer.uint32(24).bool(message.is_hidden);
    }
    if (message.size !== 0) {
      writer.uint32(32).uint64(message.size);
    }
    if (message.modified_time !== 0) {
      writer.uint32(40).uint64(message.modified_time);
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
          message.entry_type = reader.int32() as any;
          break;
        case 2:
          message.name = reader.string();
          break;
        case 3:
          message.is_hidden = reader.bool();
          break;
        case 4:
          message.size = longToNumber(reader.uint64() as Long);
          break;
        case 5:
          message.modified_time = longToNumber(reader.uint64() as Long);
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
      entry_type: isSet(object.entry_type)
        ? fileTypeFromJSON(object.entry_type)
        : 0,
      name: isSet(object.name) ? String(object.name) : "",
      is_hidden: isSet(object.is_hidden) ? Boolean(object.is_hidden) : false,
      size: isSet(object.size) ? Number(object.size) : 0,
      modified_time: isSet(object.modified_time)
        ? Number(object.modified_time)
        : 0,
    };
  },

  toJSON(message: FileEntry): unknown {
    const obj: any = {};
    message.entry_type !== undefined &&
      (obj.entry_type = fileTypeToJSON(message.entry_type));
    message.name !== undefined && (obj.name = message.name);
    message.is_hidden !== undefined && (obj.is_hidden = message.is_hidden);
    message.size !== undefined && (obj.size = Math.round(message.size));
    message.modified_time !== undefined &&
      (obj.modified_time = Math.round(message.modified_time));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileEntry>, I>>(
    object: I
  ): FileEntry {
    const message = createBaseFileEntry();
    message.entry_type = object.entry_type ?? 0;
    message.name = object.name ?? "";
    message.is_hidden = object.is_hidden ?? false;
    message.size = object.size ?? 0;
    message.modified_time = object.modified_time ?? 0;
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
  return { path: "", include_hidden: false };
}

export const ReadDir = {
  encode(
    message: ReadDir,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.path !== "") {
      writer.uint32(10).string(message.path);
    }
    if (message.include_hidden === true) {
      writer.uint32(16).bool(message.include_hidden);
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
          message.include_hidden = reader.bool();
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
      include_hidden: isSet(object.include_hidden)
        ? Boolean(object.include_hidden)
        : false,
    };
  },

  toJSON(message: ReadDir): unknown {
    const obj: any = {};
    message.path !== undefined && (obj.path = message.path);
    message.include_hidden !== undefined &&
      (obj.include_hidden = message.include_hidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ReadDir>, I>>(object: I): ReadDir {
    const message = createBaseReadDir();
    message.path = object.path ?? "";
    message.include_hidden = object.include_hidden ?? false;
    return message;
  },
};

function createBaseReadAllFiles(): ReadAllFiles {
  return { id: 0, path: "", include_hidden: false };
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
    if (message.include_hidden === true) {
      writer.uint32(24).bool(message.include_hidden);
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
          message.include_hidden = reader.bool();
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
      include_hidden: isSet(object.include_hidden)
        ? Boolean(object.include_hidden)
        : false,
    };
  },

  toJSON(message: ReadAllFiles): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.include_hidden !== undefined &&
      (obj.include_hidden = message.include_hidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<ReadAllFiles>, I>>(
    object: I
  ): ReadAllFiles {
    const message = createBaseReadAllFiles();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.include_hidden = object.include_hidden ?? false;
    return message;
  },
};

function createBaseFileAction(): FileAction {
  return {
    read_dir: undefined,
    send: undefined,
    receive: undefined,
    create: undefined,
    remove_dir: undefined,
    remove_file: undefined,
    all_files: undefined,
    cancel: undefined,
  };
}

export const FileAction = {
  encode(
    message: FileAction,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.read_dir !== undefined) {
      ReadDir.encode(message.read_dir, writer.uint32(10).fork()).ldelim();
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
    if (message.remove_dir !== undefined) {
      FileRemoveDir.encode(
        message.remove_dir,
        writer.uint32(42).fork()
      ).ldelim();
    }
    if (message.remove_file !== undefined) {
      FileRemoveFile.encode(
        message.remove_file,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.all_files !== undefined) {
      ReadAllFiles.encode(message.all_files, writer.uint32(58).fork()).ldelim();
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
          message.read_dir = ReadDir.decode(reader, reader.uint32());
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
          message.remove_dir = FileRemoveDir.decode(reader, reader.uint32());
          break;
        case 6:
          message.remove_file = FileRemoveFile.decode(reader, reader.uint32());
          break;
        case 7:
          message.all_files = ReadAllFiles.decode(reader, reader.uint32());
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
      read_dir: isSet(object.read_dir)
        ? ReadDir.fromJSON(object.read_dir)
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
      remove_dir: isSet(object.remove_dir)
        ? FileRemoveDir.fromJSON(object.remove_dir)
        : undefined,
      remove_file: isSet(object.remove_file)
        ? FileRemoveFile.fromJSON(object.remove_file)
        : undefined,
      all_files: isSet(object.all_files)
        ? ReadAllFiles.fromJSON(object.all_files)
        : undefined,
      cancel: isSet(object.cancel)
        ? FileTransferCancel.fromJSON(object.cancel)
        : undefined,
    };
  },

  toJSON(message: FileAction): unknown {
    const obj: any = {};
    message.read_dir !== undefined &&
      (obj.read_dir = message.read_dir
        ? ReadDir.toJSON(message.read_dir)
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
    message.remove_dir !== undefined &&
      (obj.remove_dir = message.remove_dir
        ? FileRemoveDir.toJSON(message.remove_dir)
        : undefined);
    message.remove_file !== undefined &&
      (obj.remove_file = message.remove_file
        ? FileRemoveFile.toJSON(message.remove_file)
        : undefined);
    message.all_files !== undefined &&
      (obj.all_files = message.all_files
        ? ReadAllFiles.toJSON(message.all_files)
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
    message.read_dir =
      object.read_dir !== undefined && object.read_dir !== null
        ? ReadDir.fromPartial(object.read_dir)
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
    message.remove_dir =
      object.remove_dir !== undefined && object.remove_dir !== null
        ? FileRemoveDir.fromPartial(object.remove_dir)
        : undefined;
    message.remove_file =
      object.remove_file !== undefined && object.remove_file !== null
        ? FileRemoveFile.fromPartial(object.remove_file)
        : undefined;
    message.all_files =
      object.all_files !== undefined && object.all_files !== null
        ? ReadAllFiles.fromPartial(object.all_files)
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
  return { id: 0, file_num: 0, data: new Uint8Array(), compressed: false };
}

export const FileTransferBlock = {
  encode(
    message: FileTransferBlock,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.file_num !== 0) {
      writer.uint32(16).sint32(message.file_num);
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
          message.file_num = reader.sint32();
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
      file_num: isSet(object.file_num) ? Number(object.file_num) : 0,
      data: isSet(object.data)
        ? bytesFromBase64(object.data)
        : new Uint8Array(),
      compressed: isSet(object.compressed) ? Boolean(object.compressed) : false,
    };
  },

  toJSON(message: FileTransferBlock): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.file_num !== undefined &&
      (obj.file_num = Math.round(message.file_num));
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
    message.file_num = object.file_num ?? 0;
    message.data = object.data ?? new Uint8Array();
    message.compressed = object.compressed ?? false;
    return message;
  },
};

function createBaseFileTransferError(): FileTransferError {
  return { id: 0, error: "", file_num: 0 };
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
    if (message.file_num !== 0) {
      writer.uint32(24).sint32(message.file_num);
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
          message.file_num = reader.sint32();
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
      file_num: isSet(object.file_num) ? Number(object.file_num) : 0,
    };
  },

  toJSON(message: FileTransferError): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.error !== undefined && (obj.error = message.error);
    message.file_num !== undefined &&
      (obj.file_num = Math.round(message.file_num));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferError>, I>>(
    object: I
  ): FileTransferError {
    const message = createBaseFileTransferError();
    message.id = object.id ?? 0;
    message.error = object.error ?? "";
    message.file_num = object.file_num ?? 0;
    return message;
  },
};

function createBaseFileTransferSendRequest(): FileTransferSendRequest {
  return { id: 0, path: "", include_hidden: false };
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
    if (message.include_hidden === true) {
      writer.uint32(24).bool(message.include_hidden);
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
          message.include_hidden = reader.bool();
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
      include_hidden: isSet(object.include_hidden)
        ? Boolean(object.include_hidden)
        : false,
    };
  },

  toJSON(message: FileTransferSendRequest): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.include_hidden !== undefined &&
      (obj.include_hidden = message.include_hidden);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferSendRequest>, I>>(
    object: I
  ): FileTransferSendRequest {
    const message = createBaseFileTransferSendRequest();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.include_hidden = object.include_hidden ?? false;
    return message;
  },
};

function createBaseFileTransferDone(): FileTransferDone {
  return { id: 0, file_num: 0 };
}

export const FileTransferDone = {
  encode(
    message: FileTransferDone,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.id !== 0) {
      writer.uint32(8).int32(message.id);
    }
    if (message.file_num !== 0) {
      writer.uint32(16).sint32(message.file_num);
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
          message.file_num = reader.sint32();
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
      file_num: isSet(object.file_num) ? Number(object.file_num) : 0,
    };
  },

  toJSON(message: FileTransferDone): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.file_num !== undefined &&
      (obj.file_num = Math.round(message.file_num));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileTransferDone>, I>>(
    object: I
  ): FileTransferDone {
    const message = createBaseFileTransferDone();
    message.id = object.id ?? 0;
    message.file_num = object.file_num ?? 0;
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
  return { id: 0, path: "", file_num: 0 };
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
    if (message.file_num !== 0) {
      writer.uint32(24).sint32(message.file_num);
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
          message.file_num = reader.sint32();
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
      file_num: isSet(object.file_num) ? Number(object.file_num) : 0,
    };
  },

  toJSON(message: FileRemoveFile): unknown {
    const obj: any = {};
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.path !== undefined && (obj.path = message.path);
    message.file_num !== undefined &&
      (obj.file_num = Math.round(message.file_num));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<FileRemoveFile>, I>>(
    object: I
  ): FileRemoveFile {
    const message = createBaseFileRemoveFile();
    message.id = object.id ?? 0;
    message.path = object.path ?? "";
    message.file_num = object.file_num ?? 0;
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

function createBaseCliprdrMonitorReady(): CliprdrMonitorReady {
  return { conn_id: 0 };
}

export const CliprdrMonitorReady = {
  encode(
    message: CliprdrMonitorReady,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): CliprdrMonitorReady {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrMonitorReady();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrMonitorReady {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
    };
  },

  toJSON(message: CliprdrMonitorReady): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrMonitorReady>, I>>(
    object: I
  ): CliprdrMonitorReady {
    const message = createBaseCliprdrMonitorReady();
    message.conn_id = object.conn_id ?? 0;
    return message;
  },
};

function createBaseCliprdrFormat(): CliprdrFormat {
  return { conn_id: 0, id: 0, format: "" };
}

export const CliprdrFormat = {
  encode(
    message: CliprdrFormat,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.id !== 0) {
      writer.uint32(16).int32(message.id);
    }
    if (message.format !== "") {
      writer.uint32(26).string(message.format);
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): CliprdrFormat {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrFormat();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.id = reader.int32();
          break;
        case 3:
          message.format = reader.string();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrFormat {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      id: isSet(object.id) ? Number(object.id) : 0,
      format: isSet(object.format) ? String(object.format) : "",
    };
  },

  toJSON(message: CliprdrFormat): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.id !== undefined && (obj.id = Math.round(message.id));
    message.format !== undefined && (obj.format = message.format);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrFormat>, I>>(
    object: I
  ): CliprdrFormat {
    const message = createBaseCliprdrFormat();
    message.conn_id = object.conn_id ?? 0;
    message.id = object.id ?? 0;
    message.format = object.format ?? "";
    return message;
  },
};

function createBaseCliprdrServerFormatList(): CliprdrServerFormatList {
  return { conn_id: 0, formats: [] };
}

export const CliprdrServerFormatList = {
  encode(
    message: CliprdrServerFormatList,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    for (const v of message.formats) {
      CliprdrFormat.encode(v!, writer.uint32(18).fork()).ldelim();
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrServerFormatList {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrServerFormatList();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.formats.push(CliprdrFormat.decode(reader, reader.uint32()));
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrServerFormatList {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      formats: Array.isArray(object?.formats)
        ? object.formats.map((e: any) => CliprdrFormat.fromJSON(e))
        : [],
    };
  },

  toJSON(message: CliprdrServerFormatList): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    if (message.formats) {
      obj.formats = message.formats.map((e) =>
        e ? CliprdrFormat.toJSON(e) : undefined
      );
    } else {
      obj.formats = [];
    }
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrServerFormatList>, I>>(
    object: I
  ): CliprdrServerFormatList {
    const message = createBaseCliprdrServerFormatList();
    message.conn_id = object.conn_id ?? 0;
    message.formats =
      object.formats?.map((e) => CliprdrFormat.fromPartial(e)) || [];
    return message;
  },
};

function createBaseCliprdrServerFormatListResponse(): CliprdrServerFormatListResponse {
  return { conn_id: 0, msg_flags: 0 };
}

export const CliprdrServerFormatListResponse = {
  encode(
    message: CliprdrServerFormatListResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.msg_flags !== 0) {
      writer.uint32(16).int32(message.msg_flags);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrServerFormatListResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrServerFormatListResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.msg_flags = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrServerFormatListResponse {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      msg_flags: isSet(object.msg_flags) ? Number(object.msg_flags) : 0,
    };
  },

  toJSON(message: CliprdrServerFormatListResponse): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.msg_flags !== undefined &&
      (obj.msg_flags = Math.round(message.msg_flags));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrServerFormatListResponse>, I>>(
    object: I
  ): CliprdrServerFormatListResponse {
    const message = createBaseCliprdrServerFormatListResponse();
    message.conn_id = object.conn_id ?? 0;
    message.msg_flags = object.msg_flags ?? 0;
    return message;
  },
};

function createBaseCliprdrServerFormatDataRequest(): CliprdrServerFormatDataRequest {
  return { conn_id: 0, requested_format_id: 0 };
}

export const CliprdrServerFormatDataRequest = {
  encode(
    message: CliprdrServerFormatDataRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.requested_format_id !== 0) {
      writer.uint32(16).int32(message.requested_format_id);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrServerFormatDataRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrServerFormatDataRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.requested_format_id = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrServerFormatDataRequest {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      requested_format_id: isSet(object.requested_format_id)
        ? Number(object.requested_format_id)
        : 0,
    };
  },

  toJSON(message: CliprdrServerFormatDataRequest): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.requested_format_id !== undefined &&
      (obj.requested_format_id = Math.round(message.requested_format_id));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrServerFormatDataRequest>, I>>(
    object: I
  ): CliprdrServerFormatDataRequest {
    const message = createBaseCliprdrServerFormatDataRequest();
    message.conn_id = object.conn_id ?? 0;
    message.requested_format_id = object.requested_format_id ?? 0;
    return message;
  },
};

function createBaseCliprdrServerFormatDataResponse(): CliprdrServerFormatDataResponse {
  return { conn_id: 0, msg_flags: 0, format_data: new Uint8Array() };
}

export const CliprdrServerFormatDataResponse = {
  encode(
    message: CliprdrServerFormatDataResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.msg_flags !== 0) {
      writer.uint32(16).int32(message.msg_flags);
    }
    if (message.format_data.length !== 0) {
      writer.uint32(26).bytes(message.format_data);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrServerFormatDataResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrServerFormatDataResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.msg_flags = reader.int32();
          break;
        case 3:
          message.format_data = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrServerFormatDataResponse {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      msg_flags: isSet(object.msg_flags) ? Number(object.msg_flags) : 0,
      format_data: isSet(object.format_data)
        ? bytesFromBase64(object.format_data)
        : new Uint8Array(),
    };
  },

  toJSON(message: CliprdrServerFormatDataResponse): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.msg_flags !== undefined &&
      (obj.msg_flags = Math.round(message.msg_flags));
    message.format_data !== undefined &&
      (obj.format_data = base64FromBytes(
        message.format_data !== undefined
          ? message.format_data
          : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrServerFormatDataResponse>, I>>(
    object: I
  ): CliprdrServerFormatDataResponse {
    const message = createBaseCliprdrServerFormatDataResponse();
    message.conn_id = object.conn_id ?? 0;
    message.msg_flags = object.msg_flags ?? 0;
    message.format_data = object.format_data ?? new Uint8Array();
    return message;
  },
};

function createBaseCliprdrFileContentsRequest(): CliprdrFileContentsRequest {
  return {
    conn_id: 0,
    stream_id: 0,
    list_index: 0,
    dw_flags: 0,
    n_position_low: 0,
    n_position_high: 0,
    cb_requested: 0,
    have_clip_data_id: false,
    clip_data_id: 0,
  };
}

export const CliprdrFileContentsRequest = {
  encode(
    message: CliprdrFileContentsRequest,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.stream_id !== 0) {
      writer.uint32(16).int32(message.stream_id);
    }
    if (message.list_index !== 0) {
      writer.uint32(24).int32(message.list_index);
    }
    if (message.dw_flags !== 0) {
      writer.uint32(32).int32(message.dw_flags);
    }
    if (message.n_position_low !== 0) {
      writer.uint32(40).int32(message.n_position_low);
    }
    if (message.n_position_high !== 0) {
      writer.uint32(48).int32(message.n_position_high);
    }
    if (message.cb_requested !== 0) {
      writer.uint32(56).int32(message.cb_requested);
    }
    if (message.have_clip_data_id === true) {
      writer.uint32(64).bool(message.have_clip_data_id);
    }
    if (message.clip_data_id !== 0) {
      writer.uint32(72).int32(message.clip_data_id);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrFileContentsRequest {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrFileContentsRequest();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 2:
          message.stream_id = reader.int32();
          break;
        case 3:
          message.list_index = reader.int32();
          break;
        case 4:
          message.dw_flags = reader.int32();
          break;
        case 5:
          message.n_position_low = reader.int32();
          break;
        case 6:
          message.n_position_high = reader.int32();
          break;
        case 7:
          message.cb_requested = reader.int32();
          break;
        case 8:
          message.have_clip_data_id = reader.bool();
          break;
        case 9:
          message.clip_data_id = reader.int32();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrFileContentsRequest {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      stream_id: isSet(object.stream_id) ? Number(object.stream_id) : 0,
      list_index: isSet(object.list_index) ? Number(object.list_index) : 0,
      dw_flags: isSet(object.dw_flags) ? Number(object.dw_flags) : 0,
      n_position_low: isSet(object.n_position_low)
        ? Number(object.n_position_low)
        : 0,
      n_position_high: isSet(object.n_position_high)
        ? Number(object.n_position_high)
        : 0,
      cb_requested: isSet(object.cb_requested)
        ? Number(object.cb_requested)
        : 0,
      have_clip_data_id: isSet(object.have_clip_data_id)
        ? Boolean(object.have_clip_data_id)
        : false,
      clip_data_id: isSet(object.clip_data_id)
        ? Number(object.clip_data_id)
        : 0,
    };
  },

  toJSON(message: CliprdrFileContentsRequest): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.stream_id !== undefined &&
      (obj.stream_id = Math.round(message.stream_id));
    message.list_index !== undefined &&
      (obj.list_index = Math.round(message.list_index));
    message.dw_flags !== undefined &&
      (obj.dw_flags = Math.round(message.dw_flags));
    message.n_position_low !== undefined &&
      (obj.n_position_low = Math.round(message.n_position_low));
    message.n_position_high !== undefined &&
      (obj.n_position_high = Math.round(message.n_position_high));
    message.cb_requested !== undefined &&
      (obj.cb_requested = Math.round(message.cb_requested));
    message.have_clip_data_id !== undefined &&
      (obj.have_clip_data_id = message.have_clip_data_id);
    message.clip_data_id !== undefined &&
      (obj.clip_data_id = Math.round(message.clip_data_id));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrFileContentsRequest>, I>>(
    object: I
  ): CliprdrFileContentsRequest {
    const message = createBaseCliprdrFileContentsRequest();
    message.conn_id = object.conn_id ?? 0;
    message.stream_id = object.stream_id ?? 0;
    message.list_index = object.list_index ?? 0;
    message.dw_flags = object.dw_flags ?? 0;
    message.n_position_low = object.n_position_low ?? 0;
    message.n_position_high = object.n_position_high ?? 0;
    message.cb_requested = object.cb_requested ?? 0;
    message.have_clip_data_id = object.have_clip_data_id ?? false;
    message.clip_data_id = object.clip_data_id ?? 0;
    return message;
  },
};

function createBaseCliprdrFileContentsResponse(): CliprdrFileContentsResponse {
  return {
    conn_id: 0,
    msg_flags: 0,
    stream_id: 0,
    requested_data: new Uint8Array(),
  };
}

export const CliprdrFileContentsResponse = {
  encode(
    message: CliprdrFileContentsResponse,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.conn_id !== 0) {
      writer.uint32(8).int32(message.conn_id);
    }
    if (message.msg_flags !== 0) {
      writer.uint32(24).int32(message.msg_flags);
    }
    if (message.stream_id !== 0) {
      writer.uint32(32).int32(message.stream_id);
    }
    if (message.requested_data.length !== 0) {
      writer.uint32(42).bytes(message.requested_data);
    }
    return writer;
  },

  decode(
    input: _m0.Reader | Uint8Array,
    length?: number
  ): CliprdrFileContentsResponse {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdrFileContentsResponse();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.conn_id = reader.int32();
          break;
        case 3:
          message.msg_flags = reader.int32();
          break;
        case 4:
          message.stream_id = reader.int32();
          break;
        case 5:
          message.requested_data = reader.bytes();
          break;
        default:
          reader.skipType(tag & 7);
          break;
      }
    }
    return message;
  },

  fromJSON(object: any): CliprdrFileContentsResponse {
    return {
      conn_id: isSet(object.conn_id) ? Number(object.conn_id) : 0,
      msg_flags: isSet(object.msg_flags) ? Number(object.msg_flags) : 0,
      stream_id: isSet(object.stream_id) ? Number(object.stream_id) : 0,
      requested_data: isSet(object.requested_data)
        ? bytesFromBase64(object.requested_data)
        : new Uint8Array(),
    };
  },

  toJSON(message: CliprdrFileContentsResponse): unknown {
    const obj: any = {};
    message.conn_id !== undefined &&
      (obj.conn_id = Math.round(message.conn_id));
    message.msg_flags !== undefined &&
      (obj.msg_flags = Math.round(message.msg_flags));
    message.stream_id !== undefined &&
      (obj.stream_id = Math.round(message.stream_id));
    message.requested_data !== undefined &&
      (obj.requested_data = base64FromBytes(
        message.requested_data !== undefined
          ? message.requested_data
          : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<CliprdrFileContentsResponse>, I>>(
    object: I
  ): CliprdrFileContentsResponse {
    const message = createBaseCliprdrFileContentsResponse();
    message.conn_id = object.conn_id ?? 0;
    message.msg_flags = object.msg_flags ?? 0;
    message.stream_id = object.stream_id ?? 0;
    message.requested_data = object.requested_data ?? new Uint8Array();
    return message;
  },
};

function createBaseCliprdr(): Cliprdr {
  return {
    ready: undefined,
    format_list: undefined,
    format_list_response: undefined,
    format_data_request: undefined,
    format_data_response: undefined,
    file_contents_request: undefined,
    file_contents_response: undefined,
  };
}

export const Cliprdr = {
  encode(
    message: Cliprdr,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.ready !== undefined) {
      CliprdrMonitorReady.encode(
        message.ready,
        writer.uint32(10).fork()
      ).ldelim();
    }
    if (message.format_list !== undefined) {
      CliprdrServerFormatList.encode(
        message.format_list,
        writer.uint32(18).fork()
      ).ldelim();
    }
    if (message.format_list_response !== undefined) {
      CliprdrServerFormatListResponse.encode(
        message.format_list_response,
        writer.uint32(26).fork()
      ).ldelim();
    }
    if (message.format_data_request !== undefined) {
      CliprdrServerFormatDataRequest.encode(
        message.format_data_request,
        writer.uint32(34).fork()
      ).ldelim();
    }
    if (message.format_data_response !== undefined) {
      CliprdrServerFormatDataResponse.encode(
        message.format_data_response,
        writer.uint32(42).fork()
      ).ldelim();
    }
    if (message.file_contents_request !== undefined) {
      CliprdrFileContentsRequest.encode(
        message.file_contents_request,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.file_contents_response !== undefined) {
      CliprdrFileContentsResponse.encode(
        message.file_contents_response,
        writer.uint32(58).fork()
      ).ldelim();
    }
    return writer;
  },

  decode(input: _m0.Reader | Uint8Array, length?: number): Cliprdr {
    const reader = input instanceof _m0.Reader ? input : new _m0.Reader(input);
    let end = length === undefined ? reader.len : reader.pos + length;
    const message = createBaseCliprdr();
    while (reader.pos < end) {
      const tag = reader.uint32();
      switch (tag >>> 3) {
        case 1:
          message.ready = CliprdrMonitorReady.decode(reader, reader.uint32());
          break;
        case 2:
          message.format_list = CliprdrServerFormatList.decode(
            reader,
            reader.uint32()
          );
          break;
        case 3:
          message.format_list_response = CliprdrServerFormatListResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 4:
          message.format_data_request = CliprdrServerFormatDataRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 5:
          message.format_data_response = CliprdrServerFormatDataResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 6:
          message.file_contents_request = CliprdrFileContentsRequest.decode(
            reader,
            reader.uint32()
          );
          break;
        case 7:
          message.file_contents_response = CliprdrFileContentsResponse.decode(
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

  fromJSON(object: any): Cliprdr {
    return {
      ready: isSet(object.ready)
        ? CliprdrMonitorReady.fromJSON(object.ready)
        : undefined,
      format_list: isSet(object.format_list)
        ? CliprdrServerFormatList.fromJSON(object.format_list)
        : undefined,
      format_list_response: isSet(object.format_list_response)
        ? CliprdrServerFormatListResponse.fromJSON(object.format_list_response)
        : undefined,
      format_data_request: isSet(object.format_data_request)
        ? CliprdrServerFormatDataRequest.fromJSON(object.format_data_request)
        : undefined,
      format_data_response: isSet(object.format_data_response)
        ? CliprdrServerFormatDataResponse.fromJSON(object.format_data_response)
        : undefined,
      file_contents_request: isSet(object.file_contents_request)
        ? CliprdrFileContentsRequest.fromJSON(object.file_contents_request)
        : undefined,
      file_contents_response: isSet(object.file_contents_response)
        ? CliprdrFileContentsResponse.fromJSON(object.file_contents_response)
        : undefined,
    };
  },

  toJSON(message: Cliprdr): unknown {
    const obj: any = {};
    message.ready !== undefined &&
      (obj.ready = message.ready
        ? CliprdrMonitorReady.toJSON(message.ready)
        : undefined);
    message.format_list !== undefined &&
      (obj.format_list = message.format_list
        ? CliprdrServerFormatList.toJSON(message.format_list)
        : undefined);
    message.format_list_response !== undefined &&
      (obj.format_list_response = message.format_list_response
        ? CliprdrServerFormatListResponse.toJSON(message.format_list_response)
        : undefined);
    message.format_data_request !== undefined &&
      (obj.format_data_request = message.format_data_request
        ? CliprdrServerFormatDataRequest.toJSON(message.format_data_request)
        : undefined);
    message.format_data_response !== undefined &&
      (obj.format_data_response = message.format_data_response
        ? CliprdrServerFormatDataResponse.toJSON(message.format_data_response)
        : undefined);
    message.file_contents_request !== undefined &&
      (obj.file_contents_request = message.file_contents_request
        ? CliprdrFileContentsRequest.toJSON(message.file_contents_request)
        : undefined);
    message.file_contents_response !== undefined &&
      (obj.file_contents_response = message.file_contents_response
        ? CliprdrFileContentsResponse.toJSON(message.file_contents_response)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Cliprdr>, I>>(object: I): Cliprdr {
    const message = createBaseCliprdr();
    message.ready =
      object.ready !== undefined && object.ready !== null
        ? CliprdrMonitorReady.fromPartial(object.ready)
        : undefined;
    message.format_list =
      object.format_list !== undefined && object.format_list !== null
        ? CliprdrServerFormatList.fromPartial(object.format_list)
        : undefined;
    message.format_list_response =
      object.format_list_response !== undefined &&
      object.format_list_response !== null
        ? CliprdrServerFormatListResponse.fromPartial(
            object.format_list_response
          )
        : undefined;
    message.format_data_request =
      object.format_data_request !== undefined &&
      object.format_data_request !== null
        ? CliprdrServerFormatDataRequest.fromPartial(object.format_data_request)
        : undefined;
    message.format_data_response =
      object.format_data_response !== undefined &&
      object.format_data_response !== null
        ? CliprdrServerFormatDataResponse.fromPartial(
            object.format_data_response
          )
        : undefined;
    message.file_contents_request =
      object.file_contents_request !== undefined &&
      object.file_contents_request !== null
        ? CliprdrFileContentsRequest.fromPartial(object.file_contents_request)
        : undefined;
    message.file_contents_response =
      object.file_contents_response !== undefined &&
      object.file_contents_response !== null
        ? CliprdrFileContentsResponse.fromPartial(object.file_contents_response)
        : undefined;
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
    image_quality: 0,
    lock_after_session_end: 0,
    show_remote_cursor: 0,
    privacy_mode: 0,
    block_input: 0,
    custom_image_quality: 0,
    disable_audio: 0,
    disable_clipboard: 0,
    enable_file_transfer: 0,
  };
}

export const OptionMessage = {
  encode(
    message: OptionMessage,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.image_quality !== 0) {
      writer.uint32(8).int32(message.image_quality);
    }
    if (message.lock_after_session_end !== 0) {
      writer.uint32(16).int32(message.lock_after_session_end);
    }
    if (message.show_remote_cursor !== 0) {
      writer.uint32(24).int32(message.show_remote_cursor);
    }
    if (message.privacy_mode !== 0) {
      writer.uint32(32).int32(message.privacy_mode);
    }
    if (message.block_input !== 0) {
      writer.uint32(40).int32(message.block_input);
    }
    if (message.custom_image_quality !== 0) {
      writer.uint32(48).int32(message.custom_image_quality);
    }
    if (message.disable_audio !== 0) {
      writer.uint32(56).int32(message.disable_audio);
    }
    if (message.disable_clipboard !== 0) {
      writer.uint32(64).int32(message.disable_clipboard);
    }
    if (message.enable_file_transfer !== 0) {
      writer.uint32(72).int32(message.enable_file_transfer);
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
          message.image_quality = reader.int32() as any;
          break;
        case 2:
          message.lock_after_session_end = reader.int32() as any;
          break;
        case 3:
          message.show_remote_cursor = reader.int32() as any;
          break;
        case 4:
          message.privacy_mode = reader.int32() as any;
          break;
        case 5:
          message.block_input = reader.int32() as any;
          break;
        case 6:
          message.custom_image_quality = reader.int32();
          break;
        case 7:
          message.disable_audio = reader.int32() as any;
          break;
        case 8:
          message.disable_clipboard = reader.int32() as any;
          break;
        case 9:
          message.enable_file_transfer = reader.int32() as any;
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
      image_quality: isSet(object.image_quality)
        ? imageQualityFromJSON(object.image_quality)
        : 0,
      lock_after_session_end: isSet(object.lock_after_session_end)
        ? optionMessage_BoolOptionFromJSON(object.lock_after_session_end)
        : 0,
      show_remote_cursor: isSet(object.show_remote_cursor)
        ? optionMessage_BoolOptionFromJSON(object.show_remote_cursor)
        : 0,
      privacy_mode: isSet(object.privacy_mode)
        ? optionMessage_BoolOptionFromJSON(object.privacy_mode)
        : 0,
      block_input: isSet(object.block_input)
        ? optionMessage_BoolOptionFromJSON(object.block_input)
        : 0,
      custom_image_quality: isSet(object.custom_image_quality)
        ? Number(object.custom_image_quality)
        : 0,
      disable_audio: isSet(object.disable_audio)
        ? optionMessage_BoolOptionFromJSON(object.disable_audio)
        : 0,
      disable_clipboard: isSet(object.disable_clipboard)
        ? optionMessage_BoolOptionFromJSON(object.disable_clipboard)
        : 0,
      enable_file_transfer: isSet(object.enable_file_transfer)
        ? optionMessage_BoolOptionFromJSON(object.enable_file_transfer)
        : 0,
    };
  },

  toJSON(message: OptionMessage): unknown {
    const obj: any = {};
    message.image_quality !== undefined &&
      (obj.image_quality = imageQualityToJSON(message.image_quality));
    message.lock_after_session_end !== undefined &&
      (obj.lock_after_session_end = optionMessage_BoolOptionToJSON(
        message.lock_after_session_end
      ));
    message.show_remote_cursor !== undefined &&
      (obj.show_remote_cursor = optionMessage_BoolOptionToJSON(
        message.show_remote_cursor
      ));
    message.privacy_mode !== undefined &&
      (obj.privacy_mode = optionMessage_BoolOptionToJSON(message.privacy_mode));
    message.block_input !== undefined &&
      (obj.block_input = optionMessage_BoolOptionToJSON(message.block_input));
    message.custom_image_quality !== undefined &&
      (obj.custom_image_quality = Math.round(message.custom_image_quality));
    message.disable_audio !== undefined &&
      (obj.disable_audio = optionMessage_BoolOptionToJSON(
        message.disable_audio
      ));
    message.disable_clipboard !== undefined &&
      (obj.disable_clipboard = optionMessage_BoolOptionToJSON(
        message.disable_clipboard
      ));
    message.enable_file_transfer !== undefined &&
      (obj.enable_file_transfer = optionMessage_BoolOptionToJSON(
        message.enable_file_transfer
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<OptionMessage>, I>>(
    object: I
  ): OptionMessage {
    const message = createBaseOptionMessage();
    message.image_quality = object.image_quality ?? 0;
    message.lock_after_session_end = object.lock_after_session_end ?? 0;
    message.show_remote_cursor = object.show_remote_cursor ?? 0;
    message.privacy_mode = object.privacy_mode ?? 0;
    message.block_input = object.block_input ?? 0;
    message.custom_image_quality = object.custom_image_quality ?? 0;
    message.disable_audio = object.disable_audio ?? 0;
    message.disable_clipboard = object.disable_clipboard ?? 0;
    message.enable_file_transfer = object.enable_file_transfer ?? 0;
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
  return { time: 0, from_client: false };
}

export const TestDelay = {
  encode(
    message: TestDelay,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.time !== 0) {
      writer.uint32(8).int64(message.time);
    }
    if (message.from_client === true) {
      writer.uint32(16).bool(message.from_client);
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
          message.from_client = reader.bool();
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
      from_client: isSet(object.from_client)
        ? Boolean(object.from_client)
        : false,
    };
  },

  toJSON(message: TestDelay): unknown {
    const obj: any = {};
    message.time !== undefined && (obj.time = Math.round(message.time));
    message.from_client !== undefined &&
      (obj.from_client = message.from_client);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<TestDelay>, I>>(
    object: I
  ): TestDelay {
    const message = createBaseTestDelay();
    message.time = object.time ?? 0;
    message.from_client = object.from_client ?? false;
    return message;
  },
};

function createBasePublicKey(): PublicKey {
  return {
    asymmetric_value: new Uint8Array(),
    symmetric_value: new Uint8Array(),
  };
}

export const PublicKey = {
  encode(
    message: PublicKey,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.asymmetric_value.length !== 0) {
      writer.uint32(10).bytes(message.asymmetric_value);
    }
    if (message.symmetric_value.length !== 0) {
      writer.uint32(18).bytes(message.symmetric_value);
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
          message.asymmetric_value = reader.bytes();
          break;
        case 2:
          message.symmetric_value = reader.bytes();
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
      asymmetric_value: isSet(object.asymmetric_value)
        ? bytesFromBase64(object.asymmetric_value)
        : new Uint8Array(),
      symmetric_value: isSet(object.symmetric_value)
        ? bytesFromBase64(object.symmetric_value)
        : new Uint8Array(),
    };
  },

  toJSON(message: PublicKey): unknown {
    const obj: any = {};
    message.asymmetric_value !== undefined &&
      (obj.asymmetric_value = base64FromBytes(
        message.asymmetric_value !== undefined
          ? message.asymmetric_value
          : new Uint8Array()
      ));
    message.symmetric_value !== undefined &&
      (obj.symmetric_value = base64FromBytes(
        message.symmetric_value !== undefined
          ? message.symmetric_value
          : new Uint8Array()
      ));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<PublicKey>, I>>(
    object: I
  ): PublicKey {
    const message = createBasePublicKey();
    message.asymmetric_value = object.asymmetric_value ?? new Uint8Array();
    message.symmetric_value = object.symmetric_value ?? new Uint8Array();
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
  return { sample_rate: 0, channels: 0 };
}

export const AudioFormat = {
  encode(
    message: AudioFormat,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.sample_rate !== 0) {
      writer.uint32(8).uint32(message.sample_rate);
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
          message.sample_rate = reader.uint32();
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
      sample_rate: isSet(object.sample_rate) ? Number(object.sample_rate) : 0,
      channels: isSet(object.channels) ? Number(object.channels) : 0,
    };
  },

  toJSON(message: AudioFormat): unknown {
    const obj: any = {};
    message.sample_rate !== undefined &&
      (obj.sample_rate = Math.round(message.sample_rate));
    message.channels !== undefined &&
      (obj.channels = Math.round(message.channels));
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<AudioFormat>, I>>(
    object: I
  ): AudioFormat {
    const message = createBaseAudioFormat();
    message.sample_rate = object.sample_rate ?? 0;
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
    chat_message: undefined,
    switch_display: undefined,
    permission_info: undefined,
    option: undefined,
    audio_format: undefined,
    close_reason: undefined,
    refresh_video: undefined,
    option_response: undefined,
    video_received: undefined,
  };
}

export const Misc = {
  encode(message: Misc, writer: _m0.Writer = _m0.Writer.create()): _m0.Writer {
    if (message.chat_message !== undefined) {
      ChatMessage.encode(
        message.chat_message,
        writer.uint32(34).fork()
      ).ldelim();
    }
    if (message.switch_display !== undefined) {
      SwitchDisplay.encode(
        message.switch_display,
        writer.uint32(42).fork()
      ).ldelim();
    }
    if (message.permission_info !== undefined) {
      PermissionInfo.encode(
        message.permission_info,
        writer.uint32(50).fork()
      ).ldelim();
    }
    if (message.option !== undefined) {
      OptionMessage.encode(message.option, writer.uint32(58).fork()).ldelim();
    }
    if (message.audio_format !== undefined) {
      AudioFormat.encode(
        message.audio_format,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.close_reason !== undefined) {
      writer.uint32(74).string(message.close_reason);
    }
    if (message.refresh_video !== undefined) {
      writer.uint32(80).bool(message.refresh_video);
    }
    if (message.option_response !== undefined) {
      OptionResponse.encode(
        message.option_response,
        writer.uint32(90).fork()
      ).ldelim();
    }
    if (message.video_received !== undefined) {
      writer.uint32(96).bool(message.video_received);
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
          message.chat_message = ChatMessage.decode(reader, reader.uint32());
          break;
        case 5:
          message.switch_display = SwitchDisplay.decode(
            reader,
            reader.uint32()
          );
          break;
        case 6:
          message.permission_info = PermissionInfo.decode(
            reader,
            reader.uint32()
          );
          break;
        case 7:
          message.option = OptionMessage.decode(reader, reader.uint32());
          break;
        case 8:
          message.audio_format = AudioFormat.decode(reader, reader.uint32());
          break;
        case 9:
          message.close_reason = reader.string();
          break;
        case 10:
          message.refresh_video = reader.bool();
          break;
        case 11:
          message.option_response = OptionResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 12:
          message.video_received = reader.bool();
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
      chat_message: isSet(object.chat_message)
        ? ChatMessage.fromJSON(object.chat_message)
        : undefined,
      switch_display: isSet(object.switch_display)
        ? SwitchDisplay.fromJSON(object.switch_display)
        : undefined,
      permission_info: isSet(object.permission_info)
        ? PermissionInfo.fromJSON(object.permission_info)
        : undefined,
      option: isSet(object.option)
        ? OptionMessage.fromJSON(object.option)
        : undefined,
      audio_format: isSet(object.audio_format)
        ? AudioFormat.fromJSON(object.audio_format)
        : undefined,
      close_reason: isSet(object.close_reason)
        ? String(object.close_reason)
        : undefined,
      refresh_video: isSet(object.refresh_video)
        ? Boolean(object.refresh_video)
        : undefined,
      option_response: isSet(object.option_response)
        ? OptionResponse.fromJSON(object.option_response)
        : undefined,
      video_received: isSet(object.video_received)
        ? Boolean(object.video_received)
        : undefined,
    };
  },

  toJSON(message: Misc): unknown {
    const obj: any = {};
    message.chat_message !== undefined &&
      (obj.chat_message = message.chat_message
        ? ChatMessage.toJSON(message.chat_message)
        : undefined);
    message.switch_display !== undefined &&
      (obj.switch_display = message.switch_display
        ? SwitchDisplay.toJSON(message.switch_display)
        : undefined);
    message.permission_info !== undefined &&
      (obj.permission_info = message.permission_info
        ? PermissionInfo.toJSON(message.permission_info)
        : undefined);
    message.option !== undefined &&
      (obj.option = message.option
        ? OptionMessage.toJSON(message.option)
        : undefined);
    message.audio_format !== undefined &&
      (obj.audio_format = message.audio_format
        ? AudioFormat.toJSON(message.audio_format)
        : undefined);
    message.close_reason !== undefined &&
      (obj.close_reason = message.close_reason);
    message.refresh_video !== undefined &&
      (obj.refresh_video = message.refresh_video);
    message.option_response !== undefined &&
      (obj.option_response = message.option_response
        ? OptionResponse.toJSON(message.option_response)
        : undefined);
    message.video_received !== undefined &&
      (obj.video_received = message.video_received);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Misc>, I>>(object: I): Misc {
    const message = createBaseMisc();
    message.chat_message =
      object.chat_message !== undefined && object.chat_message !== null
        ? ChatMessage.fromPartial(object.chat_message)
        : undefined;
    message.switch_display =
      object.switch_display !== undefined && object.switch_display !== null
        ? SwitchDisplay.fromPartial(object.switch_display)
        : undefined;
    message.permission_info =
      object.permission_info !== undefined && object.permission_info !== null
        ? PermissionInfo.fromPartial(object.permission_info)
        : undefined;
    message.option =
      object.option !== undefined && object.option !== null
        ? OptionMessage.fromPartial(object.option)
        : undefined;
    message.audio_format =
      object.audio_format !== undefined && object.audio_format !== null
        ? AudioFormat.fromPartial(object.audio_format)
        : undefined;
    message.close_reason = object.close_reason ?? undefined;
    message.refresh_video = object.refresh_video ?? undefined;
    message.option_response =
      object.option_response !== undefined && object.option_response !== null
        ? OptionResponse.fromPartial(object.option_response)
        : undefined;
    message.video_received = object.video_received ?? undefined;
    return message;
  },
};

function createBaseMessage(): Message {
  return {
    signed_id: undefined,
    public_key: undefined,
    test_delay: undefined,
    video_frame: undefined,
    login_request: undefined,
    login_response: undefined,
    hash: undefined,
    mouse_event: undefined,
    audio_frame: undefined,
    cursor_data: undefined,
    cursor_position: undefined,
    cursor_id: undefined,
    key_event: undefined,
    clipboard: undefined,
    file_action: undefined,
    file_response: undefined,
    misc: undefined,
    cliprdr: undefined,
  };
}

export const Message = {
  encode(
    message: Message,
    writer: _m0.Writer = _m0.Writer.create()
  ): _m0.Writer {
    if (message.signed_id !== undefined) {
      SignedId.encode(message.signed_id, writer.uint32(26).fork()).ldelim();
    }
    if (message.public_key !== undefined) {
      PublicKey.encode(message.public_key, writer.uint32(34).fork()).ldelim();
    }
    if (message.test_delay !== undefined) {
      TestDelay.encode(message.test_delay, writer.uint32(42).fork()).ldelim();
    }
    if (message.video_frame !== undefined) {
      VideoFrame.encode(message.video_frame, writer.uint32(50).fork()).ldelim();
    }
    if (message.login_request !== undefined) {
      LoginRequest.encode(
        message.login_request,
        writer.uint32(58).fork()
      ).ldelim();
    }
    if (message.login_response !== undefined) {
      LoginResponse.encode(
        message.login_response,
        writer.uint32(66).fork()
      ).ldelim();
    }
    if (message.hash !== undefined) {
      Hash.encode(message.hash, writer.uint32(74).fork()).ldelim();
    }
    if (message.mouse_event !== undefined) {
      MouseEvent.encode(message.mouse_event, writer.uint32(82).fork()).ldelim();
    }
    if (message.audio_frame !== undefined) {
      AudioFrame.encode(message.audio_frame, writer.uint32(90).fork()).ldelim();
    }
    if (message.cursor_data !== undefined) {
      CursorData.encode(message.cursor_data, writer.uint32(98).fork()).ldelim();
    }
    if (message.cursor_position !== undefined) {
      CursorPosition.encode(
        message.cursor_position,
        writer.uint32(106).fork()
      ).ldelim();
    }
    if (message.cursor_id !== undefined) {
      writer.uint32(112).uint64(message.cursor_id);
    }
    if (message.key_event !== undefined) {
      KeyEvent.encode(message.key_event, writer.uint32(122).fork()).ldelim();
    }
    if (message.clipboard !== undefined) {
      Clipboard.encode(message.clipboard, writer.uint32(130).fork()).ldelim();
    }
    if (message.file_action !== undefined) {
      FileAction.encode(
        message.file_action,
        writer.uint32(138).fork()
      ).ldelim();
    }
    if (message.file_response !== undefined) {
      FileResponse.encode(
        message.file_response,
        writer.uint32(146).fork()
      ).ldelim();
    }
    if (message.misc !== undefined) {
      Misc.encode(message.misc, writer.uint32(154).fork()).ldelim();
    }
    if (message.cliprdr !== undefined) {
      Cliprdr.encode(message.cliprdr, writer.uint32(162).fork()).ldelim();
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
          message.signed_id = SignedId.decode(reader, reader.uint32());
          break;
        case 4:
          message.public_key = PublicKey.decode(reader, reader.uint32());
          break;
        case 5:
          message.test_delay = TestDelay.decode(reader, reader.uint32());
          break;
        case 6:
          message.video_frame = VideoFrame.decode(reader, reader.uint32());
          break;
        case 7:
          message.login_request = LoginRequest.decode(reader, reader.uint32());
          break;
        case 8:
          message.login_response = LoginResponse.decode(
            reader,
            reader.uint32()
          );
          break;
        case 9:
          message.hash = Hash.decode(reader, reader.uint32());
          break;
        case 10:
          message.mouse_event = MouseEvent.decode(reader, reader.uint32());
          break;
        case 11:
          message.audio_frame = AudioFrame.decode(reader, reader.uint32());
          break;
        case 12:
          message.cursor_data = CursorData.decode(reader, reader.uint32());
          break;
        case 13:
          message.cursor_position = CursorPosition.decode(
            reader,
            reader.uint32()
          );
          break;
        case 14:
          message.cursor_id = longToNumber(reader.uint64() as Long);
          break;
        case 15:
          message.key_event = KeyEvent.decode(reader, reader.uint32());
          break;
        case 16:
          message.clipboard = Clipboard.decode(reader, reader.uint32());
          break;
        case 17:
          message.file_action = FileAction.decode(reader, reader.uint32());
          break;
        case 18:
          message.file_response = FileResponse.decode(reader, reader.uint32());
          break;
        case 19:
          message.misc = Misc.decode(reader, reader.uint32());
          break;
        case 20:
          message.cliprdr = Cliprdr.decode(reader, reader.uint32());
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
      signed_id: isSet(object.signed_id)
        ? SignedId.fromJSON(object.signed_id)
        : undefined,
      public_key: isSet(object.public_key)
        ? PublicKey.fromJSON(object.public_key)
        : undefined,
      test_delay: isSet(object.test_delay)
        ? TestDelay.fromJSON(object.test_delay)
        : undefined,
      video_frame: isSet(object.video_frame)
        ? VideoFrame.fromJSON(object.video_frame)
        : undefined,
      login_request: isSet(object.login_request)
        ? LoginRequest.fromJSON(object.login_request)
        : undefined,
      login_response: isSet(object.login_response)
        ? LoginResponse.fromJSON(object.login_response)
        : undefined,
      hash: isSet(object.hash) ? Hash.fromJSON(object.hash) : undefined,
      mouse_event: isSet(object.mouse_event)
        ? MouseEvent.fromJSON(object.mouse_event)
        : undefined,
      audio_frame: isSet(object.audio_frame)
        ? AudioFrame.fromJSON(object.audio_frame)
        : undefined,
      cursor_data: isSet(object.cursor_data)
        ? CursorData.fromJSON(object.cursor_data)
        : undefined,
      cursor_position: isSet(object.cursor_position)
        ? CursorPosition.fromJSON(object.cursor_position)
        : undefined,
      cursor_id: isSet(object.cursor_id) ? Number(object.cursor_id) : undefined,
      key_event: isSet(object.key_event)
        ? KeyEvent.fromJSON(object.key_event)
        : undefined,
      clipboard: isSet(object.clipboard)
        ? Clipboard.fromJSON(object.clipboard)
        : undefined,
      file_action: isSet(object.file_action)
        ? FileAction.fromJSON(object.file_action)
        : undefined,
      file_response: isSet(object.file_response)
        ? FileResponse.fromJSON(object.file_response)
        : undefined,
      misc: isSet(object.misc) ? Misc.fromJSON(object.misc) : undefined,
      cliprdr: isSet(object.cliprdr)
        ? Cliprdr.fromJSON(object.cliprdr)
        : undefined,
    };
  },

  toJSON(message: Message): unknown {
    const obj: any = {};
    message.signed_id !== undefined &&
      (obj.signed_id = message.signed_id
        ? SignedId.toJSON(message.signed_id)
        : undefined);
    message.public_key !== undefined &&
      (obj.public_key = message.public_key
        ? PublicKey.toJSON(message.public_key)
        : undefined);
    message.test_delay !== undefined &&
      (obj.test_delay = message.test_delay
        ? TestDelay.toJSON(message.test_delay)
        : undefined);
    message.video_frame !== undefined &&
      (obj.video_frame = message.video_frame
        ? VideoFrame.toJSON(message.video_frame)
        : undefined);
    message.login_request !== undefined &&
      (obj.login_request = message.login_request
        ? LoginRequest.toJSON(message.login_request)
        : undefined);
    message.login_response !== undefined &&
      (obj.login_response = message.login_response
        ? LoginResponse.toJSON(message.login_response)
        : undefined);
    message.hash !== undefined &&
      (obj.hash = message.hash ? Hash.toJSON(message.hash) : undefined);
    message.mouse_event !== undefined &&
      (obj.mouse_event = message.mouse_event
        ? MouseEvent.toJSON(message.mouse_event)
        : undefined);
    message.audio_frame !== undefined &&
      (obj.audio_frame = message.audio_frame
        ? AudioFrame.toJSON(message.audio_frame)
        : undefined);
    message.cursor_data !== undefined &&
      (obj.cursor_data = message.cursor_data
        ? CursorData.toJSON(message.cursor_data)
        : undefined);
    message.cursor_position !== undefined &&
      (obj.cursor_position = message.cursor_position
        ? CursorPosition.toJSON(message.cursor_position)
        : undefined);
    message.cursor_id !== undefined &&
      (obj.cursor_id = Math.round(message.cursor_id));
    message.key_event !== undefined &&
      (obj.key_event = message.key_event
        ? KeyEvent.toJSON(message.key_event)
        : undefined);
    message.clipboard !== undefined &&
      (obj.clipboard = message.clipboard
        ? Clipboard.toJSON(message.clipboard)
        : undefined);
    message.file_action !== undefined &&
      (obj.file_action = message.file_action
        ? FileAction.toJSON(message.file_action)
        : undefined);
    message.file_response !== undefined &&
      (obj.file_response = message.file_response
        ? FileResponse.toJSON(message.file_response)
        : undefined);
    message.misc !== undefined &&
      (obj.misc = message.misc ? Misc.toJSON(message.misc) : undefined);
    message.cliprdr !== undefined &&
      (obj.cliprdr = message.cliprdr
        ? Cliprdr.toJSON(message.cliprdr)
        : undefined);
    return obj;
  },

  fromPartial<I extends Exact<DeepPartial<Message>, I>>(object: I): Message {
    const message = createBaseMessage();
    message.signed_id =
      object.signed_id !== undefined && object.signed_id !== null
        ? SignedId.fromPartial(object.signed_id)
        : undefined;
    message.public_key =
      object.public_key !== undefined && object.public_key !== null
        ? PublicKey.fromPartial(object.public_key)
        : undefined;
    message.test_delay =
      object.test_delay !== undefined && object.test_delay !== null
        ? TestDelay.fromPartial(object.test_delay)
        : undefined;
    message.video_frame =
      object.video_frame !== undefined && object.video_frame !== null
        ? VideoFrame.fromPartial(object.video_frame)
        : undefined;
    message.login_request =
      object.login_request !== undefined && object.login_request !== null
        ? LoginRequest.fromPartial(object.login_request)
        : undefined;
    message.login_response =
      object.login_response !== undefined && object.login_response !== null
        ? LoginResponse.fromPartial(object.login_response)
        : undefined;
    message.hash =
      object.hash !== undefined && object.hash !== null
        ? Hash.fromPartial(object.hash)
        : undefined;
    message.mouse_event =
      object.mouse_event !== undefined && object.mouse_event !== null
        ? MouseEvent.fromPartial(object.mouse_event)
        : undefined;
    message.audio_frame =
      object.audio_frame !== undefined && object.audio_frame !== null
        ? AudioFrame.fromPartial(object.audio_frame)
        : undefined;
    message.cursor_data =
      object.cursor_data !== undefined && object.cursor_data !== null
        ? CursorData.fromPartial(object.cursor_data)
        : undefined;
    message.cursor_position =
      object.cursor_position !== undefined && object.cursor_position !== null
        ? CursorPosition.fromPartial(object.cursor_position)
        : undefined;
    message.cursor_id = object.cursor_id ?? undefined;
    message.key_event =
      object.key_event !== undefined && object.key_event !== null
        ? KeyEvent.fromPartial(object.key_event)
        : undefined;
    message.clipboard =
      object.clipboard !== undefined && object.clipboard !== null
        ? Clipboard.fromPartial(object.clipboard)
        : undefined;
    message.file_action =
      object.file_action !== undefined && object.file_action !== null
        ? FileAction.fromPartial(object.file_action)
        : undefined;
    message.file_response =
      object.file_response !== undefined && object.file_response !== null
        ? FileResponse.fromPartial(object.file_response)
        : undefined;
    message.misc =
      object.misc !== undefined && object.misc !== null
        ? Misc.fromPartial(object.misc)
        : undefined;
    message.cliprdr =
      object.cliprdr !== undefined && object.cliprdr !== null
        ? Cliprdr.fromPartial(object.cliprdr)
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
