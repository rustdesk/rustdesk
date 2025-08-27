use crate::ipc::{self, new_listener, Connection, Data};
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail, log, sleep,
    tokio::{
        self,
        sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        time::interval_at,
    },
    ResultType,
};
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use softbuffer::{Context, Surface};
use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{Arc, RwLock},
    time::Instant,
};
#[cfg(target_os = "linux")]
use tao::platform::unix::WindowBuilderExtUnix;
#[cfg(target_os = "windows")]
use tao::platform::windows::WindowBuilderExtWindows;
use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    window::WindowBuilder,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Point, Stroke, Transform};
use ttf_parser::Face;

lazy_static! {
    static ref EVENT_PROXY: RwLock<Option<EventLoopProxy<(String, CustomEvent)>>> =
        RwLock::new(None);
    static ref TX_WHITEBOARD: RwLock<Option<UnboundedSender<(String, CustomEvent)>>> =
        RwLock::new(None);
    static ref CONNS: RwLock<HashMap<String, Conn>> = Default::default();
}

struct Conn {
    last_cursor_pos: (f32, f32), // For click ripple
    last_cursor_evt: LastCursorEvent,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t", content = "c")]
pub enum CustomEvent {
    Cursor(Cursor),
    Clear,
    Exit,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "t")]
pub struct Cursor {
    pub x: f32,
    pub y: f32,
    pub argb: u32,
    pub btns: i32,
    pub text: String,
}

struct LastCursorEvent {
    evt: Option<CustomEvent>,
    tm: Instant,
    c: usize,
}

// A helper struct to bridge `ttf-parser` and `tiny-skia`.
struct PathBuilderWrapper<'a> {
    path_builder: &'a mut PathBuilder,
    transform: Transform,
}

impl ttf_parser::OutlineBuilder for PathBuilderWrapper<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.move_to(pt.x, pt.y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.line_to(pt.x, pt.y);
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let mut pt1 = Point::from_xy(x1, y1);
        self.transform.map_point(&mut pt1);
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder.quad_to(pt1.x, pt1.y, pt.x, pt.y);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let mut pt1 = Point::from_xy(x1, y1);
        self.transform.map_point(&mut pt1);
        let mut pt2 = Point::from_xy(x2, y2);
        self.transform.map_point(&mut pt2);
        let mut pt = Point::from_xy(x, y);
        self.transform.map_point(&mut pt);
        self.path_builder
            .cubic_to(pt1.x, pt1.y, pt2.x, pt2.y, pt.x, pt.y);
    }

    fn close(&mut self) {
        self.path_builder.close();
    }
}

