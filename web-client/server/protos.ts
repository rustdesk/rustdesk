import protobuf from 'protobufjs';

/**
 * Canonical RustDesk wire-format Protobuf definitions verified directly against:
 * - rustdesk/rustdesk: libs/base/protos/message.proto
 * - rustdesk/hbb_common: protos/rendezvous.proto
 */
const PROTO_SOURCE = `
syntax = "proto3";
package rustdesk;

// ==================== message.proto ====================

enum KeyboardMode {
  Legacy = 0;
  Map = 1;
  Translate = 2;
  Auto = 3;
}

enum ControlKey {
  Unknown = 0;
  Alt = 1;
  Backspace = 2;
  CapsLock = 3;
  Control = 4;
  Delete = 5;
  DownArrow = 6;
  End = 7;
  Escape = 8;
  F1 = 9;
  F10 = 10;
  F11 = 11;
  F12 = 12;
  F2 = 13;
  F3 = 14;
  F4 = 15;
  F5 = 16;
  F6 = 17;
  F7 = 18;
  F8 = 19;
  F9 = 20;
  Home = 21;
  LeftArrow = 22;
  Meta = 23;
  Option = 24;
  PageDown = 25;
  PageUp = 26;
  Return = 27;
  RightArrow = 28;
  Shift = 29;
  Space = 30;
  Tab = 31;
  UpArrow = 32;
  Numpad0 = 33;
  Numpad1 = 34;
  Numpad2 = 35;
  Numpad3 = 36;
  Numpad4 = 37;
  Numpad5 = 38;
  Numpad6 = 39;
  Numpad7 = 40;
  Numpad8 = 41;
  Numpad9 = 42;
  Cancel = 43;
  Clear = 44;
  Menu = 45;
  Pause = 46;
  Kana = 47;
  Hangul = 48;
  Junja = 49;
  Final = 50;
  Hanja = 51;
  Kanji = 52;
  Convert = 53;
  Select = 54;
  Print = 55;
  Execute = 56;
  Snapshot = 57;
  Insert = 58;
  Help = 59;
  Sleep = 60;
  Separator = 61;
  Scroll = 62;
  NumLock = 63;
  RWin = 64;
  Apps = 65;
  Multiply = 66;
  Add = 67;
  Subtract = 68;
  Decimal = 69;
  Divide = 70;
  Equals = 71;
  NumpadEnter = 72;
  RShift = 73;
  RControl = 74;
  RAlt = 75;
  VolumeMute = 76;
  VolumeUp = 77;
  VolumeDown = 78;
  Power = 79;
  CtrlAltDel = 100;
  LockScreen = 101;
}

enum ImageQuality {
  NotSet = 0;
  Low = 2;
  Balanced = 3;
  Best = 4;
}

enum Chroma {
  I420 = 0;
  I444 = 1;
}

enum ClipboardFormat {
  Text = 0;
  Rtf = 1;
  Html = 2;
  ImageRgba = 21;
  ImagePng = 22;
  ImageSvg = 23;
  Special = 31;
}

message SignedId {
  bytes id = 1;
}

message IdPk {
  string id = 1;
  bytes pk = 2;
  string dtls_fingerprint = 3;
  uint32 kx_version = 4;
}

message PublicKey {
  bytes asymmetric_value = 1;
  bytes symmetric_value = 2;
  uint32 kx_version = 3;
}

message Hash {
  string salt = 1;
  string challenge = 2;
}

message CodecAbility {
  bool vp8 = 1;
  bool vp9 = 2;
  bool av1 = 3;
  bool h264 = 4;
  bool h265 = 5;
}

message SupportedDecoding {
  enum PreferCodec {
    Auto = 0;
    VP9 = 1;
    H264 = 2;
    H265 = 3;
    VP8 = 4;
    AV1 = 5;
  }
  int32 ability_vp9 = 1;
  int32 ability_h264 = 2;
  int32 ability_h265 = 3;
  PreferCodec prefer = 4;
  int32 ability_vp8 = 5;
  int32 ability_av1 = 6;
  CodecAbility i444 = 7;
  Chroma prefer_chroma = 8;
}

message OptionMessage {
  enum BoolOption {
    NotSet = 0;
    No = 1;
    Yes = 2;
  }
  ImageQuality image_quality = 1;
  BoolOption lock_after_session_end = 2;
  BoolOption show_remote_cursor = 3;
  BoolOption privacy_mode = 4;
  BoolOption block_input = 5;
  int32 custom_image_quality = 6;
  BoolOption disable_audio = 7;
  BoolOption disable_clipboard = 8;
  BoolOption enable_file_transfer = 9;
  SupportedDecoding supported_decoding = 10;
  int32 custom_fps = 11;
  BoolOption disable_keyboard = 12;
  BoolOption follow_remote_cursor = 15;
  BoolOption follow_remote_window = 16;
  BoolOption disable_camera = 17;
  BoolOption terminal_persistent = 18;
  BoolOption show_my_cursor = 19;
}

message Resolution {
  int32 width = 1;
  int32 height = 2;
}

message SupportedResolutions {
  repeated Resolution resolutions = 1;
}

message DisplayInfo {
  sint32 x = 1;
  sint32 y = 2;
  int32 width = 3;
  int32 height = 4;
  string name = 5;
  bool online = 6;
  bool cursor_embedded = 7;
  Resolution original_resolution = 8;
  double scale = 9;
}

message PortForward {
  string host = 1;
  int32 port = 2;
  bool multiplex = 3;
}

message FileTransfer {
  string dir = 1;
  bool show_hidden = 2;
}

message ViewCamera {}

message Terminal {
  string service_id = 1;
}

message OSLogin {
  string username = 1;
  string password = 2;
}

message LoginRequest {
  string username = 1;
  bytes password = 2;
  string my_id = 4;
  string my_name = 5;
  OptionMessage option = 6;
  oneof union {
    FileTransfer file_transfer = 7;
    PortForward port_forward = 8;
    ViewCamera view_camera = 15;
    Terminal terminal = 16;
  }
  bool video_ack_required = 9;
  uint64 session_id = 10;
  string version = 11;
  OSLogin os_login = 12;
  string my_platform = 13;
  bytes hwid = 14;
  string avatar = 17;
  bytes trusted_device_v2_token = 18;
  bool trusted_device_v2_supported = 19;
}

message Auth2FA {
  string code = 1;
  bytes hwid = 2;
  bool trust_this_device = 3;
}

message ChatMessage {
  string text = 1;
}

message Features {
  bool privacy_mode = 1;
  bool terminal = 2;
  bool port_forward_mux = 3;
}

message SupportedEncoding {
  bool h264 = 1;
  bool h265 = 2;
  bool vp8 = 3;
  bool av1 = 4;
  CodecAbility i444 = 5;
}

message PeerInfo {
  string username = 1;
  string hostname = 2;
  string platform = 3;
  repeated DisplayInfo displays = 4;
  int32 current_display = 5;
  bool sas_enabled = 6;
  string version = 7;
  Features features = 9;
  SupportedEncoding encoding = 10;
  SupportedResolutions resolutions = 11;
  string platform_additions = 12;
}

message LoginResponse {
  oneof union {
    string error = 1;
    PeerInfo peer_info = 2;
  }
  bool enable_trusted_devices = 3;
  bool trusted_device_v2_supported = 4;
  bytes trusted_device_v2_token = 5;
}

message EncodedVideoFrame {
  bytes data = 1;
  bool key = 2;
  int64 pts = 3;
}

message EncodedVideoFrames {
  repeated EncodedVideoFrame frames = 1;
}

message RGB {
  bool compress = 1;
}

message YUV {
  bool compress = 1;
  int32 stride = 2;
}

message VideoFrame {
  oneof union {
    EncodedVideoFrames vp9s = 6;
    RGB rgb = 7;
    YUV yuv = 8;
    EncodedVideoFrames h264s = 10;
    EncodedVideoFrames h265s = 11;
    EncodedVideoFrames vp8s = 12;
    EncodedVideoFrames av1s = 13;
  }
  int32 display = 14;
}

message MouseEvent {
  int32 mask = 1;
  sint32 x = 2;
  sint32 y = 3;
  repeated ControlKey modifiers = 4;
}

message KeyEvent {
  bool down = 1;
  bool press = 2;
  oneof union {
    ControlKey control_key = 3;
    uint32 chr = 4;
    uint32 unicode = 5;
    string seq = 6;
    uint32 win2win_hotkey = 7;
  }
  repeated ControlKey modifiers = 8;
  KeyboardMode mode = 9;
}

message Clipboard {
  bool compress = 1;
  bytes content = 2;
  int32 width = 3;
  int32 height = 4;
  ClipboardFormat format = 5;
  string special_name = 6;
}

message MultiClipboards {
  repeated Clipboard clipboards = 1;
}

message CursorData {
  uint64 id = 1;
  sint32 hotx = 2;
  sint32 hoty = 3;
  int32 width = 4;
  int32 height = 5;
  bytes colors = 6;
}

message CursorPosition {
  sint32 x = 1;
  sint32 y = 2;
}

message AudioFormat {
  uint32 sample_rate = 1;
  uint32 channels = 2;
}

message AudioFrame {
  bytes data = 1;
}

message MessageBox {
  string msgtype = 1;
  string title = 2;
  string text = 3;
  string link = 4;
}

message TestDelay {
  int64 time = 1;
  bool from_client = 2;
  uint32 last_delay = 3;
  uint32 target_bitrate = 4;
}

message SwitchDisplay {
  int32 display = 1;
  sint32 x = 2;
  sint32 y = 3;
  int32 width = 4;
  int32 height = 5;
  bool cursor_embedded = 6;
  SupportedResolutions resolutions = 7;
  Resolution original_resolution = 8;
}

message PermissionInfo {
  enum Permission {
    Keyboard = 0;
    Clipboard = 2;
    Audio = 3;
    File = 4;
    Restart = 5;
    Recording = 6;
    BlockInput = 7;
    PrivacyMode = 8;
  }
  Permission permission = 1;
  bool enabled = 2;
}

message Misc {
  oneof union {
    ChatMessage chat_message = 4;
    SwitchDisplay switch_display = 5;
    PermissionInfo permission_info = 6;
    OptionMessage option = 7;
    AudioFormat audio_format = 8;
    string close_reason = 9;
    bool refresh_video = 10;
    bool video_received = 12;
  }
}

message Message {
  oneof union {
    SignedId signed_id = 3;
    PublicKey public_key = 4;
    TestDelay test_delay = 5;
    VideoFrame video_frame = 6;
    LoginRequest login_request = 7;
    LoginResponse login_response = 8;
    Hash hash = 9;
    MouseEvent mouse_event = 10;
    AudioFrame audio_frame = 11;
    CursorData cursor_data = 12;
    CursorPosition cursor_position = 13;
    uint64 cursor_id = 14;
    KeyEvent key_event = 15;
    Clipboard clipboard = 16;
    Misc misc = 19;
    MessageBox message_box = 21;
    PeerInfo peer_info = 25;
    Auth2FA auth_2fa = 27;
    MultiClipboards multi_clipboards = 28;
  }
}

// ==================== rendezvous.proto ====================

enum NatType {
  UNKNOWN_NAT = 0;
  ASYMMETRIC = 1;
  SYMMETRIC = 2;
}

enum ConnType {
  DEFAULT_CONN = 0;
  FILE_TRANSFER = 1;
  PORT_FORWARD = 2;
  RDP = 3;
  VIEW_CAMERA = 4;
  TERMINAL = 5;
}

message PunchHoleRequest {
  string id = 1;
  NatType nat_type = 2;
  string licence_key = 3;
  ConnType conn_type = 4;
  string token = 5;
  string version = 6;
  int32 udp_port = 7;
  bool force_relay = 8;
  int32 upnp_port = 9;
  bytes socket_addr_v6 = 10;
  string switch_code = 11;
}

message PunchHoleSent {
  bytes socket_addr = 1;
  string id = 2;
  string relay_server = 3;
  NatType nat_type = 4;
  string version = 5;
}

message PunchHoleResponse {
  enum Failure {
    ID_NOT_EXIST = 0;
    OFFLINE = 2;
    LICENSE_MISMATCH = 3;
    LICENSE_OVERUSE = 4;
  }
  bytes socket_addr = 1;
  bytes pk = 2;
  Failure failure = 3;
  string relay_server = 4;
  oneof union {
    NatType nat_type = 5;
    bool is_local = 6;
  }
  string other_failure = 7;
  int32 feedback = 8;
  bool is_udp = 9;
  int32 upnp_port = 10;
  bytes socket_addr_v6 = 11;
}

message RequestRelay {
  string id = 1;
  string uuid = 2;
  bytes socket_addr = 3;
  string relay_server = 4;
  bool secure = 5;
  string licence_key = 6;
  ConnType conn_type = 7;
  string token = 8;
}

message RelayResponse {
  bytes socket_addr = 1;
  string uuid = 2;
  string relay_server = 3;
  oneof union {
    string id = 4;
    bytes pk = 5;
  }
  string refuse_reason = 6;
  string version = 7;
  int32 feedback = 9;
  bytes socket_addr_v6 = 10;
}

message KeyExchange {
  repeated bytes keys = 1;
  uint32 version = 2;
  bytes signed_params = 3;
}

message KxParams {
  bytes pk = 1;
  uint32 version = 2;
}

message RendezvousMessage {
  oneof union {
    PunchHoleRequest punch_hole_request = 8;
    PunchHoleSent punch_hole_sent = 10;
    PunchHoleResponse punch_hole_response = 11;
    RequestRelay request_relay = 18;
    RelayResponse relay_response = 19;
    KeyExchange key_exchange = 25;
  }
}
`;

