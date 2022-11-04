use hbb_common::log::{debug, error, info};
use lazy_static::lazy_static;
#[cfg(target_os = "linux")]
use libappindicator::AppIndicator;
use std::env::temp_dir;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

#[derive(Clone, Eq, PartialEq, Debug)]
enum Events {
    DoubleClickTrayIcon,
    StopService,
    StartService,
}

#[cfg(target_os = "windows")]
pub fn start_tray(options: Arc<Mutex<HashMap<String, String>>>) {
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
    let _ = crate::ui_interface::SENDER.lock().unwrap();
    event_loop.run(move |event, _, control_flow| {
        if options.lock().unwrap().get("ipc-closed").is_some() {
            *control_flow = ControlFlow::Exit;
            return;
        } else {
            *control_flow = ControlFlow::Wait;
        }
        let stopped = if let Some(v) = options.lock().unwrap().get("stop-service") {
            !v.is_empty()
        } else {
            false
        };
        let stopped = if stopped { 2 } else { 1 };
        let old = *old_state.lock().unwrap();
        if stopped != old {
            hbb_common::log::info!("State changed");
            let mut m = MenuBuilder::new();
            if stopped == 2 {
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
            *old_state.lock().unwrap() = stopped;
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
pub fn start_tray(options: Arc<Mutex<HashMap<String, String>>>) {
    use std::time::Duration;

    use gtk::traits::{GtkMenuItemExt, MenuShellExt, WidgetExt};
    info!("configuring tray");
    // init gtk context
    if let Err(err) = gtk::init() {
        error!("Error when starting the tray: {}", err);
        gtk::main_quit();
        return;
    }
    if let Some(mut appindicator) = get_default_app_indicator() {
        let mut menu = gtk::Menu::new();
        let running = get_service_status(options.clone());
        // start/stop service
        let label = if !running {
            crate::client::translate("Start Service".to_owned())
        } else {
            crate::client::translate("Stop service".to_owned())
        };
        let menu_item_service = gtk::MenuItem::with_label(label.as_str());
        menu_item_service.connect_activate(move |item| {
            let lock = crate::ui_interface::SENDER.lock().unwrap();
            update_tray_service_item(options.clone(), item);
        });
        menu.append(&menu_item_service);
        // show tray item
        menu.show_all();
        appindicator.set_menu(&mut menu);
        // start event loop
        info!("Setting tray event loop");
        gtk::main();
    } else {
        eprintln!("tray process exit now");
    }
}

#[cfg(target_os = "linux")]
fn update_tray_service_item(options: Arc<Mutex<HashMap<String, String>>>, item: &gtk::MenuItem) {
    use gtk::{
        traits::{GtkMenuItemExt, ListBoxRowExt},
        MenuItem,
    };
    if get_service_status(options.clone()) {
        debug!("Now try to stop service");
        item.set_label(&crate::client::translate("Start Service".to_owned()));
        crate::ipc::set_option("stop-service", "Y");
    } else {
        debug!("Now try to start service");
        item.set_label(&crate::client::translate("Stop service".to_owned()));
        crate::ipc::set_option("stop-service", "");
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

/// Get service status
/// Return [`true`] if service is running, [`false`] otherwise.
#[inline]
fn get_service_status(options: Arc<Mutex<HashMap<String, String>>>) -> bool {
    if let Some(v) = options.lock().unwrap().get("stop-service") {
        debug!("service stopped: {}", v);
        v.is_empty()
    } else {
        true
    }
}
