use super::{CursorData, ResultType};
use cocoa::{
    appkit::NSCompositingOperation,
    base::{id, nil, NO, YES},
    foundation::{NSInteger, NSPoint, NSRect, NSSize, NSString},
};
use hbb_common::{anyhow::Context, bail};
use objc::{class, msg_send, rc::StrongPtr, sel, sel_impl};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ptr, slice,
};

const CHANNELS: usize = 4;
const BITS_PER_SAMPLE: NSInteger = 8;

pub(super) fn scale() -> ResultType<f64> {
    if !*scrap::quartz::ENABLE_RETINA.lock().unwrap() {
        return Ok(1.0);
    }
    unsafe {
        let point: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let screens: id = msg_send![class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        for index in 0..count {
            let screen: id = msg_send![screens, objectAtIndex: index];
            let frame: NSRect = msg_send![screen, frame];
            // AppKit's bottom-left coordinates include the upper screen edge.
            if point.x >= frame.origin.x
                && point.y > frame.origin.y
                && point.x < frame.origin.x + frame.size.width
                && point.y <= frame.origin.y + frame.size.height
            {
                return Ok(msg_send![screen, backingScaleFactor]);
            }
        }
    }
    bail!("No macOS display contains the cursor")
}

pub(super) fn cache_id(cursor: u64, scale: f64) -> u64 {
    let mut hash = DefaultHasher::new();
    (cursor, scale.to_bits()).hash(&mut hash);
    hash.finish()
}

unsafe fn bitmap(size: NSSize) -> ResultType<StrongPtr> {
    let color_space = StrongPtr::new(NSString::alloc(nil).init_str("NSDeviceRGBColorSpace"));
    let bitmap: id = msg_send![class!(NSBitmapImageRep), alloc];
    let bitmap: id = msg_send![bitmap,
        initWithBitmapDataPlanes: ptr::null_mut::<*mut u8>()
        pixelsWide: size.width as NSInteger pixelsHigh: size.height as NSInteger
        bitsPerSample: BITS_PER_SAMPLE samplesPerPixel: CHANNELS as NSInteger
        hasAlpha: YES isPlanar: NO colorSpaceName: *color_space
        bitmapFormat: 0usize bytesPerRow: (size.width as usize * CHANNELS) as NSInteger
        bitsPerPixel: BITS_PER_SAMPLE * CHANNELS as NSInteger];
    if bitmap == nil {
        bail!("Could not allocate the macOS cursor bitmap");
    }
    Ok(StrongPtr::new(bitmap))
}

unsafe fn render(image: id, bitmap: id, size: NSSize) -> ResultType<()> {
    let context: id =
        msg_send![class!(NSGraphicsContext), graphicsContextWithBitmapImageRep: bitmap];
    if context == nil {
        bail!("Could not create the macOS cursor graphics context");
    }
    let (): () = msg_send![class!(NSGraphicsContext), saveGraphicsState];
    let (): () = msg_send![class!(NSGraphicsContext), setCurrentContext: context];
    // Drawing at the pixel size lets AppKit select the matching image representation.
    let (): () = msg_send![image,
        drawInRect: NSRect::new(NSPoint::new(0.0, 0.0), size)
        fromRect: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
        operation: NSCompositingOperation::NSCompositeCopy fraction: 1.0f64];
    let (): () = msg_send![class!(NSGraphicsContext), restoreGraphicsState];
    Ok(())
}

pub(super) unsafe fn data(cursor: id, id: u64, scale: f64) -> ResultType<CursorData> {
    let image: id = msg_send![cursor, image];
    let logical: NSSize = msg_send![image, size];
    let size = NSSize::new(
        (logical.width * scale).round(),
        (logical.height * scale).round(),
    );
    if !size.width.is_finite()
        || !size.height.is_finite()
        || size.width <= 0.0
        || size.height <= 0.0
        || size.width > i32::MAX as f64
        || size.height > i32::MAX as f64
    {
        bail!("Invalid macOS cursor dimensions");
    }
    let length = (size.width as usize)
        .checked_mul(size.height as usize)
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .context("Cursor bitmap size overflow")?;
    let bitmap = bitmap(size)?;
    render(image, *bitmap, size)?;
    let pixels: *const u8 = msg_send![*bitmap, bitmapData];
    if pixels.is_null() {
        bail!("Could not read the macOS cursor bitmap");
    }
    let hotspot: NSPoint = msg_send![cursor, hotSpot];
    Ok(CursorData {
        id,
        colors: slice::from_raw_parts(pixels, length).to_vec().into(),
        hotx: (hotspot.x * size.width / logical.width).round() as _,
        hoty: (hotspot.y * size.height / logical.height).round() as _,
        width: size.width as _,
        height: size.height as _,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc::rc::autoreleasepool;

    #[test]
    fn retina_cursor_uses_complete_high_resolution_artwork() {
        autoreleasepool(|| unsafe {
            let logical = NSSize::new(9.0, 18.0);
            let image: id = msg_send![class!(NSImage), alloc];
            let image = StrongPtr::new(msg_send![image, initWithSize: logical]);
            let mut expected = Vec::new();
            for scale in [1, 2] {
                let width = logical.width as usize * scale;
                let height = logical.height as usize * scale;
                let rep = bitmap(NSSize::new(width as f64, height as f64)).unwrap();
                let mut pixels = vec![0; width * height * CHANNELS];
                for y in 0..height {
                    for x in 0..width {
                        if y == 0 || y == height - 1 || x == width / 2 {
                            let color = if y == 0 {
                                [255, 0, 0, 255]
                            } else {
                                [0, 255, 0, 255]
                            };
                            pixels[(y * width + x) * CHANNELS..(y * width + x + 1) * CHANNELS]
                                .copy_from_slice(&color);
                        }
                    }
                }
                let buffer: *mut u8 = msg_send![*rep, bitmapData];
                ptr::copy_nonoverlapping(pixels.as_ptr(), buffer, pixels.len());
                let (): () = msg_send![*rep, setSize: logical];
                let (): () = msg_send![*image, addRepresentation: *rep];
                if scale == 2 {
                    expected = pixels;
                }
            }
            let cursor: id = msg_send![class!(NSCursor), alloc];
            let cursor = StrongPtr::new(
                msg_send![cursor, initWithImage: *image hotSpot: NSPoint::new(4.0, 9.0)],
            );
            let result = data(*cursor, 1, 2.0).unwrap();
            assert_eq!(
                (result.width, result.height, result.hotx, result.hoty),
                (18, 36, 8, 18)
            );
            assert_eq!(result.colors.as_ref(), expected.as_slice());
        });
    }

    #[test]
    fn cursor_cache_changes_with_display_scale() {
        assert_ne!(cache_id(123, 1.0), cache_id(123, 2.0));
    }
}
