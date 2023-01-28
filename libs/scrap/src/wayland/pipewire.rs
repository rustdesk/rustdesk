use std::collections::HashMap;
use std::error::Error;
use std::os::unix::io::AsRawFd;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::Duration;
use tracing::{debug, trace, warn};

use dbus::{
    arg::{OwnedFd, PropMap, RefArg, Variant},
    blocking::{Proxy, SyncConnection},
    message::{MatchRule, MessageType},
    Message,
};

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app::AppSink;

use hbb_common::config;

use super::capturable::PixelProvider;
use super::capturable::{Capturable, Recorder};
use super::pipewire_dbus::{OrgFreedesktopPortalRequestResponse, OrgFreedesktopPortalScreenCast};

#[derive(Debug, Clone, Copy)]
struct PwStreamInfo {
    path: u64,
    source_type: u64,
    position: (i32, i32),
    size: (usize, usize),
}

#[derive(Debug)]
pub struct DBusError(String);

impl std::fmt::Display for DBusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(s) = self;
        write!(f, "{}", s)
    }
}

impl Error for DBusError {}

#[derive(Debug)]
pub struct GStreamerError(String);

impl std::fmt::Display for GStreamerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self(s) = self;
        write!(f, "{}", s)
    }
}

impl Error for GStreamerError {}

#[derive(Clone)]
pub struct PipeWireCapturable {
    // connection needs to be kept alive for recording
    dbus_conn: Arc<SyncConnection>,
    fd: OwnedFd,
    path: u64,
    source_type: u64,
    pub position: (i32, i32),
    pub size: (usize, usize),
}

impl PipeWireCapturable {
    fn new(conn: Arc<SyncConnection>, fd: OwnedFd, stream: PwStreamInfo) -> Self {
        Self {
            dbus_conn: conn,
            fd,
            path: stream.path,
            source_type: stream.source_type,
            position: stream.position,
            size: stream.size,
        }
    }
}

impl std::fmt::Debug for PipeWireCapturable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PipeWireCapturable {{dbus: {}, fd: {}, path: {}, source_type: {}}}",
            self.dbus_conn.unique_name(),
            self.fd.as_raw_fd(),
            self.path,
            self.source_type
        )
    }
}

impl Capturable for PipeWireCapturable {
    fn name(&self) -> String {
        let type_str = match self.source_type {
            1 => "Desktop",
            2 => "Window",
            _ => "Unknow",
        };
        format!("Pipewire {}, path: {}", type_str, self.path)
    }

    fn geometry_relative(&self) -> Result<(f64, f64, f64, f64), Box<dyn Error>> {
        Ok((0.0, 0.0, 1.0, 1.0))
    }