// Draws a string of text onto the pixmap.
fn draw_text(
    pixmap: &mut PixmapMut,
    face: &Face,
    text: &str,
    x: f32,
    y: f32,
    paint: &Paint,
    font_size: f32,
) {
    let units_per_em = face.units_per_em() as f32;
    let scale = font_size / units_per_em;
    let transform = Transform::from_translate(x, y).pre_scale(scale, -scale);

    let mut path_builder = PathBuilder::new();
    let mut current_x = 0.0;

    for ch in text.chars() {
        let glyph_id = face.glyph_index(ch).unwrap_or_default();

        let mut builder = PathBuilderWrapper {
            path_builder: &mut path_builder,
            transform: transform.post_translate(current_x, 0.0),
        };

        face.outline_glyph(glyph_id, &mut builder);

        if let Some(h_advance) = face.glyph_hor_advance(glyph_id) {
            current_x += h_advance as f32 * scale;
        }
    }

    if let Some(path) = path_builder.finish() {
        pixmap.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

#[inline]
pub fn get_key_cursor(conn_id: i32) -> String {
    format!("{}-cursor", conn_id)
}

pub fn register_whiteboard(k: String) {
    std::thread::spawn(|| {
        allow_err!(start_whiteboard_());
    });
    let mut conns = CONNS.write().unwrap();
    if !conns.contains_key(&k) {
        conns.insert(
            k,
            Conn {
                last_cursor_pos: (0.0, 0.0),
                last_cursor_evt: LastCursorEvent {
                    evt: None,
                    tm: Instant::now(),
                    c: 0,
                },
            },
        );
    }
}

pub fn unregister_whiteboard(k: String) {
    let mut conns = CONNS.write().unwrap();
    conns.remove(&k);
    let is_conns_empty = conns.is_empty();
    drop(conns);

    TX_WHITEBOARD.read().unwrap().as_ref().map(|tx| {
        allow_err!(tx.send((k, CustomEvent::Clear)));
    });
    if is_conns_empty {
        std::thread::spawn(|| {
            let mut whiteboard = TX_WHITEBOARD.write().unwrap();
            whiteboard.as_ref().map(|tx| {
                allow_err!(tx.send(("".to_string(), CustomEvent::Exit)));
                // Simple sleep to wait the whiteboard process exiting.
                std::thread::sleep(std::time::Duration::from_millis(3_00));
            });
            whiteboard.take();
        });
    }
}

pub fn update_whiteboard(k: String, e: CustomEvent) {
    let mut conns = CONNS.write().unwrap();
    let Some(conn) = conns.get_mut(&k) else {
        return;
    };
    match &e {
        CustomEvent::Cursor(cursor) => {
            conn.last_cursor_evt.c += 1;
            conn.last_cursor_evt.tm = Instant::now();
            if cursor.btns == 0 {
                // Send one movement event every 4.
                if conn.last_cursor_evt.c > 3 {
                    conn.last_cursor_evt.c = 0;
                    conn.last_cursor_evt.evt = None;
                    tx_send_event(conn, k, e);
                } else {
                    conn.last_cursor_evt.evt = Some(e);
                }
            } else {
                if let Some(evt) = conn.last_cursor_evt.evt.take() {
                    tx_send_event(conn, k.clone(), evt);
                    conn.last_cursor_evt.c = 0;
                }
                let click_evt = CustomEvent::Cursor(Cursor {
                    x: conn.last_cursor_pos.0,
                    y: conn.last_cursor_pos.1,
                    argb: cursor.argb,
                    btns: cursor.btns,
                    text: cursor.text.clone(),
                });
                tx_send_event(conn, k, click_evt);
            }
        }
        _ => {
            tx_send_event(conn, k, e);
        }
    }
}

#[inline]
fn tx_send_event(conn: &mut Conn, k: String, event: CustomEvent) {
    if let CustomEvent::Cursor(cursor) = &event {
        if cursor.btns == 0 {
            conn.last_cursor_pos = (cursor.x, cursor.y);
        }
    }

    TX_WHITEBOARD.read().unwrap().as_ref().map(|tx| {
        allow_err!(tx.send((k, event)));
    });
}

#[tokio::main(flavor = "current_thread")]
async fn start_whiteboard_() -> ResultType<()> {
    let mut tx_whiteboard = TX_WHITEBOARD.write().unwrap();
    if tx_whiteboard.is_some() {
        log::warn!("Whiteboard already started");
        return Ok(());
    }

    loop {
        if !crate::platform::is_prelogin() {
            break;
        }
        sleep(1.).await;
    }
    let mut stream = None;
    if let Ok(s) = ipc::connect(1000, "_whiteboard").await {
        stream = Some(s);
    } else {
        #[allow(unused_mut)]
        #[allow(unused_assignments)]
        let mut args = vec!["--whiteboard"];
        #[allow(unused_mut)]
        #[cfg(target_os = "linux")]
        let mut user = None;

        let run_done;
        if crate::platform::is_root() {
            let mut res = Ok(None);
            for _ in 0..10 {
                #[cfg(not(any(target_os = "linux")))]
                {
                    log::debug!("Start whiteboard");
                    res = crate::platform::run_as_user(args.clone());
                }
                #[cfg(target_os = "linux")]
                {
                    log::debug!("Start whiteboard");
                    res = crate::platform::run_as_user(
                        args.clone(),
                        user.clone(),
                        None::<(&str, &str)>,
                    );
                }
                if res.is_ok() {
                    break;
                }
                log::error!("Failed to run whiteboard: {res:?}");
                sleep(1.).await;
            }
            if let Some(task) = res? {
                super::CHILD_PROCESS.lock().unwrap().push(task);
            }
            run_done = true;
        } else {
            run_done = false;
        }
        if !run_done {
            log::debug!("Start whiteboard");
            super::CHILD_PROCESS
                .lock()
                .unwrap()
                .push(crate::run_me(args)?);
        }
        for _ in 0..20 {
            sleep(0.3).await;
            if let Ok(s) = ipc::connect(1000, "_whiteboard").await {
                stream = Some(s);
                break;
            }
        }
        if stream.is_none() {
            bail!("Failed to connect to connection manager");
        }
    }

    let mut stream = stream.ok_or(anyhow!("none stream"))?;
    let (tx, mut rx) = unbounded_channel();
    tx_whiteboard.replace(tx);
    drop(tx_whiteboard);
    let _call_on_ret = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let _ = TX_WHITEBOARD.write().unwrap().take();
        }),
    };

    let dur = tokio::time::Duration::from_millis(300);
    let mut timer = interval_at(tokio::time::Instant::now() + dur, dur);
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            res = rx.recv() => {
                match res {
                    Some(data) => {
                        if matches!(data.1, CustomEvent::Exit) {
                            break;
                        } else {
                            allow_err!(stream.send(&Data::Whiteboard(data)).await);
                            timer.reset();
                        }
                    }
                    None => {
                        bail!("expected");
                    }
                }
            },
            _ = timer.tick() => {
                let mut conns = CONNS.write().unwrap();
                for (k, conn) in conns.iter_mut() {
                    if conn.last_cursor_evt.tm.elapsed().as_millis() > 300 {
                        if let Some(evt) = conn.last_cursor_evt.evt.take() {
                            allow_err!(stream.send(&Data::Whiteboard((k.clone(), evt))).await);
                            conn.last_cursor_evt.c = 0;
                        }
                    }
                }
            }
        }
    }
    allow_err!(
        stream
            .send(&Data::Whiteboard(("".to_string(), CustomEvent::Exit)))
            .await
    );
    Ok(())
}

