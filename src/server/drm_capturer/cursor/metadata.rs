use super::DrmCursorData;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    io,
    mem::size_of,
};

const RGBA: u32 = 11;
const PIXEL_BYTES: usize = 4;
const CURSOR_WORDS: usize = 7;
const BITMAP_WORDS: usize = 5;
const CURSOR_BYTES: usize = CURSOR_WORDS * size_of::<u32>();
const BITMAP_BYTES: usize = BITMAP_WORDS * size_of::<u32>();
pub const META_HEADER_BYTES: u32 = (CURSOR_BYTES + BITMAP_BYTES) as u32;
// Preferred allocation; negotiation accepts the compositor's metadata size.
const CURSOR_META_SIDE: usize = 384;
pub const META_BYTES: u32 =
    META_HEADER_BYTES + (CURSOR_META_SIDE * CURSOR_META_SIDE * PIXEL_BYTES) as u32;

#[derive(Default)]
pub struct CursorState {
    image: Option<DrmCursorData>,
    published: Option<u64>,
}

impl CursorState {
    pub fn update(&mut self, data: &[u8]) -> io::Result<Option<DrmCursorData>> {
        // Mutter uses id 0 outside this monitor or while the pointer is hidden. Reentry may
        // contain only a position, so retain the sprite while publishing the hidden sentinel.
        if words::<CURSOR_WORDS>(data)?[0] == 0 {
            if self.published == Some(scrap::drm_reader::HIDDEN_CURSOR_ID) {
                return Ok(None);
            }
            self.published = Some(scrap::drm_reader::HIDDEN_CURSOR_ID);
            return Ok(Some(hidden_cursor()));
        }
        if let Some(cursor) = decode(data)? {
            self.image = Some(cursor);
        }
        if let Some(cursor) = self.image.as_ref() {
            if self.published != Some(cursor.id) {
                self.published = Some(cursor.id);
                return Ok(Some(cursor.clone()));
            }
        }
        Ok(None)
    }
}

