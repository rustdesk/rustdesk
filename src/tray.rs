#[cfg(any(target_os = "linux", target_os = "windows"))]
use super::ui_interface::get_option_opt;
#[cfg(target_os = "linux")]
use hbb_common::log::{debug, error, info};
#[cfg(target_os = "linux")]
use libappindicator::AppIndicator;
#[cfg(target_os = "linux")]
use std::env::temp_dir;
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

/// Start a tray icon in Linux
///
/// [Block]
/// This function will block current execution, show the tray icon and handle events.
#[cfg(target_os = "linux")]
pub fn start_tray() {
    use std::time::Duration;

    use glib::{clone, Continue};
    use gtk::traits::{GtkMenuItemExt, MenuShellExt, WidgetExt};

    info!("configuring tray");
    // init gtk context
    if let Err(err) = gtk::init() {
        error!("Error when starting the tray: {}", err);
        return;
    }
    if let Some(mut appindicator) = get_default_app_indicator() {
        let mut menu = gtk::Menu::new();
        let stoped = is_service_stopped();
        // start/stop service
        let label = if stoped {
            crate::client::translate("Start Service".to_owned())
        } else {
            crate::client::translate("Stop service".to_owned())
        };
        let menu_item_service = gtk::MenuItem::with_label(label.as_str());
        menu_item_service.connect_activate(move |_| {
            let _lock = crate::ui_interface::SENDER.lock().unwrap();
            change_service_state();
        });
        menu.append(&menu_item_service);
        // show tray item
        menu.show_all();
        appindicator.set_menu(&mut menu);
        // start event loop
        info!("Setting tray event loop");
        // check the connection status for every second
        glib::timeout_add_local(
            Duration::from_secs(1),
            clone!(@strong menu_item_service as item => move || {
                let _lock = crate::ui_interface::SENDER.lock().unwrap();
                update_tray_service_item(&item);
                // continue to trigger the next status check
                Continue(true)
            }),
        );
        gtk::main();
    } else {
        error!("Tray process exit now");
    }
}

#[cfg(target_os = "linux")]
fn change_service_state() {
    if is_service_stopped() {
        debug!("Now try to start service");
        crate::ipc::set_option("stop-service", "");
    } else {
        debug!("Now try to stop service");
        crate::ipc::set_option("stop-service", "Y");
    }
}

#[cfg(target_os = "linux")]
#[inline]
fn update_tray_service_item(item: &gtk::MenuItem) {
    use gtk::traits::GtkMenuItemExt;

    if is_service_stopped() {
        item.set_label(&crate::client::translate("Start Service".to_owned()));
    } else {
        item.set_label(&crate::client::translate("Stop service".to_owned()));
    }
}

#[cfg(target_os = "linux")]
fn get_default_app_indicator() -> Option<AppIndicator> {
    use libappindicator::AppIndicatorStatus;
    use std::io::Write;

    let icon = include_bytes!("../res/icon.png");
    // appindicator does not support icon buffer, so we write it to tmp folder
    let mut icon_path = temp_dir();
    icon_path.push("RustDesk");
    icon_path.push("rustdesk.png");
    match std::fs::File::create(icon_path.clone()) {
        Ok(mut f) => {
            f.write_all(icon).unwrap();
            // set .png icon file to be writable
            // this ensures successful file rewrite when switching between x11 and wayland.
            let mut perm = f.metadata().unwrap().permissions();
            if perm.readonly() {
                perm.set_readonly(false);
                f.set_permissions(perm).unwrap();
            }
        }
        Err(err) => {
            error!("Error when writing icon to {:?}: {}", icon_path, err);
            return None;
        }
    }
    debug!("write temp icon complete");
    let mut appindicator = AppIndicator::new("RustDesk", icon_path.to_str().unwrap_or("rustdesk"));
    appindicator.set_label("RustDesk", "A remote control software.");
    appindicator.set_status(AppIndicatorStatus::Active);
    Some(appindicator)
}

/// Check if service is stoped.
/// Return [`true`] if service is stoped, [`false`] otherwise.
#[inline]
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn is_service_stopped() -> bool {
    if let Some(v) = get_option_opt("stop-service") {
        v == "Y"
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
pub fn make_tray() {
    use tray_item::TrayItem;
    let mode = dark_light::detect();
    let icon_path;
    match mode {
        dark_light::Mode::Dark => {
            icon_path = "mac-tray-light.png";
        }
        dark_light::Mode::Light => {
            icon_path = "mac-tray-dark.png";
        }
    }
    if let Ok(mut tray) = TrayItem::new(&crate::get_app_name(), icon_path) {
        tray.add_label(&format!(
            "{} {}",
            crate::get_app_name(),
            crate::lang::translate("Service is running".to_owned())
        ))
        .ok();

        let inner = tray.inner_mut();
        inner.add_quit_item(&crate::lang::translate("Quit".to_owned()));
        inner.display();
    } else {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    }
}
