use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::io;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Clone, Copy)]
pub struct BytesCodec {
    state: DecodeState,
    raw: bool,
    max_packet_length: usize,
}

#[derive(Debug, Clone, Copy)]
enum DecodeState {
    Head,
    Data(usize),
}

impl Default for BytesCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl BytesCodec {
    pub fn new() -> Self {
        Self {
            state: DecodeState::Head,
            raw: false,
            max_packet_length: usize::MAX,
        }
    }

    pub fn set_raw(&mut self) {
        self.raw = true;
    }

    pub fn set_max_packet_length(&mut self, n: usize) {
        self.max_packet_length = n;
    }

    fn decode_head(&mut self, src: &mut BytesMut) -> io::Result<Option<usize>> {
        if src.is_empty() {
            return Ok(None);
        }
        let head_len = ((src[0] & 0x3) + 1) as usize;
        if src.len() < head_len {
            return Ok(None);
        }
        let mut n = src[0] as usize;
        if head_len > 1 {
            n |= (src[1] as usize) << 8;
        }
        if head_len > 2 {
            n |= (src[2] as usize) << 16;
        }
        if head_len > 3 {
            n |= (src[3] as usize) << 24;
        }
        n >>= 2;
        if n > self.max_packet_length {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Too big packet"));
        }
        src.advance(head_len);
        src.reserve(n);
        Ok(Some(n))
    }

    fn decode_data(&self, n: usize, src: &mut BytesMut) -> io::Result<Option<BytesMut>> {
        if src.len() < n {
            return Ok(None);
        }
        Ok(Some(src.split_to(n)))
    }
}

impl Decoder for BytesCodec {
    type Item = BytesMut;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<BytesMut>, io::Error> {
        if self.raw {
            if !src.is_empty() {
                let len = src.len();
                return Ok(Some(src.split_to(len)));
            } else {
                return Ok(None);
            }
        }
        let n = match self.state {
            DecodeState::Head => match self.decode_head(src)? {
                Some(n) => {
                    self.state = DecodeState::Data(n);
                    n
                }
                None => return Ok(None),
            },
            DecodeState::Data(n) => n,
        };

        match self.decode_data(n, src)? {
            Some(data) => {
                self.state = DecodeState::Head;
                Ok(Some(data))
            }
            None => Ok(None),
        }
    }
}

impl Encoder<Bytes> for BytesCodec {
    type Error = io::Error;

    fn encode(&mut self, data: Bytes, buf: &mut BytesMut) -> Result<(), io::Error> {
        if self.raw {
            buf.reserve(data.len());
            buf.put(data);
            return Ok(());
        }
        if data.len() <= 0x3F {
            buf.put_u8((data.len() << 2) as u8);
        } else if data.len() <= 0x3FFF {
            buf.put_u16_le((data.len() << 2) as u16 | 0x1);
        } else if data.len() <= 0x3FFFFF {
            let h = (data.len() << 2) as u32 | 0x2;
            buf.put_u16_le((h & 0xFFFF) as u16);
            buf.put_u8((h >> 16) as u8);
        } else if data.len() <= 0x3FFFFFFF {
            buf.put_u32_le((data.len() << 2) as u32 | 0x3);
        } else {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Overflow"));
        }
        buf.extend(data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_codec1() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        bytes.resize(0x3F, 1);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        let buf_saved = buf.clone();
        assert_eq!(buf.len(), 0x3F + 1);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3F);
            assert_eq!(res[0], 1);
        } else {
            panic!();
        }
        let mut codec2 = BytesCodec::new();
        let mut buf2 = BytesMut::new();
        if let Ok(None) = codec2.decode(&mut buf2) {
        } else {
            panic!();
        }
        buf2.extend(&buf_saved[0..1]);
        if let Ok(None) = codec2.decode(&mut buf2) {
        } else {
            panic!();
        }
        buf2.extend(&buf_saved[1..]);
        if let Ok(Some(res)) = codec2.decode(&mut buf2) {
            assert_eq!(res.len(), 0x3F);
            assert_eq!(res[0], 1);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_codec2() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        assert!(codec.encode("".into(), &mut buf).is_ok());
        assert_eq!(buf.len(), 1);
        bytes.resize(0x3F + 1, 2);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        assert_eq!(buf.len(), 0x3F + 2 + 2);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0);
        } else {
            panic!();
        }
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3F + 1);
            assert_eq!(res[0], 2);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_codec3() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        bytes.resize(0x3F - 1, 3);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        assert_eq!(buf.len(), 0x3F + 1 - 1);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3F - 1);
            assert_eq!(res[0], 3);
        } else {
            panic!();
        }
    }
    #[test]
    fn test_codec4() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        bytes.resize(0x3FFF, 4);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        assert_eq!(buf.len(), 0x3FFF + 2);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3FFF);
            assert_eq!(res[0], 4);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_codec5() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        bytes.resize(0x3FFFFF, 5);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        assert_eq!(buf.len(), 0x3FFFFF + 3);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3FFFFF);
            assert_eq!(res[0], 5);
        } else {
            panic!();
        }
    }

    #[test]
    fn test_codec6() {
        let mut codec = BytesCodec::new();
        let mut buf = BytesMut::new();
        let mut bytes: Vec<u8> = Vec::new();
        bytes.resize(0x3FFFFF + 1, 6);
        assert!(codec.encode(bytes.into(), &mut buf).is_ok());
        let buf_saved = buf.clone();
        assert_eq!(buf.len(), 0x3FFFFF + 4 + 1);
        if let Ok(Some(res)) = codec.decode(&mut buf) {
            assert_eq!(res.len(), 0x3FFFFF + 1);
            assert_eq!(res[0], 6);
        } else {
            panic!();
        }
        let mut codec2 = BytesCodec::new();
        let mut buf2 = BytesMut::new();
        buf2.extend(&buf_saved[0..1]);
        if let Ok(None) = codec2.decode(&mut buf2) {
        } else {
            panic!();
        }
        buf2.extend(&buf_saved[1..6]);
        if let Ok(None) = codec2.decode(&mut buf2) {
        } else {
            panic!();
        }
        buf2.extend(&buf_saved[6..]);
        if let Ok(Some(res)) = codec2.decode(&mut buf2) {
            assert_eq!(res.len(), 0x3FFFFF + 1);
            assert_eq!(res[0], 6);
        } else {
            panic!();
        }
    }
}
