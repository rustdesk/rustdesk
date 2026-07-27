use std::sync::Mutex;

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

pub fn register_direct_render_target_lookup(lookup: DirectRenderTargetLookup) {
    *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap() = Some(lookup);
}

pub fn lookup_direct_render_target(peer_id: &str, display: usize) -> Option<DirectRenderTarget> {
    let lookup = *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap();
    lookup.and_then(|lookup| lookup(peer_id, display))
}
