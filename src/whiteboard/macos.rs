use super::{server::EVENT_PROXY, Cursor, CustomEvent, Ripple};
use core_graphics::context::CGContextRef;
use foreign_types::ForeignTypeRef;
use hbb_common::{bail, log, ResultType};
use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use piet::{
    kurbo::{BezPath, Point},
    FontFamily, RenderContext, Text, TextLayout, TextLayoutBuilder,
};
use piet_coregraphics::{CoreGraphicsContext, CoreGraphicsTextLayout};
use std::{collections::HashMap, sync::Arc, time::Instant};
use tao::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopBuilder},
    platform::macos::MonitorHandleExtMacOS,
    rwh_06::{HasWindowHandle, RawWindowHandle},
    window::{Window, WindowBuilder, WindowId},
};

const MAXIMUM_WINDOW_LEVEL: i64 = 2147483647;
const CURSOR_TEXT_FONT_SIZE: f64 = 14.0;
const CURSOR_TEXT_OFFSET: f64 = 20.0;

struct WindowState {
    window: Arc<Window>,
    logical_size: LogicalSize<f64>,
    outer_position: PhysicalPosition<i32>,
    // A simple workaround to the (logical) cursor position.
    display_origin: (f64, f64),
}

struct CursorInfo {
    window_id: WindowId,
    text_key: (String, u32),
    cursor: Cursor,
}

fn set_window_properties(window: &Arc<Window>) -> ResultType<()> {
    let handle = window.window_handle()?;
    if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
        unsafe {
            let ns_view = appkit_handle.ns_view.as_ptr() as *mut Object;
            if ns_view.is_null() {
                bail!("Ns view of the window handle is null.");
            }
            let ns_window: *mut Object = msg_send![ns_view, window];
            if ns_window.is_null() {
                bail!("Ns window of the ns view is null.");
            }
            let _: () = msg_send![ns_window, setOpaque: false];
            let _: () = msg_send![ns_window, setLevel: MAXIMUM_WINDOW_LEVEL];
            // NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorIgnoresCycle
            let _: () = msg_send![ns_window, setCollectionBehavior: 5];
            let current_style_mask: u64 = msg_send![ns_window, styleMask];
            // NSWindowStyleMaskNonactivatingPanel
            let new_style_mask = current_style_mask | (1 << 7);
            let _: () = msg_send![ns_window, setStyleMask: new_style_mask];
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
        }
    }
    Ok(())
}

fn create_windows(event_loop: &EventLoop<(String, CustomEvent)>) -> ResultType<Vec<WindowState>> {
    let mut windows = Vec::new();
    let map_display_origins: HashMap<_, _> = crate::server::display_service::try_get_displays()?
        .into_iter()
        .map(|display| (display.name(), display.origin()))
        .collect();
    // We can't use `crate::server::display_service::try_get_displays()` here.
    // Because the `display` returned by `crate::server::display_service::try_get_displays()`:
    // 1. `display.origin()` is the logic position.
    // 2. `display.width()` and `display.height()` are the physical size.
    for monitor in event_loop.available_monitors() {
        let Some(origin) = map_display_origins.get(&monitor.native_id().to_string()) else {
            // unreachable!
            bail!(
                "Failed to find display origin for monitor: {}",
                monitor.native_id()
            );
        };

        let window_builder = WindowBuilder::new()
            .with_title("RustDesk whiteboard")
            .with_transparent(true)
            .with_decorations(false)
            .with_position(monitor.position())
            .with_inner_size(monitor.size());

        let window = Arc::new(window_builder.build::<(String, CustomEvent)>(event_loop)?);
        set_window_properties(&window)?;

        let mut scale_factor = window.scale_factor();
        if scale_factor == 0.0 {
            scale_factor = 1.0;
        }
        let physical_size = window.inner_size();
        let logical_size = physical_size.to_logical::<f64>(scale_factor);
        let inner_position = window.inner_position()?;
        let outer_position = inner_position;
        windows.push(WindowState {
            window,
            logical_size,
            outer_position,
            display_origin: (origin.0 as f64, origin.1 as f64),
        });
    }
    Ok(windows)
}

