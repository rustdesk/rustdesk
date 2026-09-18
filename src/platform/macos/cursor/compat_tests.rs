use super::*;
use hbb_common::protobuf::Message;
use objc::rc::autoreleasepool;

#[test]
fn retina_export_preserves_legacy_dimensions_and_hotspot() {
    autoreleasepool(|| unsafe {
        let logical = NSSize::new(32.0, 32.0);
        let image: id = msg_send![class!(NSImage), alloc];
        let image = StrongPtr::new(msg_send![image, initWithSize: logical]);
        let rep = bitmap(NSSize::new(64.0, 64.0)).unwrap();
        let buffer: *mut u8 = msg_send![*rep, bitmapData];
        ptr::write_bytes(buffer, 255, 64 * 64 * CHANNELS);
        let (): () = msg_send![*rep, setSize: logical];
        let (): () = msg_send![*image, addRepresentation: *rep];
        let cursor: id = msg_send![class!(NSCursor), alloc];
        let cursor = StrongPtr::new(msg_send![cursor,
            initWithImage: *image hotSpot: NSPoint::new(8.0, 12.0)]);

        let exported = data(*cursor, 123, 2.0).unwrap();
        let legacy = CursorData::parse_from_bytes(&exported.write_to_bytes().unwrap()).unwrap();
        assert_eq!((legacy.width, legacy.height), (32, 32));
        assert_eq!((legacy.hotx, legacy.hoty), (8, 12));
        assert_eq!(legacy.colors.len(), 32 * 32 * CHANNELS);
        assert_eq!(legacy.scale, 1.0);
        assert!(legacy.colors.iter().all(|channel| *channel == 255));
        let physical = legacy.high_resolution.as_ref().unwrap();
        assert_eq!((physical.width, physical.height), (64, 64));
        assert_eq!((physical.hotx, physical.hoty), (16, 24));
        assert_eq!(physical.colors.len(), 64 * 64 * CHANNELS);
        assert_eq!((physical.id, physical.scale), (legacy.id, 2.0));
        assert!(physical.colors.iter().all(|channel| *channel == 255));
        assert!(data(*cursor, 123, 1.0).unwrap().high_resolution.is_none());
    });
}
