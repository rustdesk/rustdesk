export const HEADER_LEN = 32;

export const CODEC_VP8 = 1;
export const CODEC_VP9 = 2;
export const CODEC_H264 = 3;
export const CODEC_H265 = 4;
export const CODEC_AV1 = 5;

export interface VideoPacket {
  codec: number;
  key: boolean;
  display: number;
  width: number;
  height: number;
  pts: bigint;
  gatewayUnixMs: bigint;
  data: Uint8Array;
}

const MAGIC_0 = 0x52; // 'R'
const MAGIC_1 = 0x44; // 'D'
const MAGIC_2 = 0x56; // 'V'
const MAGIC_3 = 0x31; // '1'

/**
 * Encodes a single encoded video frame into the 32-byte little-endian RDV1 binary format.
 */
export function encodeRdv1Frame(packet: VideoPacket): Buffer {
  const out = Buffer.allocUnsafe(HEADER_LEN + packet.data.byteLength);
  out[0] = MAGIC_0;
  out[1] = MAGIC_1;
  out[2] = MAGIC_2;
  out[3] = MAGIC_3;
  out[4] = packet.codec & 0xff;
  out[5] = packet.key ? 0x01 : 0x00;
  out.writeUInt16LE(packet.display & 0xffff, 6);
  out.writeUInt32LE(packet.width >>> 0, 8);
  out.writeUInt32LE(packet.height >>> 0, 12);
  out.writeBigInt64LE(packet.pts, 16);
  out.writeBigUInt64LE(packet.gatewayUnixMs, 24);
  Buffer.from(packet.data.buffer, packet.data.byteOffset, packet.data.byteLength).copy(
    out,
    HEADER_LEN,
  );
  return out;
}

/**
 * Decodes an RDV1 binary frame buffer. Returns null if too short or magic does not match.
 */
export function decodeRdv1Frame(buf: Uint8Array): VideoPacket | null {
  if (buf.byteLength < HEADER_LEN) return null;
  if (
    buf[0] !== MAGIC_0 ||
    buf[1] !== MAGIC_1 ||
    buf[2] !== MAGIC_2 ||
    buf[3] !== MAGIC_3
  ) {
    return null;
  }
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  return {
    codec: view.getUint8(4),
    key: (view.getUint8(5) & 0x01) !== 0,
    display: view.getUint16(6, true),
    width: view.getUint32(8, true),
    height: view.getUint32(12, true),
    pts: view.getBigInt64(16, true),
    gatewayUnixMs: view.getBigUint64(24, true),
    data: buf.subarray(HEADER_LEN),
  };
}

/**
 * Parses width and height from a VP8 keyframe uncompressed data chunk header.
 * Returns null if the packet is not a VP8 keyframe or is truncated.
 */
export function vp8KeyframeSize(data: Uint8Array): { width: number; height: number } | null {
  if (data.byteLength < 10) return null;
  // Bit 0 of byte 0 is 0 for keyframes, 1 for interframes
  if ((data[0] & 0x01) !== 0) return null;
  // VP8 keyframe start code: 0x9d 0x01 0x2a
  if (data[3] !== 0x9d || data[4] !== 0x01 || data[5] !== 0x2a) return null;
  const width = (data[6] | (data[7] << 8)) & 0x3fff;
  const height = (data[8] | (data[9] << 8)) & 0x3fff;
  if (width === 0 || height === 0) return null;
  return { width, height };
}