fn hidden_cursor() -> DrmCursorData {
    DrmCursorData {
        id: scrap::drm_reader::HIDDEN_CURSOR_ID,
        width: 1,
        height: 1,
        hotx: 0,
        hoty: 0,
        colors: vec![0; PIXEL_BYTES],
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn words<const N: usize>(data: &[u8]) -> io::Result<[u32; N]> {
    let bytes = data
        .get(..N * size_of::<u32>())
        .ok_or_else(|| invalid("Truncated PipeWire cursor metadata"))?;
    let mut values = [0; N];
    for (value, bytes) in values.iter_mut().zip(bytes.chunks_exact(size_of::<u32>())) {
        *value = u32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    }
    Ok(values)
}

fn decode(data: &[u8]) -> io::Result<Option<DrmCursorData>> {
    let [id, _flags, _x, _y, hotx, hoty, offset] = words::<CURSOR_WORDS>(data)?;
    // Position-only updates have no valid hotspot or bitmap. Keep the previous shape.
    if id == 0 || offset == 0 {
        return Ok(None);
    }
    if (offset as usize) < CURSOR_BYTES {
        return Err(invalid("PipeWire cursor bitmap overlaps its header"));
    }
    let bitmap = data
        .get(offset as usize..)
        .ok_or_else(|| invalid("PipeWire cursor bitmap offset exceeds metadata"))?;
    let [format, width, height, stride, offset] = words::<BITMAP_WORDS>(bitmap)?;
    if offset == 0 {
        return Ok(Some(hidden_cursor()));
    }
    if format == 0 {
        return Ok(None);
    }
    let mut cursor = decode_bitmap(bitmap, [format, width, height, stride, offset])?;
    cursor.hotx = hotx as i32;
    cursor.hoty = hoty as i32;
    let mut hash = DefaultHasher::new();
    (cursor.width, cursor.height, cursor.hotx, cursor.hoty).hash(&mut hash);
    cursor.colors.hash(&mut hash);
    // Mutter reuses id 1 for every sprite, including different hotspots.
    cursor.id = hash.finish();
    Ok(Some(cursor))
}

fn decode_bitmap(data: &[u8], header: [u32; BITMAP_WORDS]) -> io::Result<DrmCursorData> {
    let [format, width, height, stride, offset] = header;
    if format != RGBA {
        return Err(invalid("Unsupported PipeWire cursor pixel format"));
    }
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(invalid("Invalid PipeWire cursor dimensions"));
    }
    let row = (width as usize)
        .checked_mul(PIXEL_BYTES)
        .ok_or_else(|| invalid("PipeWire cursor row overflows"))?;
    if (stride as i32) <= 0 || (stride as usize) < row || (offset as usize) < BITMAP_BYTES {
        return Err(invalid("Invalid PipeWire cursor stride or pixel offset"));
    }
    let length = (stride as usize)
        .checked_mul(height as usize - 1)
        .and_then(|n| n.checked_add(row))
        .ok_or_else(|| invalid("PipeWire cursor bitmap size overflows"))?;
    let pixels = data
        .get(offset as usize..)
        .and_then(|bytes| bytes.get(..length))
        .ok_or_else(|| invalid("Truncated PipeWire cursor pixels"))?;
    let mut colors = Vec::with_capacity(row * height as usize);
    for bytes in pixels.chunks(stride as usize) {
        colors.extend_from_slice(&bytes[..row]);
    }
    Ok(DrmCursorData {
        id: 0,
        width: width as i32,
        height: height as i32,
        hotx: 0,
        hoty: 0,
        colors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(hotspot: [u32; 2]) -> Vec<u8> {
        let mut bytes: Vec<_> = [
            1,
            0,
            100,
            200,
            hotspot[0],
            hotspot[1],
            CURSOR_BYTES as u32,
            RGBA,
            96,
            96,
            96 * PIXEL_BYTES as u32,
            BITMAP_BYTES as u32,
        ]
        .into_iter()
        .flat_map(u32::to_ne_bytes)
        .collect();
        bytes.resize(CURSOR_BYTES + BITMAP_BYTES + 96 * 96 * PIXEL_BYTES, 0);
        let artwork = CURSOR_BYTES + BITMAP_BYTES + (4 * 96 + 4) * PIXEL_BYTES;
        bytes[artwork..artwork + PIXEL_BYTES].copy_from_slice(&[128, 64, 32, 128]);
        bytes
    }

    #[test]
    fn compositor_hotspot_is_independent_of_visible_bounds_and_id() {
        let bytes = packet([42, 42]);
        let cross = decode(&bytes).unwrap().unwrap();
        assert_eq!(
            (cross.width, cross.height, cross.hotx, cross.hoty),
            (96, 96, 42, 42)
        );
        assert_eq!(&cross.colors, &bytes[CURSOR_BYTES + BITMAP_BYTES..]);
        let origin = decode(&packet([0, 0])).unwrap().unwrap();
        assert_eq!((origin.hotx, origin.hoty), (0, 0));
        assert_ne!(cross.id, origin.id);
        let mut changed = bytes.clone();
        *changed.last_mut().unwrap() = 255;
        assert_ne!(cross.id, decode(&changed).unwrap().unwrap().id);
    }

    #[test]
    fn movement_keeps_the_shape_and_empty_bitmap_hides_it() {
        let mut bytes = packet([42, 45]);
        bytes[CURSOR_BYTES - size_of::<u32>()..CURSOR_BYTES].fill(0);
        assert!(decode(&bytes).unwrap().is_none());
        bytes[..size_of::<u32>()].fill(0);
        assert!(decode(&bytes).unwrap().is_none());
        let mut hidden = packet([42, 45]);
        hidden[CURSOR_BYTES..CURSOR_BYTES + BITMAP_BYTES].fill(0);
        let hidden = decode(&hidden).unwrap().unwrap();
        assert_eq!(hidden.id, scrap::drm_reader::HIDDEN_CURSOR_ID);
        assert_eq!(hidden.colors, [0; PIXEL_BYTES]);
    }

    #[test]
    fn leaving_and_returning_to_a_monitor_restores_the_cached_sprite() {
        let mut state = CursorState::default();
        let mut bytes = packet([42, 42]);
        let initial = state.update(&bytes).unwrap().unwrap();
        bytes[..size_of::<u32>()].fill(0);
        let hidden = state
            .update(&bytes)
            .unwrap()
            .expect("Mutter id 0 hides the cursor");
        assert_eq!(hidden.id, scrap::drm_reader::HIDDEN_CURSOR_ID);
        assert!(state.update(&bytes).unwrap().is_none());
        bytes[..size_of::<u32>()].copy_from_slice(&1u32.to_ne_bytes());
        bytes[CURSOR_BYTES - size_of::<u32>()..CURSOR_BYTES].fill(0);
        let restored = state
            .update(&bytes)
            .unwrap()
            .expect("Position-only reentry restores the sprite");
        assert_eq!(
            (restored.id, restored.hotx, restored.hoty),
            (initial.id, 42, 42)
        );
        assert_eq!(restored.colors, initial.colors);
        assert!(state.update(&bytes).unwrap().is_none());
    }

    #[test]
    fn malformed_metadata_is_an_error() {
        let bytes = packet([12, 3]);
        for length in [
            0,
            CURSOR_BYTES - 1,
            CURSOR_BYTES + BITMAP_BYTES - 1,
            bytes.len() - 1,
        ] {
            assert!(decode(&bytes[..length]).is_err());
        }
        for (word, value) in [
            (6, 1),
            (6, u32::MAX),
            (7, 99),
            (8, 0),
            (9, u32::MAX),
            (10, 1),
            (10, u32::MAX),
            (11, 1),
        ] {
            let mut invalid = bytes.clone();
            invalid[word * size_of::<u32>()..(word + 1) * size_of::<u32>()]
                .copy_from_slice(&value.to_ne_bytes());
            assert!(decode(&invalid).is_err(), "word {word}, value {value}");
        }
    }
}
