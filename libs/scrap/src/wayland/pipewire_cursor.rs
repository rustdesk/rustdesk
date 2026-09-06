// Wayland host cursor shape from the xdg-desktop-portal ScreenCast `SPA_META_Cursor` metadata.
//
// The video path (GStreamer `pipewiresrc`) drops SPA buffer metadata, and the legacy cursor path
// reads the shape from XWayland via XFixes, which native Wayland clients never update -- so the
// remote sees a stale shape (the "cursor stuck as I-beam" bug, rustdesk/rustdesk#12206). This module
// opens a second, native-PipeWire consumer on the same portal fd and node ids, negotiates the cursor
// metadata, and publishes the latest shape into a global store that `platform::linux::get_cursor*`
// reads before falling back to XFixes. It never touches the video pipeline.
//
// Reading the metadata uses only the safe pipewire-rs 0.10 API (`Buffer::find_meta::<MetaCursor>`,
// `MetaBitmap::bitmap_data`); the only unsafe is `libc::dup` for the fd the PipeWire core takes over.

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Mutex;

use pipewire as pw;
use pw::spa;
use tracing::warn;

/// Cursor shape published to the cursor service. `colors` is tightly packed RGBA, matching what the
/// XFixes path in `platform::linux::get_cursor_data` produces, so the consumer is format-agnostic.
#[derive(Clone, Default)]
pub struct PortalCursorData {
    pub id: u64,
    pub width: i32,
    pub height: i32,
    pub hotx: i32,
    pub hoty: i32,
    pub colors: Vec<u8>,
}

/// Published when the compositor reports the cursor invisible, so the id changes and the client drops
/// the previous shape rather than keeping a stale visible cursor.
const HIDDEN_CURSOR_ID: u64 = u64::MAX;

// Mutter advertises the cursor meta sized for a 384x384 RGBA bitmap; KWin sizes it from its own
// bitmap. SPA_PARAM_META_size negotiates a fixed value, so a smaller request does not truncate the
// bitmap -- it drops the Cursor meta from the allocated buffers entirely. Ask for the larger of the
// two so both compositors attach it.
const MAX_CURSOR_DIMENSION: usize = 384;
const CURSOR_META_SIZE: usize = std::mem::size_of::<spa::sys::spa_meta_cursor>()
    + std::mem::size_of::<spa::sys::spa_meta_bitmap>()
    + MAX_CURSOR_DIMENSION * MAX_CURSOR_DIMENSION * 4;

static PORTAL_CURSOR: Mutex<Option<PortalCursorData>> = Mutex::new(None);
static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

struct Worker {
    stop: pw::channel::Sender<()>,
    thread: std::thread::JoinHandle<()>,
}

/// Latest cursor shape id, or `None` until the first shape arrives (caller then falls back to XFixes).
pub fn portal_cursor_id() -> Option<u64> {
    PORTAL_CURSOR
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|c| c.id))
}

/// Latest cursor shape, or `None` until the first shape arrives.
pub fn portal_cursor() -> Option<PortalCursorData> {
    PORTAL_CURSOR.lock().ok().and_then(|g| g.clone())
}

fn publish(cursor: PortalCursorData) {
    if let Ok(mut g) = PORTAL_CURSOR.lock() {
        *g = Some(cursor);
    }
}

fn clear() {
    if let Ok(mut g) = PORTAL_CURSOR.lock() {
        *g = None;
    }
}

/// Start the native-PipeWire cursor listener for a freshly created portal session. `fd` is the portal
/// remote fd (borrowed -- it is duplicated here, the caller keeps ownership); `node_ids` are the
/// ScreenCast stream node ids. No-op if a listener is already running or there are no nodes.
pub fn start_cursor_capture(fd: RawFd, node_ids: &[u64]) {
    if node_ids.is_empty() {
        return;
    }
    let mut worker = match WORKER.lock() {
        Ok(w) => w,
        Err(_) => return,
    };
    if worker.is_some() {
        return;
    }
    // The PipeWire core takes ownership of the fd it is given (it closes it on drop), so hand it a dup
    // and leave the portal session's original fd untouched.
    let dup_fd = unsafe { hbb_common::libc::dup(fd) };
    if dup_fd < 0 {
        warn!("portal-cursor: failed to dup portal fd, cursor shape will fall back to XFixes");
        return;
    }
    let owned = unsafe { OwnedFd::from_raw_fd(dup_fd) };
    let nodes: Vec<u32> = node_ids.iter().map(|n| *n as u32).collect();
    let (stop_tx, stop_rx) = pw::channel::channel::<()>();
    let thread = match std::thread::Builder::new()
        .name("portal-cursor".into())
        .spawn(move || {
            if let Err(e) = run_worker(owned, nodes, stop_rx) {
                warn!("portal-cursor: worker exited: {e}");
            }
            clear();
        }) {
        Ok(t) => t,
        Err(e) => {
            warn!("portal-cursor: failed to spawn worker thread: {e}");
            return;
        }
    };
    *worker = Some(Worker {
        stop: stop_tx,
        thread,
    });
}