pub fn run() {
    let (tx_exit, rx_exit) = unbounded_channel();
    std::thread::spawn(move || {
        start_ipc(rx_exit);
    });
    if let Err(e) = create_event_loop() {
        log::error!("Failed to create event loop: {}", e);
        tx_exit.send(()).ok();
        return;
    }
}

#[tokio::main(flavor = "current_thread")]
async fn start_ipc(mut rx_exit: UnboundedReceiver<()>) {
    match new_listener("_whiteboard").await {
        Ok(mut incoming) => loop {
            tokio::select! {
                _ = rx_exit.recv() => {
                    log::info!("Exiting IPC");
                    break;
                }
                res = incoming.next() => match res {
                    Some(result) => match result {
                        Ok(stream) => {
                            log::debug!("Got new connection");
                            tokio::spawn(handle_new_stream(Connection::new(stream)));
                        }
                        Err(err) => {
                            log::error!("Couldn't get whiteboard client: {:?}", err);
                        }
                    },
                    None => {
                        log::error!("Failed to get whiteboard client");
                    }
                }
            }
        },
        Err(err) => {
            log::error!("Failed to start whiteboard ipc server: {}", err);
        }
    }
}

async fn handle_new_stream(mut conn: Connection) {
    loop {
        tokio::select! {
            res = conn.next() => {
                match res {
                    Err(err) => {
                        log::info!("whiteboard ipc connection closed: {}", err);
                        break;
                    }
                    Ok(Some(data)) => {
                        match data {
                            Data::Whiteboard((k, evt)) => {
                                if matches!(evt, CustomEvent::Exit) {
                                    log::info!("whiteboard ipc connection closed");
                                    break;
                                } else {
                                    EVENT_PROXY.read().unwrap().as_ref().map(|ep| {
                                        allow_err!(ep.send_event((k, evt)));
                                    });
                                }
                            }
                            _ => {

                            }
                        }
                    }
                    Ok(None) => {
                        log::info!("whiteboard ipc connection closed");
                        break;
                    }
                }
            }
        }
    }
    EVENT_PROXY.read().unwrap().as_ref().map(|ep| {
        allow_err!(ep.send_event(("".to_string(), CustomEvent::Exit)));
    });
}