const parsed = protobuf.parse(PROTO_SOURCE, { keepCase: true });
export const protoRoot = parsed.root;

export const MessageProto = protoRoot.lookupType('rustdesk.Message');
export const IdPkProto = protoRoot.lookupType('rustdesk.IdPk');
export const RendezvousMessageProto = protoRoot.lookupType('rustdesk.RendezvousMessage');
export const KxParamsProto = protoRoot.lookupType('rustdesk.KxParams');

export const ControlKeyValues = protoRoot.lookupEnum('rustdesk.ControlKey').values;
export const KeyboardModeValues = protoRoot.lookupEnum('rustdesk.KeyboardMode').values;
export const ImageQualityValues = protoRoot.lookupEnum('rustdesk.ImageQuality').values;
export const BoolOptionValues = protoRoot.lookupEnum('rustdesk.OptionMessage.BoolOption').values;
export const PreferCodecValues = protoRoot.lookupEnum('rustdesk.SupportedDecoding.PreferCodec').values;
export const ChromaValues = protoRoot.lookupEnum('rustdesk.Chroma').values;
export const ClipboardFormatValues = protoRoot.lookupEnum('rustdesk.ClipboardFormat').values;
export const NatTypeValues = protoRoot.lookupEnum('rustdesk.NatType').values;
export const ConnTypeValues = protoRoot.lookupEnum('rustdesk.ConnType').values;
export const PunchFailureValues = protoRoot.lookupEnum('rustdesk.PunchHoleResponse.Failure').values;