fn draw_cursors(
    windows: &Vec<WindowState>,
    window_id: WindowId,
    window_ripples: &mut HashMap<WindowId, Vec<Ripple>>,
    last_cursors: &HashMap<String, CursorInfo>,
    map_cursor_text: &mut HashMap<(String, u32), CoreGraphicsTextLayout>,
) {
    for window in windows.iter() {
        if window.window.id() != window_id {
            continue;
        }

        if let Ok(handle) = window.window.window_handle() {
            if let RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
                unsafe {
                    let ns_view = appkit_handle.ns_view.as_ptr() as *mut Object;
                    let current_context: *mut Object =
                        msg_send![class!(NSGraphicsContext), currentContext];
                    if !current_context.is_null() {
                        let cg_context_ptr: *mut std::ffi::c_void =
                            msg_send![current_context, CGContext];
                        if !cg_context_ptr.is_null() {
                            let cg_context_ref =
                                CGContextRef::from_ptr_mut(cg_context_ptr as *mut _);
                            let mut context = CoreGraphicsContext::new_y_up(
                                cg_context_ref,
                                window.logical_size.height,
                                None,
                            );
                            context.clear(None, piet::Color::TRANSPARENT);

                            if let Some(ripples) = window_ripples.get_mut(&window_id) {
                                Ripple::retain_active(ripples);
                                for ripple in ripples.iter() {
                                    let (radius, alpha) = ripple.get_radius_alpha();
                                    let color = piet::Color::rgba(1.0, 0.25, 0.25, alpha * 0.5);
                                    let circle =
                                        piet::kurbo::Circle::new((ripple.x, ripple.y), radius);
                                    context.stroke(circle, &color, 2.0);
                                }
                            }

                            for info in last_cursors.values() {
                                if info.window_id != window.window.id() {
                                    continue;
                                }
                                let cursor = &info.cursor;

                                let (x, y) = (cursor.x as f64, cursor.y as f64);
                                let size = 1.0;

                                let mut pb = BezPath::new();
                                pb.move_to((x, y));
                                pb.line_to((x, y + 16.0 * size));
                                pb.line_to((x + 4.0 * size, y + 13.0 * size));
                                pb.line_to((x + 7.0 * size, y + 20.0 * size));
                                pb.line_to((x + 9.0 * size, y + 19.0 * size));
                                pb.line_to((x + 6.0 * size, y + 12.0 * size));
                                pb.line_to((x + 11.0 * size, y + 12.0 * size));

                                let rgba = super::argb_to_rgba(cursor.argb);
                                let color = piet::Color::rgba8(rgba.0, rgba.1, rgba.2, rgba.3);
                                context.fill(pb, &color);

                                let pos =
                                    (x + CURSOR_TEXT_OFFSET * size, y + CURSOR_TEXT_OFFSET * size);
                                let get_rounded_rect = |layout: &CoreGraphicsTextLayout| {
                                    let text_pos = Point::new(pos.0, pos.1);
                                    let padded_bounds = (layout.image_bounds()
                                        + text_pos.to_vec2())
                                    .inflate(3.0, 3.0);
                                    padded_bounds.to_rounded_rect(5.0)
                                };

                                if let Some(layout) = map_cursor_text.get(&info.text_key) {
                                    context.fill(get_rounded_rect(layout), &piet::Color::WHITE);
                                    context.draw_text(layout, pos);
                                } else {
                                    let text = context.text();
                                    let color = piet::Color::rgba8(0, 0, 0, 255);
                                    if let Ok(layout) = text
                                        .new_text_layout(cursor.text.clone())
                                        .font(FontFamily::SYSTEM_UI, CURSOR_TEXT_FONT_SIZE)
                                        .text_color(color)
                                        .build()
                                    {
                                        context
                                            .fill(get_rounded_rect(&layout), &piet::Color::WHITE);
                                        context.draw_text(&layout, pos);
                                        map_cursor_text.insert(info.text_key.clone(), layout);
                                    }
                                }
                            }
                            if let Err(e) = context.finish() {
                                log::error!("Failed to draw cursor: {}", e);
                            }
                        } else {
                            log::warn!("CGContext is null");
                        }
                    }
                    let _: () = msg_send![ns_view, setNeedsDisplay:true];
                }
            }
        }
    }
}

pub(super) fn create_event_loop() -> ResultType<()> {
    crate::platform::hide_dock();
    let event_loop = EventLoopBuilder::<(String, CustomEvent)>::with_user_event().build();

    let windows = create_windows(&event_loop)?;

    let proxy = event_loop.create_proxy();
    EVENT_PROXY.write().unwrap().replace(proxy);
    let _call_on_ret = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let _ = EVENT_PROXY.write().unwrap().take();
        }),
    };

    let mut window_ripples: HashMap<WindowId, Vec<Ripple>> = HashMap::new();
    let mut last_cursors: HashMap<String, CursorInfo> = HashMap::new();
    let mut map_cursor_text: HashMap<(String, u32), CoreGraphicsTextLayout> = HashMap::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::NewEvents(StartCause::Init) => {
                for window in windows.iter() {
                    window.window.set_outer_position(window.outer_position);
                    window.window.request_redraw();
                }
                crate::platform::hide_dock();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::RedrawRequested(window_id) => {
                draw_cursors(
                    &windows,
                    window_id,
                    &mut window_ripples,
                    &last_cursors,
                    &mut map_cursor_text,
                );
            }
            Event::MainEventsCleared => {
                for window in windows.iter() {
                    window.window.request_redraw();
                }
            }
            Event::UserEvent((k, evt)) => match evt {
                CustomEvent::Cursor(cursor) => {
                    for window in windows.iter() {
                        let (l, t, r, b) = (
                            window.display_origin.0,
                            window.display_origin.1,
                            window.display_origin.0 + window.logical_size.width,
                            window.display_origin.1 + window.logical_size.height,
                        );
                        if (cursor.x as f64) < l
                            || (cursor.x as f64) > r
                            || (cursor.y as f64) < t
                            || (cursor.y as f64) > b
                        {
                            continue;
                        }

                        if cursor.btns != 0 {
                            let window_id = window.window.id();
                            let ripple = Ripple {
                                x: (cursor.x as f64 - window.display_origin.0),
                                y: (cursor.y as f64 - window.display_origin.1),
                                start_time: Instant::now(),
                            };
                            if let Some(ripples) = window_ripples.get_mut(&window_id) {
                                ripples.push(ripple);
                            } else {
                                window_ripples.insert(window_id, vec![ripple]);
                            }
                        }
                        last_cursors.insert(
                            k,
                            CursorInfo {
                                window_id: window.window.id(),
                                text_key: (cursor.text.clone(), cursor.argb),
                                cursor: Cursor {
                                    x: (cursor.x - window.display_origin.0 as f32),
                                    y: (cursor.y - window.display_origin.1 as f32),
                                    ..cursor
                                },
                            },
                        );
                        window.window.request_redraw();
                        break;
                    }
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
