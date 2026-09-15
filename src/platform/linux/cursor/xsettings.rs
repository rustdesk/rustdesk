use std::{convert::TryInto, io};

const HEADER_SIZE: usize = 12;
const ALIGNMENT: usize = 4;
const INTEGER: u8 = 0;
const STRING: u8 = 1;
const COLOR: u8 = 2;
const WINDOW_SCALE: &[u8] = b"Gdk/WindowScalingFactor";

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    little_endian: bool,
}

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> io::Result<&'a [u8]> {
        let end = self.offset.checked_add(length).ok_or_else(invalid)?;
        let bytes = self.bytes.get(self.offset..end).ok_or_else(invalid)?;
        self.offset = end;
        Ok(bytes)
    }

    fn number(&mut self, length: usize) -> io::Result<u32> {
        let bytes = self.take(length)?;
        Ok(if self.little_endian {
            bytes
                .iter()
                .rev()
                .fold(0, |n, byte| (n << 8) | *byte as u32)
        } else {
            bytes.iter().fold(0, |n, byte| (n << 8) | *byte as u32)
        })
    }

    fn string(&mut self, length: usize) -> io::Result<&'a [u8]> {
        let bytes = self.take(length)?;
        self.take((ALIGNMENT - length % ALIGNMENT) % ALIGNMENT)?;
        Ok(bytes)
    }
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid XSETTINGS cursor density",
    )
}

pub(super) fn scale(bytes: &[u8]) -> io::Result<Option<f64>> {
    let little_endian = match bytes.first() {
        Some(0) => true,
        Some(1) => false,
        _ => return Err(invalid()),
    };
    let mut reader = Reader {
        bytes,
        offset: 0,
        little_endian,
    };
    reader.take(HEADER_SIZE - ALIGNMENT)?;
    let count = reader.number(ALIGNMENT)?;
    let mut scale = None;
    for _ in 0..count {
        let kind = reader.number(1)? as u8;
        reader.take(1)?;
        let length = reader.number(2)? as usize;
        let name = reader.string(length)?;
        reader.take(ALIGNMENT)?; // Last-change serial.
        match kind {
            INTEGER => {
                let value = reader.number(ALIGNMENT)? as i32;
                if name == WINDOW_SCALE {
                    if value <= 0 {
                        return Err(invalid());
                    }
                    scale = Some(f64::from(value));
                }
            }
            STRING => {
                let length = reader
                    .number(ALIGNMENT)?
                    .try_into()
                    .map_err(|_| invalid())?;
                reader.string(length)?;
            }
            COLOR => {
                reader.take(ALIGNMENT * 2)?;
            }
            _ => return Err(invalid()),
        }
    }
    Ok(scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_scale_ignores_text_dpi_and_checks_the_entire_property() {
        for little in [false, true] {
            let mut bytes = vec![u8::from(!little), 0, 0, 0];
            let number = |n: u32| {
                if little {
                    n.to_le_bytes()
                } else {
                    n.to_be_bytes()
                }
            };
            bytes.extend(number(1));
            bytes.extend(number(2));
            for (name, value) in [(b"Xft/DPI".as_slice(), 196608), (WINDOW_SCALE, 2)] {
                bytes.extend([INTEGER, 0]);
                let length = name.len() as u16;
                bytes.extend(if little {
                    length.to_le_bytes()
                } else {
                    length.to_be_bytes()
                });
                bytes.extend(name);
                bytes.resize(bytes.len().div_ceil(ALIGNMENT) * ALIGNMENT, 0);
                bytes.extend(number(1));
                bytes.extend(number(value));
            }
            assert_eq!(scale(&bytes).unwrap(), Some(2.0));
            for length in 0..bytes.len() {
                assert!(scale(&bytes[..length]).is_err());
            }
            let end = bytes.len();
            bytes[end - ALIGNMENT..].copy_from_slice(&number(0));
            assert!(scale(&bytes).is_err());
        }
    }
}
