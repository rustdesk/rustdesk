use super::{server::EVENT_PROXY, Cursor, CustomEvent};
use core_graphics::context::CGContextRef;
use foreign_types::ForeignTypeRef;
use hbb_common::{bail, log, ResultType};
use objc::{class, msg_send, runtime::Object, sel, sel_impl};
use piet::{kurbo::BezPath, RenderContext};
use piet_coregraphics::CoreGraphicsContext;
use std::{collections::HashMap, sync::Arc, time::Instant};
use tao::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    rwh_06::{HasWindowHandle, RawWindowHandle},
    window::{Window, WindowBuilder},
};

const MAXIMUM_WINDOW_LEVEL: i64 = 2147483647;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct NSRect {
    origin: NSPoint,
    size: NSSize,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct NSPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct NSSize {
    width: f64,
    height: f64,
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
            let ns_screen_class = class!(NSScreen);
            let main_screen: *mut Object = msg_send![ns_screen_class, mainScreen];
            let screen_frame: NSRect = msg_send![main_screen, frame];
            let _: () = msg_send![ns_window, setFrame: screen_frame display: true];
            let ns_color_class = class!(NSColor);
            let clear_color: *mut Object = msg_send![ns_color_class, clearColor];
            let _: () = msg_send![ns_window, setBackgroundColor: clear_color];
            let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
        }
    }
    Ok(())
}

pub(super) fn create_event_loop() -> ResultType<()> {
    crate::platform::hide_dock();
    let event_loop = EventLoopBuilder::<(String, CustomEvent)>::with_user_event().build();
    let mut window_builder = WindowBuilder::new()
        .with_title("RustDesk whiteboard")
        .with_transparent(true)
        .with_decorations(false);

    let (x, y, w, h) = super::server::get_displays_rect()?;
    if w > 0 && h > 0 {
        window_builder = window_builder
            .with_position(PhysicalPosition::new(x, y))
            .with_inner_size(PhysicalSize::new(w, h));
    } else {
        bail!("No valid display found, wxh: {}x{}", w, h);
    }

    let window = Arc::new(window_builder.build::<(String, CustomEvent)>(&event_loop)?);
    set_window_properties(&window)?;

    let proxy = event_loop.create_proxy();
    EVENT_PROXY.write().unwrap().replace(proxy);
    let _call_on_ret = crate::common::SimpleCallOnReturn {
        b: true,
        f: Box::new(move || {
            let _ = EVENT_PROXY.write().unwrap().take();
        }),
    };

    // to-do: The scale factor may not be correct.
    // There may be multiple monitors with different scale factors.
    // But we only have one window, and one scale factor.
    let mut scale_factor = window.scale_factor();
    if scale_factor == 0.0 {
        scale_factor = 1.0;
    }
    let physical_size = window.inner_size();
    let logical_size = physical_size.to_logical::<f64>(scale_factor);

    struct Ripple {
        x: f64,
        y: f64,
        start_time: Instant,
    }
    let mut ripples: Vec<Ripple> = Vec::new();
    let mut last_cursors: HashMap<String, Cursor> = HashMap::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::NewEvents(StartCause::Init) => {
                window.set_outer_position(PhysicalPosition::new(0, 0));
                window.request_redraw();
                crate::platform::hide_dock();
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                if let Ok(handle) = window.window_handle() {
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
                                        logical_size.height,
                                        None,
                                    );
                                    context.clear(None, piet::Color::TRANSPARENT);

                                    let ripple_duration = std::time::Duration::from_millis(500);
                                    ripples.retain_mut(|ripple| {
                                        let elapsed = ripple.start_time.elapsed();
                                        let progress =
                                            elapsed.as_secs_f64() / ripple_duration.as_secs_f64();
                                        let radius = 45.0 * progress / scale_factor;
                                        let alpha = 1.0 - progress;
                                        if alpha > 0.0 {
                                            let color = piet::Color::rgba(1.0, 0.5, 0.5, alpha);
                                            let circle = piet::kurbo::Circle::new(
                                                (ripple.x / scale_factor, ripple.y / scale_factor),
                                                radius,
                                            );
                                            context.stroke(circle, &color, 2.0);
                                            true
                                        } else {
                                            false
                                        }
                                    });

                                    for cursor in last_cursors.values() {
                                        let (x, y) = (
                                            cursor.x as f64 / scale_factor,
                                            cursor.y as f64 / scale_factor,
                                        );
                                        let size = 1.0;

                                        let mut pb = BezPath::new();
                                        pb.move_to((x, y));
                                        pb.line_to((x, y + 16.0 * size));
                                        pb.line_to((x + 4.0 * size, y + 13.0 * size));
                                        pb.line_to((x + 7.0 * size, y + 20.0 * size));
                                        pb.line_to((x + 9.0 * size, y + 19.0 * size));
                                        pb.line_to((x + 6.0 * size, y + 12.0 * size));
                                        pb.line_to((x + 11.0 * size, y + 12.0 * size));

                                        let color = piet::Color::rgba8(
                                            (cursor.argb >> 16 & 0xFF) as u8,
                                            (cursor.argb >> 8 & 0xFF) as u8,
                                            (cursor.argb & 0xFF) as u8,
                                            (cursor.argb >> 24 & 0xFF) as u8,
                                        );
                                        context.fill(pb, &color);
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
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::UserEvent((k, evt)) => match evt {
                CustomEvent::Cursor(cursor) => {
                    if cursor.btns != 0 {
                        ripples.push(Ripple {
                            x: cursor.x as _,
                            y: cursor.y as _,
                            start_time: Instant::now(),
                        });
                    }
                    last_cursors.insert(k, cursor);
                    window.request_redraw();
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
