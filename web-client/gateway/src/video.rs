//! Binary WebSocket envelope for one encoded frame.
//!
//! ```text
//!  0  magic          b"RDV1"
//!  4  codec          u8   1=vp8 2=vp9 3=h264 4=h265 5=av1
//!  5  flags          u8   bit 0 = keyframe
//!  6  display        u16  le
//!  8  width          u32  le
//! 12  height         u32  le
//! 16  pts            i64  le
//! 24  gateway_unix_ms u64 le
//! 32  payload        raw encoder bytes, no re-encode
//! ```

pub const HEADER_LEN: usize = 32;
pub const MAGIC: &[u8; 4] = b"RDV1";

pub const CODEC_VP8: u8 = 1;
pub const CODEC_VP9: u8 = 2;
pub const CODEC_H264: u8 = 3;
pub const CODEC_H265: u8 = 4;
pub const CODEC_AV1: u8 = 5;

pub const FLAG_KEY: u8 = 0b0000_0001;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedPacket<'a> {
    pub codec: u8,
    pub key: bool,
    pub display: u16,
    pub width: u32,
    pub height: u32,
    pub pts: i64,
    pub gateway_unix_ms: u64,
    pub data: &'a [u8],
}

pub fn encode(packet: &EncodedPacket<'_>) -> Vec<u8> {
    let mut out = Vec::with_capacity(HEADER_LEN + packet.data.len());
    out.extend_from_slice(MAGIC);
    out.push(packet.codec);
    out.push(if packet.key { FLAG_KEY } else { 0 });
    out.extend_from_slice(&packet.display.to_le_bytes());
    out.extend_from_slice(&packet.width.to_le_bytes());
    out.extend_from_slice(&packet.height.to_le_bytes());
    out.extend_from_slice(&packet.pts.to_le_bytes());
    out.extend_from_slice(&packet.gateway_unix_ms.to_le_bytes());
    out.extend_from_slice(packet.data);
    out
}

pub fn decode(bytes: &[u8]) -> Option<EncodedPacket<'_>> {
    if bytes.len() < HEADER_LEN || &bytes[0..4] != MAGIC {
        return None;
    }
    let codec = bytes[4];
    let flags = bytes[5];
    let display = u16::from_le_bytes([bytes[6], bytes[7]]);
    let width = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
    let height = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
    let pts = i64::from_le_bytes(bytes[16..24].try_into().ok()?);
    let gateway_unix_ms = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
    Some(EncodedPacket {
        codec,
        key: flags & FLAG_KEY != 0,
        display,
        width,
        height,
        pts,
        gateway_unix_ms,
        data: &bytes[HEADER_LEN..],
    })
}

pub fn codec_name(codec: u8) -> &'static str {
    match codec {
        CODEC_VP8 => "vp8",
        CODEC_VP9 => "vp9",
        CODEC_H264 => "h264",
        CODEC_H265 => "h265",
        CODEC_AV1 => "av1",
        _ => "unknown",
    }
}

/// Coded size from a VP8 keyframe. Interframes do not repeat it.
pub fn vp8_keyframe_size(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 10 || data[0] & 1 != 0 {
        return None;
    }
    if data[3] != 0x9d || data[4] != 0x01 || data[5] != 0x2a {
        return None;
    }
    let width = u16::from_le_bytes([data[6], data[7]]) as u32 & 0x3fff;
    let height = u16::from_le_bytes([data[8], data[9]]) as u32 & 0x3fff;
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip_keeps_payload_and_keyframe_flag() {
        let payload = [0x9d, 0x01, 0x2a, 9, 8, 7];
        let packet = EncodedPacket {
            codec: CODEC_VP8,
            key: true,
            display: 2,
            width: 1280,
            height: 720,
            pts: -5,
            gateway_unix_ms: 1_700_000_000_000,
            data: &payload,
        };
        let bytes = encode(&packet);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(decoded, packet);
        assert_eq!(codec_name(decoded.codec), "vp8");
    }

    #[test]
    fn reject_short_or_foreign_frames() {
        assert!(decode(b"RDV1").is_none());
        assert!(decode(&[0u8; HEADER_LEN]).is_none());
    }

    #[test]
    fn vp8_keyframe_size_reads_the_uncompressed_header() {
        let mut frame = vec![0u8; 10];
        frame[3] = 0x9d;
        frame[4] = 0x01;
        frame[5] = 0x2a;
        frame[6..8].copy_from_slice(&1280u16.to_le_bytes());
        frame[8..10].copy_from_slice(&720u16.to_le_bytes());
        assert_eq!(vp8_keyframe_size(&frame), Some((1280, 720)));
        frame[0] |= 1;
        assert_eq!(vp8_keyframe_size(&frame), None);
    }
}