export function encodeMessage(obj: Record<string, unknown>): Buffer {
  const msg = MessageProto.create(obj);
  return Buffer.from(MessageProto.encode(msg).finish());
}

function wrapProtoDecodeError(err: unknown, buf: Uint8Array, kind: 'host' | 'id-server'): Error {
  const head = Buffer.from(buf.subarray(0, Math.min(buf.length, 32))).toString('ascii');
  if (/(?:HTTP\/\d\.\d|<!DOCTYPE|<html|Bad Request|Not Found)/i.test(head)) {
    return new Error(
      kind === 'id-server'
        ? 'Not a RustDesk ID server: received an HTTP response instead of RustDesk protocol data.'
        : 'Not a RustDesk server: received an HTTP response instead of RustDesk protocol data.',
    );
  }
  const msg = err instanceof Error ? err.message : String(err ?? '');
  if (/invalid wire type|index out of range|illegal tag|unexpected end/i.test(msg)) {
    return new Error(
      kind === 'id-server'
        ? 'Not a RustDesk ID server: received invalid protocol data.'
        : 'Not a RustDesk server: received invalid protocol data.',
    );
  }
  return err instanceof Error ? err : new Error(msg);
}

export function decodeMessage(buf: Uint8Array): Record<string, any> {
  try {
    const decoded = MessageProto.decode(buf);
    return MessageProto.toObject(decoded, {
      bytes: Buffer,
      longs: String,
      enums: Number,
      defaults: false,
      oneofs: true,
    });
  } catch (err) {
    throw wrapProtoDecodeError(err, buf, 'host');
  }
}

