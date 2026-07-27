use std::{collections::HashMap, sync::Mutex};

#[derive(Clone, Copy, Debug, Default)]
pub struct DirectRenderTarget {
    pub surface_id: Option<u64>,
    pub decode_size: Option<(usize, usize)>,
}

pub type DirectRenderTargetLookup = fn(&str, usize) -> Option<DirectRenderTarget>;
pub type RenderStatsCallback = fn(String, usize, Option<u64>);

pub(crate) struct RenderContext {
    pub session_id: String,
    pub display: usize,
}

lazy_static::lazy_static! {
    static ref DIRECT_RENDER_TARGET_LOOKUP: Mutex<Option<DirectRenderTargetLookup>> =
        Default::default();
    static ref RENDER_STATS_CALLBACK: Mutex<Option<RenderStatsCallback>> = Default::default();
    static ref RENDER_CONTEXTS: Mutex<HashMap<u64, RenderContext>> = Default::default();
}

pub fn register_direct_render_target_lookup(lookup: DirectRenderTargetLookup) {
    *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap() = Some(lookup);
}

pub fn lookup_direct_render_target(peer_id: &str, display: usize) -> Option<DirectRenderTarget> {
    let lookup = *DIRECT_RENDER_TARGET_LOOKUP.lock().unwrap();
    lookup.and_then(|lookup| lookup(peer_id, display))
}

pub fn register_render_stats_callback(callback: RenderStatsCallback) {
    *RENDER_STATS_CALLBACK.lock().unwrap() = Some(callback);
}

pub fn register_render_context(surface_id: u64, session_id: String, display: usize) {
    RENDER_CONTEXTS.lock().unwrap().insert(
        surface_id,
        RenderContext {
            session_id,
            display,
        },
    );
}

pub(crate) fn take_render_context(surface_id: u64) -> Option<RenderContext> {
    RENDER_CONTEXTS.lock().unwrap().remove(&surface_id)
}

pub(crate) fn notify_frame_rendered(
    session_id: Option<&str>,
    display: usize,
    latency: Option<u64>,
) {
    let Some(session_id) = session_id else {
        return;
    };
    let callback = *RENDER_STATS_CALLBACK.lock().unwrap();
    if let Some(callback) = callback {
        callback(session_id.to_owned(), display, latency);
    }
}
