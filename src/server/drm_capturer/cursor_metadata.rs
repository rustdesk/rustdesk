use base::platform::linux::WaylandDisplayInfo;
use scrap::wayland::display::{CachedDisplays, Displays};
use std::cell::Cell;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Context {
    pub display: i32,
    pub epoch: u64,
    pub layout_generation: u64,
}

thread_local! {
    static LAST_MONITOR: Cell<Option<(Context, WaylandDisplayInfo)>> = const { Cell::new(None) };
}

pub(super) fn monitor(
    context: Context,
    snapshot: CachedDisplays,
    resolve: impl FnOnce(&Displays) -> Option<WaylandDisplayInfo>,
) -> Option<WaylandDisplayInfo> {
    // ID polling and bitmap retrieval run on the same cursor-service thread.
    // Only contention may reuse metadata, and only for this output, stream and layout.
    LAST_MONITOR.with(|last| match snapshot {
        CachedDisplays::Busy => {
            let cached = last.take();
            let monitor = cached
                .as_ref()
                .filter(|(key, _)| *key == context)
                .map(|(_, monitor)| monitor.clone());
            last.set(cached);
            monitor
        }
        CachedDisplays::Ready(displays) => {
            let monitor = displays.as_deref().and_then(resolve);
            // An accessible cache with no matching metadata invalidates the old density.
            last.set(monitor.clone().map(|monitor| (context, monitor)));
            monitor
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    const CONTEXT: Context = Context {
        display: 0,
        epoch: 7,
        layout_generation: 11,
    };

    fn displays(logical_width: i32) -> CachedDisplays {
        CachedDisplays::Ready(Some(Arc::new(Displays {
            primary: 0,
            displays: vec![WaylandDisplayInfo {
                name: "DP-1".into(),
                x: 0,
                y: 0,
                width: 1280,
                height: 800,
                logical_size: Some((logical_width, logical_width * 800 / 1280)),
                refresh_rate: 60000,
                transform: 0,
            }],
        })))
    }

    fn read(context: Context, snapshot: CachedDisplays) -> Option<WaylandDisplayInfo> {
        monitor(context, snapshot, |displays| {
            displays.displays.first().cloned()
        })
    }

    #[test]
    fn busy_cache_preserves_metadata_until_a_completed_read() {
        for width in [640, 1280] {
            let expected = read(CONTEXT, displays(width)).unwrap().logical_size;
            for _ in 0..5 {
                let retained = monitor(CONTEXT, CachedDisplays::Busy, |_| {
                    panic!("a busy cursor lookup must not resolve displays")
                });
                assert_eq!(retained.unwrap().logical_size, expected);
            }
        }
    }

    #[test]
    fn busy_cache_does_not_borrow_another_output_stream_or_layout() {
        read(CONTEXT, displays(640));
        for changed in [
            Context {
                display: 1,
                ..CONTEXT
            },
            Context {
                epoch: 8,
                ..CONTEXT
            },
            Context {
                layout_generation: 12,
                ..CONTEXT
            },
        ] {
            assert!(read(changed, CachedDisplays::Busy).is_none());
        }
        assert_eq!(
            read(CONTEXT, CachedDisplays::Busy).unwrap().logical_size,
            Some((640, 400))
        );
    }

    #[test]
    fn missing_or_unmatched_metadata_invalidates_retained_density() {
        read(CONTEXT, displays(640));
        assert!(read(CONTEXT, CachedDisplays::Ready(None)).is_none());
        assert!(read(CONTEXT, CachedDisplays::Busy).is_none());

        read(CONTEXT, displays(640));
        assert!(monitor(CONTEXT, displays(640), |_| None).is_none());
        assert!(read(CONTEXT, CachedDisplays::Busy).is_none());
    }
}