/// Stop the listener (if any) and clear the published shape.
pub fn stop_cursor_capture() {
    let worker = match WORKER.lock() {
        Ok(mut w) => w.take(),
        Err(_) => None,
    };
    if let Some(worker) = worker {
        // Waking the loop makes it quit; the thread then returns from run().
        let _ = worker.stop.send(());
        let _ = worker.thread.join();
    }
    clear();
}

fn run_worker(
    fd: OwnedFd,
    node_ids: Vec<u32>,
    stop_rx: pw::channel::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error>> {
    pw::init();
    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_fd_rc(fd, None)?;

    // One stream per node; the cursor can sit on any of the shared displays and whichever stream sees
    // it updates the single global shape. Listeners must outlive `run()`, so keep them alive here.
    let mut streams = Vec::new();
    let mut listeners = Vec::new();
    for node in node_ids {
        let stream = pw::stream::StreamRc::new(
            core.clone(),
            "rustdesk-cursor",
            pw::properties::properties! {
                *pw::keys::MEDIA_TYPE => "Video",
                *pw::keys::MEDIA_CATEGORY => "Capture",
                *pw::keys::MEDIA_ROLE => "Screen",
            },
        )?;
        let listener = stream
            .add_local_listener_with_user_data(())
            .param_changed(|stream, _, id, param| {
                if param.is_none() || id != spa::param::ParamType::Format.as_raw() {
                    return;
                }
                if let Err(e) = negotiate_meta(stream) {
                    warn!("portal-cursor: meta negotiation failed: {e}");
                }
            })
            .process(|stream, _| {
                while let Some(buffer) = stream.dequeue_buffer() {
                    handle_buffer(&buffer);
                }
            })
            .register()?;

        let params = match build_format_params() {
            Ok(p) => p,
            Err(e) => {
                warn!("portal-cursor: failed to build format params: {e}");
                continue;
            }
        };
        let mut param_refs: Vec<&spa::pod::Pod> = params.iter().map(|p| p.as_ref()).collect();
        stream.connect(
            spa::utils::Direction::Input,
            Some(node),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut param_refs,
        )?;
        streams.push(stream);
        listeners.push(listener);
    }

    if streams.is_empty() {
        return Ok(());
    }

    let quit_loop = mainloop.clone();
    let _attached = stop_rx.attach(mainloop.loop_(), move |_| {
        quit_loop.quit();
    });

    mainloop.run();
    Ok(())
}

/// Owned pod bytes; `as_ref()` borrows a `&Pod` for the pipewire calls.
struct OwnedPod(Vec<u8>);

impl OwnedPod {
    fn as_ref(&self) -> &spa::pod::Pod {
        // The bytes were just produced by PodSerializer, so they are a valid serialized pod.
        spa::pod::Pod::from_bytes(&self.0).expect("serialized pod is valid")
    }
}

fn serialize_pod(value: &spa::pod::Value) -> Result<OwnedPod, Box<dyn std::error::Error>> {
    let bytes = spa::pod::serialize::PodSerializer::serialize(std::io::Cursor::new(Vec::new()), value)
        .map(|(cursor, _)| cursor.into_inner())
        .map_err(|e| format!("pod serialize: {e}"))?;
    Ok(OwnedPod(bytes))
}

/// A permissive raw-video EnumFormat: we do not consume the frames, but the node only produces buffers
/// (and thus attaches the cursor meta) once a format is negotiated.
fn build_format_params() -> Result<Vec<OwnedPod>, Box<dyn std::error::Error>> {
    use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    use spa::param::video::VideoFormat;
    use spa::pod::{property, Value};
    use spa::utils::{Fraction, Rectangle};

    let obj = spa::pod::object! {
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        property!(FormatProperties::MediaType, Id, MediaType::Video),
        property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::RGBA,
            VideoFormat::RGBA,
            VideoFormat::RGBx,
            VideoFormat::BGRA,
            VideoFormat::BGRx,
            VideoFormat::RGB,
            VideoFormat::BGR
        ),
        property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            Rectangle { width: 1920, height: 1080 },
            Rectangle { width: 1, height: 1 },
            Rectangle { width: 8192, height: 8192 }
        ),
        property!(
            FormatProperties::VideoFramerate,
            Choice,
            Range,
            Fraction,
            Fraction { num: 30, denom: 1 },
            Fraction { num: 0, denom: 1 },
            Fraction { num: 240, denom: 1 }
        ),
    };
    Ok(vec![serialize_pod(&Value::Object(obj))?])
}

/// Respond to a negotiated Format with Buffers + Meta params that reserve room for the cursor meta.
fn negotiate_meta(stream: &pw::stream::Stream) -> Result<(), Box<dyn std::error::Error>> {
    use spa::pod::{Object, Property, Value};
    use spa::utils::Id;

    let meta = Object {
        type_: spa::utils::SpaTypes::ObjectParamMeta.as_raw(),
        id: spa::param::ParamType::Meta.as_raw(),
        properties: vec![
            Property::new(
                spa::sys::SPA_PARAM_META_type,
                Value::Id(Id(spa::sys::SPA_META_Cursor)),
            ),
            Property::new(
                spa::sys::SPA_PARAM_META_size,
                Value::Int(CURSOR_META_SIZE as i32),
            ),
        ],
    };
    let pod = serialize_pod(&Value::Object(meta))?;
    let mut params = [pod.as_ref()];
    stream.update_params(&mut params)?;
    Ok(())
}

