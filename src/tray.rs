pub fn start_tray() {
    use hbb_common::{allow_err, log};
    allow_err!(make_tray());
}

pub fn make_tray() -> hbb_common::ResultType<()> {
    // https://github.com/tauri-apps/tray-icon/blob/dev/examples/tao.rs
    use hbb_common::anyhow::Context;
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tray_icon::{
        menu::{Menu, MenuEvent, MenuItem},
        TrayEvent, TrayIconBuilder,
    };
    let icon;
    #[cfg(target_os = "macos")]
    {
        let mode = dark_light::detect();
        const LIGHT: &[u8] = include_bytes!("../res/mac-tray-light-x2.png");
        const DARK: &[u8] = include_bytes!("../res/mac-tray-dark-x2.png");
        icon = match mode {
            dark_light::Mode::Dark => LIGHT,
            _ => DARK,
        };
    }
    #[cfg(not(target_os = "macos"))]
    {
        icon = include_bytes!("../res/tray-icon.ico");
    }
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

    let tray_menu = Menu::new();
    let quit_i = MenuItem::new(crate::client::translate("Exit".to_owned()), true, None);
    let open_i = MenuItem::new(crate::client::translate("Open".to_owned()), true, None);
    tray_menu.append_items(&[&open_i, &quit_i]);

    let _tray_icon = Some(
        TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip(format!(
                "{} {}",
                crate::get_app_name(),
                crate::lang::translate("Service is running".to_owned())
            ))
            .with_icon(icon)
            .build()?,
    );

    let menu_channel = MenuEvent::receiver();
    let tray_channel = TrayEvent::receiver();
    let mut docker_hiden = false;

    let open_func = move || {
        #[cfg(not(feature = "flutter"))]
        {
        crate::run_me::<&str>(vec![]).ok();
        return;
        }
        #[cfg(target_os = "macos")]
        crate::platform::macos::handle_application_should_open_untitled_file();
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            use std::process::Command;
            Command::new("cmd")
                .arg("/c")
                .arg("start rustdesk://")
                .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
                .spawn()
                .ok();
        }
        #[cfg(target_os = "linux")]
        if !std::process::Command::new("xdg-open")
            .arg("rustdesk://")
            .spawn()
            .is_ok()
        {
            crate::run_me::<&str>(vec![]).ok();
        }
    };

    event_loop.run(move |_event, _, control_flow| {
        if !docker_hiden {
            #[cfg(target_os = "macos")]
            crate::platform::macos::hide_dock();
            docker_hiden = true;
        }
        *control_flow = ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(100),
        );

        if let Ok(event) = menu_channel.try_recv() {
            if event.id == quit_i.id() {
                if !crate::check_process("--server", false) {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                crate::platform::uninstall_service(false);
            } else if event.id == open_i.id() {
                open_func();
            }
        }

        if let Ok(_event) = tray_channel.try_recv() {
            #[cfg(target_os = "windows")]
            if _event.event == tray_icon::ClickEvent::Left {
                open_func();
            }
        }
    });
}
