use std::{cell::RefCell, sync::Mutex};

#[derive(Clone, Copy, Debug, Default)]
pub struct DirectRenderTarget {
    pub surface_id: Option<u64>,
    pub decode_size: Option<(usize, usize)>,
}

pub type DirectRenderTargetLookup = fn(&str, usize) -> Option<DirectRenderTarget>;

lazy_static::lazy_static! {
    static ref DIRECT_RENDER_TARGET_LOOKUP: Mutex<Option<DirectRenderTargetLookup>> =
        Default::default();
}

thread_local! {
    static DIRECT_RENDER_CONTEXT: RefCell<Option<(String, usize)>> = const { RefCell::new(None) };
}

pub struct DirectRenderContextGuard {
    previous: Option<(String, usize)>,
}

impl Drop for DirectRenderContextGuard {
    fn drop(&mut self) {
        DIRECT_RENDER_CONTEXT.with(|context| {
            *context.borrow_mut() = self.previous.take();
        });
    }
}

pub fn enter_direct_render_context(
    peer_id: String,
    display: usize,
) -> DirectRenderContextGuard {
    let previous = DIRECT_RENDER_CONTEXT.with(|context| {
        context
            .borrow_mut()
            .replace((peer_id, display))
    });
    DirectRenderContextGuard { previous }
}

pub(crate) fn current_direct_render_target() -> DirectRenderTarget {
    DIRECT_RENDER_CONTEXT.with(|context| {
        context
            .borrow()
            .as_ref()
            .and_then(|(peer_id, display)| lookup_direct_render_target(peer_id, *display))
            .unwrap_or_default()
    })
}

pub fn register_direct_render_target_lookup(lookup: DirectRenderTargetLookup) {
    *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap() = Some(lookup);
}

pub fn lookup_direct_render_target(peer_id: &str, display: usize) -> Option<DirectRenderTarget> {
    let lookup = *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap();
    lookup.and_then(|lookup| lookup(peer_id, display))
}
