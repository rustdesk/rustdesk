#[cfg(any(target_os = "windows"))]
use super::ui_interface::get_option_opt;
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "windows")]
use trayicon::{MenuBuilder, TrayIconBuilder};
#[cfg(target_os = "windows")]
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
};

#[cfg(target_os = "windows")]
#[derive(Clone, Eq, PartialEq, Debug)]
enum Events {
    DoubleClickTrayIcon,
    StopService,
    StartService,
}

#[cfg(target_os = "windows")]
pub fn start_tray() {
    let event_loop = EventLoop::<Events>::with_user_event();
    let proxy = event_loop.create_proxy();
    let icon = include_bytes!("../res/tray-icon.ico");
    let mut tray_icon = TrayIconBuilder::new()
        .sender_winit(proxy)
        .icon_from_buffer(icon)
        .tooltip("RustDesk")
        .on_double_click(Events::DoubleClickTrayIcon)
        .build()
        .unwrap();
    let old_state = Arc::new(Mutex::new(0));
    let _sender = crate::ui_interface::SENDER.lock().unwrap();
    event_loop.run(move |event, _, control_flow| {
        if get_option_opt("ipc-closed").is_some() {
            *control_flow = ControlFlow::Exit;
            return;
        } else {
            *control_flow = ControlFlow::Wait;
        }
        let stopped = is_service_stopped();
        let state = if stopped { 2 } else { 1 };
        let old = *old_state.lock().unwrap();
        if state != old {
            hbb_common::log::info!("State changed");
            let mut m = MenuBuilder::new();
            if state == 2 {
                m = m.item(
                    &crate::client::translate("Start Service".to_owned()),
                    Events::StartService,
                );
            } else {
                m = m.item(
                    &crate::client::translate("Stop service".to_owned()),
                    Events::StopService,
                );
            }
            tray_icon.set_menu(&m).ok();
            *old_state.lock().unwrap() = state;
        }

        match event {
            Event::UserEvent(e) => match e {
                Events::DoubleClickTrayIcon => {
                    crate::run_me(Vec::<&str>::new()).ok();
                }
                Events::StopService => {
                    crate::ipc::set_option("stop-service", "Y");
                }
                Events::StartService => {
                    crate::ipc::set_option("stop-service", "");
                }
            },
            _ => (),
        }
    });
}

/// Check if service is stoped.
/// Return [`true`] if service is stoped, [`false`] otherwise.
#[inline]
#[cfg(any(target_os = "windows"))]
fn is_service_stopped() -> bool {
    if let Some(v) = get_option_opt("stop-service") {
        v == "Y"
    } else {
        false
    }
}

/// Start a tray icon in Linux
///
/// [Block]
/// This function will block current execution, show the tray icon and handle events.
#[cfg(target_os = "linux")]
pub fn start_tray() {}

#[cfg(target_os = "macos")]
pub fn start_tray() {
    use hbb_common::{allow_err, log};
    allow_err!(make_tray());
}

#[cfg(target_os = "macos")]
pub fn make_tray() -> hbb_common::ResultType<()> {
    // https://github.com/tauri-apps/tray-icon/blob/dev/examples/tao.rs
    use hbb_common::anyhow::Context;
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::{TrayEvent, TrayIconBuilder};
    let mode = dark_light::detect();
    const LIGHT: &[u8] = include_bytes!("../res/mac-tray-light-x2.png");
    const DARK: &[u8] = include_bytes!("../res/mac-tray-dark-x2.png");
    let icon = match mode {
        dark_light::Mode::Dark => DARK,
        _ => LIGHT,
    };
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(icon)
            .context("Failed to open icon path")?
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    let icon = tray_icon::icon::Icon::from_rgba(icon_rgba, icon_width, icon_height)
        .context("Failed to open icon")?;

    let event_loop = EventLoopBuilder::new().build();

    let _tray_icon = Some(
        TrayIconBuilder::new()
            .with_tooltip(format!(
                "{} {}",
                crate::get_app_name(),
                crate::lang::translate("Service is running".to_owned())
            ))
            .with_icon(icon)
            .build()?,
    );

    let tray_channel = TrayEvent::receiver();
    let mut docker_hiden = false;

    event_loop.run(move |_event, _, control_flow| {
        if !docker_hiden {
            crate::platform::macos::hide_dock();
            docker_hiden = true;
        }
        *control_flow = ControlFlow::Poll;

        if tray_channel.try_recv().is_ok() {
            crate::platform::macos::handle_application_should_open_untitled_file();
        }
    });
}