    fn before_input(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn recorder(&self, _capture_cursor: bool) -> Result<Box<dyn Recorder>, Box<dyn Error>> {
        Ok(Box::new(PipeWireRecorder::new(self.clone())?))
    }
}

pub struct PipeWireRecorder {
    buffer: Option<gst::MappedBuffer<gst::buffer::Readable>>,
    buffer_cropped: Vec<u8>,
    pix_fmt: String,
    is_cropped: bool,
    pipeline: gst::Pipeline,
    appsink: AppSink,
    width: usize,
    height: usize,
    saved_raw_data: Vec<u8>, // for faster compare and copy
}

impl PipeWireRecorder {
    pub fn new(capturable: PipeWireCapturable) -> Result<Self, Box<dyn Error>> {
        let pipeline = gst::Pipeline::new(None);

        let src = gst::ElementFactory::make("pipewiresrc", None)?;
        src.set_property("fd", &capturable.fd.as_raw_fd())?;
        src.set_property("path", &format!("{}", capturable.path))?;
        src.set_property("keepalive_time", &1_000.as_raw_fd())?;

        // For some reason pipewire blocks on destruction of AppSink if this is not set to true,
        // see: https://gitlab.freedesktop.org/pipewire/pipewire/-/issues/982
        src.set_property("always-copy", &true)?;

        let sink = gst::ElementFactory::make("appsink", None)?;
        sink.set_property("drop", &true)?;
        sink.set_property("max-buffers", &1u32)?;

        pipeline.add_many(&[&src, &sink])?;
        src.link(&sink)?;

        let appsink = sink
            .dynamic_cast::<AppSink>()
            .map_err(|_| GStreamerError("Sink element is expected to be an appsink!".into()))?;
        let mut caps = gst::Caps::new_empty();
        caps.merge_structure(gst::structure::Structure::new(
            "video/x-raw",
            &[("format", &"BGRx")],
        ));
        caps.merge_structure(gst::structure::Structure::new(
            "video/x-raw",
            &[("format", &"RGBx")],
        ));
        appsink.set_caps(Some(&caps));

        pipeline.set_state(gst::State::Playing)?;
        Ok(Self {
            pipeline,
            appsink,
            buffer: None,
            pix_fmt: "".into(),
            width: 0,
            height: 0,
            buffer_cropped: vec![],
            is_cropped: false,
            saved_raw_data: Vec::new(),
        })
    }
}

impl Recorder for PipeWireRecorder {
    fn capture(&mut self, timeout_ms: u64) -> Result<PixelProvider, Box<dyn Error>> {
        if let Some(sample) = self
            .appsink
            .try_pull_sample(gst::ClockTime::from_mseconds(timeout_ms))
        {
            let cap = sample
                .get_caps()
                .ok_or("Failed get caps")?
                .get_structure(0)
                .ok_or("Failed to get structure")?;
            let w: i32 = cap.get_value("width")?.get_some()?;
            let h: i32 = cap.get_value("height")?.get_some()?;
            let w = w as usize;
            let h = h as usize;
            self.pix_fmt = cap
                .get::<&str>("format")?
                .ok_or("Failed to get pixel format")?
                .to_string();

            let buf = sample
                .get_buffer_owned()
                .ok_or_else(|| GStreamerError("Failed to get owned buffer.".into()))?;
            let mut crop = buf
                .get_meta::<gstreamer_video::VideoCropMeta>()
                .map(|m| m.get_rect());
            // only crop if necessary
            if Some((0, 0, w as u32, h as u32)) == crop {
                crop = None;
            }
            let buf = buf
                .into_mapped_buffer_readable()
                .map_err(|_| GStreamerError("Failed to map buffer.".into()))?;
            if let Err(..) = crate::would_block_if_equal(&mut self.saved_raw_data, buf.as_slice()) {
                return Ok(PixelProvider::NONE);
            }
            let buf_size = buf.get_size();
            // BGRx is 4 bytes per pixel
            if buf_size != (w * h * 4) {
                // for some reason the width and height of the caps do not guarantee correct buffer
                // size, so ignore those buffers, see:
                // https://gitlab.freedesktop.org/pipewire/pipewire/-/issues/985
                trace!(
                    "Size of mapped buffer: {} does NOT match size of capturable {}x{}@BGRx, \
                    dropping it!",
                    buf_size,
                    w,
                    h
                );
            } else {
                // Copy region specified by crop into self.buffer_cropped
                // TODO: Figure out if ffmpeg provides a zero copy alternative
                if let Some((x_off, y_off, w_crop, h_crop)) = crop {
                    let x_off = x_off as usize;
                    let y_off = y_off as usize;
                    let w_crop = w_crop as usize;
                    let h_crop = h_crop as usize;
                    self.buffer_cropped.clear();
                    let data = buf.as_slice();
                    // BGRx is 4 bytes per pixel
                    self.buffer_cropped.reserve(w_crop * h_crop * 4);
                    for y in y_off..(y_off + h_crop) {
                        let i = 4 * (w * y + x_off);
                        self.buffer_cropped.extend(&data[i..i + 4 * w_crop]);
                    }
                    self.width = w_crop;
                    self.height = h_crop;
                } else {
                    self.width = w;
                    self.height = h;
                }
                self.is_cropped = crop.is_some();
                self.buffer = Some(buf);
            }
        } else {
            return Ok(PixelProvider::NONE);
        }
        if self.buffer.is_none() {
            return Err(Box::new(GStreamerError("No buffer available!".into())));
        }
        let buf = if self.is_cropped {
            self.buffer_cropped.as_slice()
        } else {
            self.buffer
                .as_ref()
                .ok_or("Failed to get buffer as ref")?
                .as_slice()
        };
        match self.pix_fmt.as_str() {
            "BGRx" => Ok(PixelProvider::BGR0(self.width, self.height, buf)),
            "RGBx" => Ok(PixelProvider::RGB0(self.width, self.height, buf)),
            _ => Err(Box::new(GStreamerError(format!(
                "Unreachable! Unknown pix_fmt, {}",
                &self.pix_fmt
            )))),
        }
    }
}

impl Drop for PipeWireRecorder {
    fn drop(&mut self) {
        if let Err(err) = self.pipeline.set_state(gst::State::Null) {
            warn!("Failed to stop GStreamer pipeline: {}.", err);
        }
    }
}

fn handle_response<F>(
    conn: &SyncConnection,
    path: dbus::Path<'static>,
    mut f: F,
    failure_out: Arc<AtomicBool>,
) -> Result<dbus::channel::Token, dbus::Error>
where
    F: FnMut(
            OrgFreedesktopPortalRequestResponse,
            &SyncConnection,
            &Message,
        ) -> Result<(), Box<dyn Error>>
        + Send
        + Sync
        + 'static,
{
    let mut m = MatchRule::new();
    m.path = Some(path);
    m.msg_type = Some(MessageType::Signal);
    m.sender = Some("org.freedesktop.portal.Desktop".into());
    m.interface = Some("org.freedesktop.portal.Request".into());
    conn.add_match(m, move |r: OrgFreedesktopPortalRequestResponse, c, m| {
        debug!("Response from DBus: response: {:?}, message: {:?}", r, m);
        match r.response {
            0 => {}
            1 => {
                warn!("DBus response: User cancelled interaction.");
                failure_out.store(true, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
            c => {
                warn!("DBus response: Unknown error, code: {}.", c);
                failure_out.store(true, std::sync::atomic::Ordering::Relaxed);
                return true;
            }
        }
        if let Err(err) = f(r, c, m) {
            warn!("Error requesting screen capture via dbus: {}", err);
            failure_out.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        true
    })
}

fn get_portal(conn: &SyncConnection) -> Proxy<&SyncConnection> {
    conn.with_proxy(
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        Duration::from_millis(1000),
    )
}

fn streams_from_response(response: OrgFreedesktopPortalRequestResponse) -> Vec<PwStreamInfo> {
    (move || {
        Some(
            response
                .results
                .get("streams")?
                .as_iter()?
                .next()?
                .as_iter()?
                .filter_map(|stream| {
                    let mut itr = stream.as_iter()?;
                    let path = itr.next()?.as_u64()?;
                    let (keys, values): (Vec<(usize, &dyn RefArg)>, Vec<(usize, &dyn RefArg)>) =
                        itr.next()?
                            .as_iter()?
                            .enumerate()
                            .partition(|(i, _)| i % 2 == 0);
                    let attributes = keys
                        .iter()
                        .filter_map(|(_, key)| Some(key.as_str()?.to_owned()))
                        .zip(
                            values
                                .iter()
                                .map(|(_, arg)| *arg)
                                .collect::<Vec<&dyn RefArg>>(),
                        )
                        .collect::<HashMap<String, &dyn RefArg>>();
                    let mut info = PwStreamInfo {
                        path,
                        source_type: attributes
                            .get("source_type")
                            .map_or(Some(0), |v| v.as_u64())?,
                        position: (0, 0),
                        size: (0, 0),
                    };
                    let v = attributes
                        .get("size")?
                        .as_iter()?
                        .filter_map(|v| {
                            Some(
                                v.as_iter()?
                                    .map(|x| x.as_i64().unwrap_or(0))
                                    .collect::<Vec<i64>>(),
                            )
                        })
                        .next();
                    if let Some(v) = v {
                        if v.len() == 2 {
                            info.size.0 = v[0] as _;
                            info.size.1 = v[1] as _;
                        }
                    }
                    let v = attributes
                        .get("position")?
                        .as_iter()?
                        .filter_map(|v| {
                            Some(
                                v.as_iter()?
                                    .map(|x| x.as_i64().unwrap_or(0))
                                    .collect::<Vec<i64>>(),
                            )
                        })
                        .next();
                    if let Some(v) = v {
                        if v.len() == 2 {
                            info.position.0 = v[0] as _;
                            info.position.1 = v[1] as _;
                        }
                    }
                    Some(info)
                })
                .collect::<Vec<PwStreamInfo>>(),
        )
    })()
    .unwrap_or_default()
}

static mut INIT: bool = false;
const RESTORE_TOKEN: &str = "restore_token";
const RESTORE_TOKEN_CONF_KEY: &str = "wayland-restore-token";

// mostly inspired by https://gitlab.gnome.org/snippets/19
fn request_screen_cast(
    capture_cursor: bool,
) -> Result<(SyncConnection, OwnedFd, Vec<PwStreamInfo>), Box<dyn Error>> {
    unsafe {
        if !INIT {
            gstreamer::init()?;
            INIT = true;
        }
    }
    let conn = SyncConnection::new_session()?;
    let portal = get_portal(&conn);
    let mut args: PropMap = HashMap::new();
    let fd: Arc<Mutex<Option<OwnedFd>>> = Arc::new(Mutex::new(None));
    let fd_res = fd.clone();
    let streams: Arc<Mutex<Vec<PwStreamInfo>>> = Arc::new(Mutex::new(Vec::new()));
    let streams_res = streams.clone();
    let failure = Arc::new(AtomicBool::new(false));
    let failure_res = failure.clone();
    args.insert(
        "session_handle_token".to_string(),
        Variant(Box::new("u1".to_string())),
    );
    args.insert(
        "handle_token".to_string(),
        Variant(Box::new("u1".to_string())),
    );
    // The following code may be improved.
    // https://flatpak.github.io/xdg-desktop-portal/#:~:text=To%20avoid%20a%20race%20condition
    // To avoid a race condition
    // between the caller subscribing to the signal after receiving the reply for the method call and the signal getting emitted,
    // a convention for Request object paths has been established that allows
    // the caller to subscribe to the signal before making the method call.
    let path = portal.create_session(args)?;
    handle_response(
        &conn,
        path,
        move |r: OrgFreedesktopPortalRequestResponse, c, _| {
            let portal = get_portal(c);
            let mut args: PropMap = HashMap::new();
            if let Ok(version) = portal.version() {
                if version >= 4 {
                    let restore_token = config::LocalConfig::get_option(RESTORE_TOKEN_CONF_KEY);
                    if !restore_token.is_empty() {
                        args.insert(RESTORE_TOKEN.to_string(), Variant(Box::new(restore_token)));
                    }
                    // persist_mode may be configured by the user.
                    args.insert("persist_mode".to_string(), Variant(Box::new(2u32)));
                }
            }
            args.insert(
                "handle_token".to_string(),
                Variant(Box::new("u2".to_string())),
            );
            // https://flatpak.github.io/xdg-desktop-portal/portal-docs.html#gdbus-method-org-freedesktop-portal-ScreenCast.SelectSources
            args.insert("multiple".into(), Variant(Box::new(true)));
            args.insert("types".into(), Variant(Box::new(1u32))); //| 2u32)));

            let mut cursor_mode = 0u32;
            let mut available_cursor_modes = 0u32;
            if let Ok(modes) = portal.available_cursor_modes() {
                available_cursor_modes = modes;
            }
            if capture_cursor {
                cursor_mode = 2u32 & available_cursor_modes;
            }
            if cursor_mode == 0 {
                cursor_mode = 1u32 & available_cursor_modes;
            }
            let plasma = std::env::var("DESKTOP_SESSION").map_or(false, |s| s.contains("plasma"));
            if plasma && capture_cursor {
                // Warn the user if capturing the cursor is tried on kde as this can crash
                // kwin_wayland and tear down the plasma desktop, see:
                // https://bugs.kde.org/show_bug.cgi?id=435042
                warn!("You are attempting to capture the cursor under KDE Plasma, this may crash your \
                    desktop, see https://bugs.kde.org/show_bug.cgi?id=435042 for details! \
                    You have been warned.");
            }
            if cursor_mode > 0 {
                args.insert("cursor_mode".into(), Variant(Box::new(cursor_mode)));
            }
            let session: dbus::Path = r
                .results
                .get("session_handle")
                .ok_or_else(|| {
                    DBusError(format!(
                        "Failed to obtain session_handle from response: {:?}",
                        r
                    ))
                })?
                .as_str()
                .ok_or_else(|| DBusError("Failed to convert session_handle to string.".into()))?
                .to_string()
                .into();
            let path = portal.select_sources(session.clone(), args)?;
            let fd = fd.clone();
            let streams = streams.clone();
            let failure = failure.clone();
            let failure_out = failure.clone();
            handle_response(
                c,
                path,
                move |_: OrgFreedesktopPortalRequestResponse, c, _| {
                    let portal = get_portal(c);
                    let mut args: PropMap = HashMap::new();
                    args.insert(
                        "handle_token".to_string(),
                        Variant(Box::new("u3".to_string())),
                    );
                    let path = portal.start(session.clone(), "", args)?;
                    let session = session.clone();
                    let fd = fd.clone();
                    let streams = streams.clone();
                    let failure = failure.clone();
                    let failure_out = failure.clone();
                    handle_response(
                        c,
                        path,
                        move |r: OrgFreedesktopPortalRequestResponse, c, _| {
                            let portal = get_portal(c);
                            if let Ok(version) = portal.version() {
                                if version >= 4 {
                                    if let Some(restore_token) = r.results.get(RESTORE_TOKEN) {
                                        if let Some(restore_token) = restore_token.as_str() {
                                            config::LocalConfig::set_option(
                                                RESTORE_TOKEN_CONF_KEY.to_owned(),
                                                restore_token.to_owned(),
                                            );
                                        }
                                    }
                                }
                            }
                            streams
                                .clone()
                                .lock()
                                .unwrap()
                                .append(&mut streams_from_response(r));
                            fd.clone().lock().unwrap().replace(
                                portal.open_pipe_wire_remote(session.clone(), HashMap::new())?,
                            );
                            Ok(())
                        },
                        failure_out,
                    )?;
                    Ok(())
                },
                failure_out,
            )?;
            Ok(())
        },
        failure_res.clone(),
    )?;
    // wait 3 minutes for user interaction
    for _ in 0..1800 {
        conn.process(Duration::from_millis(100))?;
        // Once we got a file descriptor we are done!
        if fd_res.lock().unwrap().is_some() {
            break;
        }

        if failure_res.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
    }
    let fd_res = fd_res.lock().unwrap();
    let streams_res = streams_res.lock().unwrap();
    if fd_res.is_some() && !streams_res.is_empty() {
        Ok((conn, fd_res.clone().unwrap(), streams_res.clone()))
    } else {
        Err(Box::new(DBusError(
            "Failed to obtain screen capture.".into(),
        )))
    }
}

pub fn get_capturables(capture_cursor: bool) -> Result<Vec<PipeWireCapturable>, Box<dyn Error>> {
    let (conn, fd, streams) = request_screen_cast(capture_cursor)?;
    let conn = Arc::new(conn);
    Ok(streams
        .into_iter()
        .map(|s| PipeWireCapturable::new(conn.clone(), fd.clone(), s))
        .collect())
}