export function encodeRendezvous(obj: Record<string, unknown>): Buffer {
  const msg = RendezvousMessageProto.create(obj);
  return Buffer.from(RendezvousMessageProto.encode(msg).finish());
}

export function decodeRendezvous(buf: Uint8Array): Record<string, any> {
  try {
    const decoded = RendezvousMessageProto.decode(buf);
    return RendezvousMessageProto.toObject(decoded, {
      bytes: Buffer,
      longs: String,
      enums: Number,
      defaults: false,
      oneofs: true,
    });
  } catch (err) {
    throw wrapProtoDecodeError(err, buf, 'id-server');
  }
}

export function decodeIdPk(buf: Uint8Array): { id: string; pk: Uint8Array; kx_version: number } {
  const decoded = IdPkProto.decode(buf);
  const obj = IdPkProto.toObject(decoded, { bytes: Buffer, defaults: true });
  return {
    id: String(obj.id ?? ''),
    pk: new Uint8Array(obj.pk ?? []),
    kx_version: Number(obj.kx_version ?? 0),
  };
}

export function encodeIdPk(id: string, pk: Uint8Array, kxVersion: number): Buffer {
  return Buffer.from(
    IdPkProto.encode(IdPkProto.create({ id, pk, kx_version: kxVersion })).finish(),
  );
}