fn handle_buffer(buffer: &pw::buffer::Buffer) {
    use spa::buffer::meta::MetaCursor;

    let cursor = match buffer.find_meta::<MetaCursor>() {
        Some(c) => c,
        None => return,
    };
    // id == 0 means "no new cursor data"; leave the last shape untouched.
    if cursor.id() == 0 {
        return;
    }
    // bitmap_offset == 0 means position-only update, shape unchanged; the cursor position travels over
    // a separate service, so there is nothing for us to do.
    if cursor.bitmap_offset() == 0 {
        return;
    }
    let bitmap = match cursor.bitmap() {
        Some(b) if b.is_valid() => b,
        // Invalid/format 0 means the cursor is invisible: publish a hidden sentinel so the client drops
        // the stale shape instead of keeping the last visible one.
        _ => {
            publish(PortalCursorData {
                id: HIDDEN_CURSOR_ID,
                width: 0,
                height: 0,
                hotx: 0,
                hoty: 0,
                colors: Vec::new(),
            });
            return;
        }
    };

    let size = bitmap.size();
    let width = size.width as usize;
    let height = size.height as usize;
    let stride = bitmap.stride().unsigned_abs() as usize;
    if width == 0
        || height == 0
        || width > MAX_CURSOR_DIMENSION
        || height > MAX_CURSOR_DIMENSION
        || stride < width * 4
    {
        return;
    }
    let src = match bitmap.bitmap_data() {
        Some(d) if d.len() >= height * stride => d,
        _ => return,
    };

    let swap_rb = match bitmap.format() {
        f if f == spa::param::video::VideoFormat::RGBA
            || f == spa::param::video::VideoFormat::RGBx =>
        {
            false
        }
        f if f == spa::param::video::VideoFormat::BGRA
            || f == spa::param::video::VideoFormat::BGRx =>
        {
            true
        }
        // Unknown/unsupported layout: skip and let XFixes serve this frame.
        _ => return,
    };

    let mut colors = vec![0u8; width * height * 4];
    for y in 0..height {
        let row = &src[y * stride..y * stride + width * 4];
        for x in 0..width {
            let s = &row[x * 4..x * 4 + 4];
            let d = &mut colors[(y * width + x) * 4..(y * width + x) * 4 + 4];
            if swap_rb {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = s[3];
            } else {
                d[0] = s[0];
                d[1] = s[1];
                d[2] = s[2];
                d[3] = s[3];
            }
        }
    }

    let hotspot = cursor.hotspot();
    let hotx = hotspot.x;
    let hoty = hotspot.y;
    let id = shape_id(&colors, width as i32, height as i32, hotx, hoty);
    publish(PortalCursorData {
        id,
        width: width as i32,
        height: height as i32,
        hotx,
        hoty,
        colors,
    });
}

/// Synthetic shape id: FNV-1a over the pixels with geometry and hotspot folded in, so an identical
/// bitmap at a different size or hotspot counts as a new shape (the cursor service dedupes by id).
fn shape_id(colors: &[u8], width: i32, height: i32, hotx: i32, hoty: i32) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for b in colors {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    for v in [width, height, hotx, hoty] {
        hash ^= v as u32 as u64;
        hash = hash.wrapping_mul(PRIME);
    }
    // Never collide with the hidden sentinel.
    if hash == HIDDEN_CURSOR_ID {
        HIDDEN_CURSOR_ID - 1
    } else {
        hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_id_stable_and_geometry_sensitive() {
        let px = vec![10u8, 20, 30, 40, 50, 60, 70, 80];
        let a = shape_id(&px, 2, 1, 0, 0);
        assert_eq!(a, shape_id(&px, 2, 1, 0, 0), "same input, same id");
        assert_ne!(a, shape_id(&px, 1, 2, 0, 0), "geometry changes the id");
        assert_ne!(a, shape_id(&px, 2, 1, 3, 3), "hotspot changes the id");
        let mut px2 = px.clone();
        px2[0] = 11;
        assert_ne!(a, shape_id(&px2, 2, 1, 0, 0), "pixel change changes the id");
    }

    #[test]
    fn shape_id_never_hidden_sentinel() {
        // Exhaustive collisions are untestable, but the guard must map an exact hit off the sentinel.
        assert_ne!(shape_id(&[1, 2, 3, 4], 1, 1, 0, 0), HIDDEN_CURSOR_ID);
    }

    #[test]
    fn cursor_meta_size_covers_max_bitmap() {
        assert!(CURSOR_META_SIZE >= MAX_CURSOR_DIMENSION * MAX_CURSOR_DIMENSION * 4);
    }
}
