use super::{drawOutline, handleMask, CursorData};
use hbb_common::{anyhow::Context, bail, ResultType};
use scrap::dxgi::cursor::{self, Shape, Snapshot, CURSOR_ID_FLAG};
use winapi::{
    shared::dxgi1_2::{
        DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR, DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME,
    },
    um::winuser::{MonitorFromPoint, CURSORINFO, MONITOR_DEFAULTTONULL},
};

const CHANNELS: usize = 4;
const BORDER: i32 = 1;

pub(super) fn current(info: &CURSORINFO) -> ResultType<Option<u64>> {
    let monitor = unsafe { MonitorFromPoint(info.ptScreenPos, MONITOR_DEFAULTTONULL) };
    match cursor::snapshot(monitor as usize) {
        Snapshot::Unavailable => Ok(Some(info.hCursor as usize as u32 as u64)),
        Snapshot::Pending => Ok(None),
        Snapshot::Ready(shape) => Ok(Some(shape.id)),
        Snapshot::Failed(error) => bail!("DXGI cursor capture: {error}"),
    }
}

pub(super) fn data(id: u64) -> ResultType<Option<CursorData>> {
    if id & CURSOR_ID_FLAG == 0 {
        return Ok(None);
    }
    let shape = cursor::shape(id).context("DXGI cursor changed before export")?;
    let (colors, outline) = colors(&shape)?;
    let data = CursorData {
        id,
        colors: colors.into(),
        width: shape.width as _,
        height: shape.height as _,
        hotx: shape.hotspot.0,
        hoty: shape.hotspot.1,
        ..Default::default()
    };
    Ok(Some(if outline { outlined(data)? } else { data }))
}

fn colors(shape: &Shape) -> ResultType<(Vec<u8>, bool)> {
    let length = (shape.width as usize)
        .checked_mul(shape.height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .context("Cursor size overflow")?;
    if shape.kind == DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME {
        let mut colors = vec![0; length];
        let outline = unsafe {
            handleMask(
                colors.as_mut_ptr(),
                shape.pixels.as_ptr(),
                shape.width as _,
                shape.height as _,
                shape.pitch as _,
                (shape.height * 2) as _,
            )
        } > 0;
        return Ok((colors, outline));
    }
    let mut colors = Vec::with_capacity(length);
    let mut outline = false;
    for row in shape.pixels.chunks_exact(shape.pitch as usize) {
        for pixel in row[..shape.width as usize * CHANNELS].chunks_exact(CHANNELS) {
            let (rgba, xor) = rgba(pixel, shape.kind);
            outline |= xor;
            colors.extend_from_slice(&rgba);
        }
    }
    Ok((colors, outline))
}

fn rgba(pixel: &[u8], kind: u32) -> ([u8; CHANNELS], bool) {
    if kind != DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR {
        return ([pixel[2], pixel[1], pixel[0], pixel[3]], false);
    }
    if pixel[3] == 0 {
        return ([pixel[2], pixel[1], pixel[0], 255], false);
    }
    // Match the Win32 exporter's outlined replacement for background-dependent XOR.
    if pixel[..3].iter().any(|value| *value != 0) {
        ([0, 0, 0, 255], true)
    } else {
        ([0; CHANNELS], false)
    }
}

fn outlined(data: CursorData) -> ResultType<CursorData> {
    let width = data
        .width
        .checked_add(BORDER * 2)
        .context("Cursor width overflow")?;
    let height = data
        .height
        .checked_add(BORDER * 2)
        .context("Cursor height overflow")?;
    let length = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .context("Cursor size overflow")?;
    let length_i32 =
        i32::try_from(length).context("Cursor outline exceeds the native buffer size")?;
    let mut colors = vec![0; length];
    unsafe {
        drawOutline(
            colors.as_mut_ptr(),
            data.colors.as_ptr(),
            data.width,
            data.height,
            length_i32,
        );
    }
    Ok(CursorData {
        colors: colors.into(),
        width,
        height,
        hotx: data.hotx + BORDER,
        hoty: data.hoty + BORDER,
        ..data
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use winapi::shared::dxgi1_2::DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR;

    #[test]
    fn physical_cursor_preserves_alpha_and_ignores_row_padding() {
        let shape = Shape {
            id: CURSOR_ID_FLAG,
            kind: DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR,
            width: 1,
            height: 2,
            pitch: 8,
            hotspot: (0, 1),
            pixels: vec![
                32, 64, 128, 128, 255, 255, 255, 255, 1, 2, 3, 255, 255, 255, 255, 255,
            ],
        };
        assert_eq!(
            colors(&shape).unwrap(),
            (vec![128, 64, 32, 128, 3, 2, 1, 255], false)
        );
        let masked = Shape {
            kind: DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR,
            pixels: vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 0, 0, 0, 0],
            ..shape
        };
        assert_eq!(
            colors(&masked).unwrap(),
            (vec![0, 0, 0, 255, 0, 0, 0, 255], true)
        );
    }
}