export function decodeKxParams(buf: Uint8Array): { pk: Uint8Array; version: number } {
  const decoded = KxParamsProto.decode(buf);
  const obj = KxParamsProto.toObject(decoded, { bytes: Buffer, defaults: true });
  return {
    pk: new Uint8Array(obj.pk ?? []),
    version: Number(obj.version ?? 0),
  };
}

/**
 * Encodes an IPv4 or IPv6 SocketAddr identically to hbb_common::AddrMangle::encode.
 */
export function encodeAddrMangle(host: string, port: number, tmOverride?: number): Buffer {
  const ipv4Parts = host.split('.');
  if (ipv4Parts.length === 4 && ipv4Parts.every((p) => /^\d+$/.test(p))) {
    const octets = ipv4Parts.map((p) => Number(p) & 0xff);
    const ip =
      BigInt(octets[0]) |
      (BigInt(octets[1]) << 8n) |
      (BigInt(octets[2]) << 16n) |
      (BigInt(octets[3]) << 24n);
    const tm = BigInt((tmOverride ?? (Date.now() * 1000)) >>> 0);
    const p = BigInt(port & 0xffff);
    const v = ((ip + tm) << 49n) | (tm << 17n) | (p + (tm & 0xffffn));
    const bytes = Buffer.alloc(16);
    for (let i = 0; i < 16; i++) {
      bytes[i] = Number((v >> BigInt(i * 8)) & 0xffn);
    }
    let end = 16;
    while (end > 0 && bytes[end - 1] === 0) {
      end--;
    }
    return bytes.subarray(0, end);
  }

  // IPv6: 16 raw bytes + 2-byte little-endian port
  const out = Buffer.alloc(18);
  const expanded = expandIpv6(host);
  expanded.copy(out, 0);
  out.writeUInt16LE(port & 0xffff, 16);
  return out;
}

function expandIpv6(ip: string): Buffer {
  const cleaned = ip.replace(/^\[|\]$/g, '');
  const halves = cleaned.split('::');
  const left = halves[0] ? halves[0].split(':').filter(Boolean) : [];
  const right = halves[1] ? halves[1].split(':').filter(Boolean) : [];
  const missing = 8 - left.length - right.length;
  const groups = [...left, ...Array(Math.max(0, missing)).fill('0'), ...right];
  const buf = Buffer.alloc(16);
  for (let i = 0; i < 8; i++) {
    const val = parseInt(groups[i] || '0', 16) & 0xffff;
    buf.writeUInt16BE(val, i * 2);
  }
  return buf;
}

/**
 * Decodes RustDesk's hbb_common::AddrMangle binary socket address format (IPv4 or IPv6).
 * Matches upstream rustdesk/hbb_common/src/lib.rs AddrMangle::decode byte-for-byte:
 * - len > 16: must be 18 bytes (16-byte IPv6 + 2-byte little-endian port)
 * - len <= 16: right-zero-padded to 16-byte LE u128:
 *     tm = (number >> 17) & 0xFFFF_FFFF
 *     ip = ((number >> 49) - tm) as u32 LE bytes
 *     port = ((number & 0xFFFFFF) - (tm & 0xFFFF)) as u16
 */
export function decodeAddrMangle(bytes: Uint8Array): { host: string; port: number } | null {
  if (!bytes || bytes.length === 0) return null;

  if (bytes.length > 16) {
    if (bytes.length !== 18) return null;
    const parts: string[] = [];
    for (let i = 0; i < 16; i += 2) {
      parts.push(((bytes[i] << 8) | bytes[i + 1]).toString(16));
    }
    const port = bytes[16] | (bytes[17] << 8);
    if (port === 0) return null;
    return { host: parts.join(':'), port };
  }

  const padded = new Uint8Array(16);
  padded.set(bytes, 0);
  let number = 0n;
  for (let i = 0; i < 16; i++) {
    number |= BigInt(padded[i]) << BigInt(i * 8);
  }
  const tm = (number >> 17n) & 0xffffffffn;
  const ipNum = Number(((number >> 49n) - tm) & 0xffffffffn) >>> 0;
  const port = Number(((number & 0xffffffn) - (tm & 0xffffn)) & 0xffffn);
  if (port === 0) return null;

  const b0 = ipNum & 0xff;
  const b1 = (ipNum >>> 8) & 0xff;
  const b2 = (ipNum >>> 16) & 0xff;
  const b3 = (ipNum >>> 24) & 0xff;
  return { host: `${b0}.${b1}.${b2}.${b3}`, port };
}