fn create_font_face() -> ResultType<Face<'static>> {
    let mut font_db = fontdb::Database::new();
    font_db.load_system_fonts();
    let query = fontdb::Query {
        families: &[fontdb::Family::Monospace],
        ..fontdb::Query::default()
    };
    let Some(font_id) = font_db.query(&query) else {
        bail!("No monospace font found!");
    };
    let Some((font_source, face_index)) = font_db.face_source(font_id) else {
        bail!("No face found for font!");
    };
    let font_data: &'static [u8] = Box::leak(match font_source {
        fontdb::Source::File(path) => std::fs::read(path)?.into_boxed_slice(),
        fontdb::Source::Binary(data) => data.as_ref().as_ref().to_vec().into_boxed_slice(),
        fontdb::Source::SharedFile(path, _) => std::fs::read(path)?.into_boxed_slice(),
    });
    let face = Face::parse(font_data, face_index)?;
    Ok(face)
}

fn create_event_loop() -> ResultType<()> {
    let face = match create_font_face() {
        Ok(face) => Some(face),
        Err(err) => {
            log::error!("Failed to create font face: {}", err);
            None
        }
    };

    let event_loop = EventLoopBuilder::<(String, CustomEvent)>::with_user_event().build();
    let mut window_builder = WindowBuilder::new()
        .with_title("RustDesk whiteboard")
        .with_transparent(true)
        .with_always_on_top(true)
        .with_decorations(false);

    use tao::dpi::{PhysicalPosition, PhysicalSize};
    let mut final_size = None;
    if let Ok(displays) = crate::server::display_service::try_get_displays() {
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for display in displays {
            let (x, y) = (display.origin().0 as i32, display.origin().1 as i32);
            let (w, h) = (display.width() as i32, display.height() as i32);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + w);
            max_y = max_y.max(y + h);
        }

        let (x, y) = (min_x, min_y);
        let (w, h) = ((max_x - min_x) as u32, (max_y - min_y) as u32);

        if w > 0 && h > 0 {
            final_size = Some(PhysicalSize::new(w, h));
            window_builder = window_builder
                .with_position(PhysicalPosition::new(x, y))
                .with_inner_size(PhysicalSize::new(1, 1));
        } else {
            window_builder =
                window_builder.with_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
        }
    } else {
        window_builder =
            window_builder.with_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        window_builder = window_builder.with_skip_taskbar(true);
    }

    let window = Arc::new(window_builder.build::<(String, CustomEvent)>(&event_loop)?);
    window.set_ignore_cursor_events(true)?;

    let context = Context::new(window.clone()).map_err(|e| {
        log::error!("Failed to create context: {}", e);
        anyhow!(e.to_string())
    })?;
    let mut surface = Surface::new(&context, window.clone()).map_err(|e| {
        log::error!("Failed to create surface: {}", e);
        anyhow!(e.to_string())
    })?;

    let proxy = event_loop.create_proxy();
    EVENT_PROXY.write().unwrap().replace(proxy);
    let _call_on_ret = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let _ = EVENT_PROXY.write().unwrap().take();
        }),
    };

    struct Ripple {
        x: f32,
        y: f32,
        start_time: Instant,
    }
    let mut ripples: Vec<Ripple> = Vec::new();
    let mut last_cursors: HashMap<String, Cursor> = HashMap::new();
    let mut resized = final_size.is_none();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                if !resized {
                    if let Some(size) = final_size.take() {
                        window.set_inner_size(size);
                    }
                    resized = true;
                    return;
                }

                let (width, height) = {
                    let size = window.inner_size();
                    (size.width, size.height)
                };

                let (Some(width), Some(height)) = (NonZeroU32::new(width), NonZeroU32::new(height))
                else {
                    return;
                };
                if let Err(e) = surface.resize(width, height) {
                    log::error!("Failed to resize surface: {}", e);
                    return;
                }

                let mut buffer = match surface.buffer_mut() {
                    Ok(buf) => buf,
                    Err(e) => {
                        log::error!("Failed to get buffer: {}", e);
                        return;
                    }
                };
                let Some(mut pixmap) = PixmapMut::from_bytes(
                    bytemuck::cast_slice_mut(&mut buffer),
                    width.get(),
                    height.get(),
                ) else {
                    log::error!("Failed to create pixmap from buffer");
                    return;
                };
                pixmap.fill(Color::TRANSPARENT);

                let ripple_duration = std::time::Duration::from_millis(500);
                ripples.retain(|r| r.start_time.elapsed() < ripple_duration);

                for ripple in &ripples {
                    let elapsed = ripple.start_time.elapsed();
                    let progress = elapsed.as_secs_f32() / ripple_duration.as_secs_f32();
                    let radius = 45.0 * progress;
                    let alpha = 1.0 - progress;

                    let mut ripple_paint = Paint::default();
                    // Note: The real color is bgra here.
                    ripple_paint.set_color_rgba8(128, 128, 255, (alpha * 128.0) as u8);
                    ripple_paint.anti_alias = true;

                    let mut ripple_pb = PathBuilder::new();
                    let (rx, ry) = (ripple.x as f64, ripple.y as f64);
                    ripple_pb.push_circle(rx as f32, ry as f32, radius as f32);
                    if let Some(path) = ripple_pb.finish() {
                        pixmap.fill_path(
                            &path,
                            &ripple_paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );
                    }
                }

                for cursor in last_cursors.values() {
                    let (x, y) = (cursor.x as f64, cursor.y as f64);
                    let (x, y) = (x as f32, y as f32);
                    let size = 1.5 as f32;

                    let mut pb = PathBuilder::new();
                    pb.move_to(x, y);
                    pb.line_to(x, y + 16.0 * size);
                    pb.line_to(x + 4.0 * size, y + 13.0 * size);
                    pb.line_to(x + 7.0 * size, y + 20.0 * size);
                    pb.line_to(x + 9.0 * size, y + 19.0 * size);
                    pb.line_to(x + 6.0 * size, y + 12.0 * size);
                    pb.line_to(x + 11.0 * size, y + 12.0 * size);
                    pb.close();

                    if let Some(path) = pb.finish() {
                        let mut arrow_paint = Paint::default();
                        // Note: The real color is bgra here.
                        arrow_paint.set_color_rgba8(
                            (cursor.argb & 0xFF) as u8,
                            (cursor.argb >> 8 & 0xFF) as u8,
                            (cursor.argb >> 16 & 0xFF) as u8,
                            (cursor.argb >> 24 & 0xFF) as u8,
                        );
                        arrow_paint.anti_alias = true;
                        pixmap.fill_path(
                            &path,
                            &arrow_paint,
                            FillRule::Winding,
                            Transform::identity(),
                            None,
                        );

                        let mut black_paint = Paint::default();
                        black_paint.set_color_rgba8(0, 0, 0, 255);
                        black_paint.anti_alias = true;
                        let mut stroke = Stroke::default();
                        stroke.width = 1.0 as f32;
                        pixmap.stroke_path(
                            &path,
                            &black_paint,
                            &stroke,
                            Transform::identity(),
                            None,
                        );

                        face.as_ref().map(|face| {
                            draw_text(
                                &mut pixmap,
                                face,
                                &cursor.text,
                                x + 24.0 * size,
                                y + 24.0 * size,
                                &arrow_paint,
                                24.0 as f32,
                            );
                        });
                    }
                }

                if let Err(e) = buffer.present() {
                    log::error!("Failed to present surface: {}", e);
                    return;
                }
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::UserEvent((k, evt)) => match evt {
                CustomEvent::Cursor(cursor) => {
                    if cursor.btns != 0 {
                        ripples.push(Ripple {
                            x: cursor.x,
                            y: cursor.y,
                            start_time: Instant::now(),
                        });
                    }
                    last_cursors.insert(k, cursor);
                }
                CustomEvent::Exit => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            _ => (),
        }
    });
}
