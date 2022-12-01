mod tauri_session_handler;
use hbb_common::config;
use tauri::{Builder, Wry};

use crate::{ui_interface, ui_cm_interface};

#[tauri::command(async)]
fn exit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command(async)]
fn send_wol(id: String) {
    crate::lan::send_wol(id)
}

#[tauri::command(async)]
fn remove_discovered(id: String) {
    let mut peers = config::LanPeers::load().peers;
    peers.retain(|x| x.id != id);
    config::LanPeers::store(&peers);
}

#[tauri::command(async)]
fn get_os() -> String{
    #[cfg(target_os = "windows")]
    return "Windows".to_string();
    #[cfg(target_os = "linux")]
    return "Linux".to_string();
    #[cfg(target_os = "macos")]
    return "OSX".to_string();
}

#[tauri::command(async)]
fn centerize(window: tauri::Window, mut w: u32, mut h: u32) {    
    let current_monitor = window.current_monitor().unwrap().unwrap();
    let size = current_monitor.size();
    let position = current_monitor.position();
    
    let sw = size.width;
    let sh = size.height;
    let sx = position.x;
    let sy = position.y;
    if w > sw {
        w = sw;
    }
    if h > sh {
        h = sh;
    }
    let x = (sx + sw as i32 - w as i32) / 2;
    let y = (sy + sh as i32 - h as i32) / 2;
    window.set_position(tauri::Position::Physical(tauri::PhysicalPosition::new(x, y))).unwrap();
    window.set_size(tauri::Size::Physical(tauri::PhysicalSize { width: w, height: h })).unwrap();
}


#[tauri::command(async)]
fn remove_disconnected_connection(id: i32) {
    ui_cm_interface::remove(id);
}

#[tauri::command(async)]
fn quit() {
    crate::platform::quit_gui();
}

#[tauri::command(async)]
fn send_msg(id: i32, text: String) {
    ui_cm_interface::send_chat(id, text);
}

pub fn invoke_handler(builder: Builder<Wry>) -> Builder<Wry>{
    builder.invoke_handler(tauri::generate_handler![
        exit,
        // 
        // connectionManager,
        // native_remote,
        centerize,
        get_os,
        tauri_session_handler::test_tauri,
        // UI for index.html and install.html
        ui_interface::t, // translate text for react component
        ui_interface::get_api_server, // get api server address 
        ui_interface::is_xfce, // get xfce UNIX settings
        ui_interface::using_public_server, // is server public
        ui_interface::temporary_password, // get temporary password
        ui_interface::update_temporary_password, // update temporary password
        ui_interface::permanent_password, // get permanent password
        ui_interface::set_permanent_password, // set permanent password
        ui_interface::get_remote_id,
        ui_interface::set_remote_id,
        ui_interface::closing, // set config size
        ui_interface::get_size, // get config size
        ui_interface::new_remote, // create new remote connection
        // ui_interface::new_remote_tauri,
        send_wol, // wake on lan signal
        ui_interface::remove_peer, // remove id from PeerConfig
        remove_discovered, // remove id from discovered peers
        ui_interface::get_connect_status,
        ui_interface::get_mouse_time,
        ui_interface::check_mouse_time,
        ui_interface::get_recent_sessions,
        ui_interface::get_peer,
        ui_interface::get_fav, // get favorite ?
        ui_interface::store_fav, // store favorite ?
        ui_interface::get_icon,
        ui_interface::install_me,
        ui_interface::is_installed,
        ui_interface::is_root, // does user have root access to OS
        ui_interface::is_release, // is it release version
        ui_interface::set_socks,
        ui_interface::get_socks,
        ui_interface::is_rdp_service_open, // is Remote Desktop Protocol service open
        ui_interface::is_share_rdp, // is Remote Desktop Protocol shared in registr
        ui_interface::set_share_rdp, // is Remote Desktop Protocol shared in registr
        ui_interface::is_installed_lower_version,
        ui_interface::install_path,
        ui_interface::goto_install, // run -install script
        ui_interface::is_process_trusted, // Macos: is process trusted
        ui_interface::is_can_screen_recording, // Macos: permission to screen recording
        ui_interface::is_installed_daemon, // Macos: is daemon installed
        ui_interface::get_error, 
        ui_interface::is_login_wayland,
        ui_interface::fix_login_wayland,
        ui_interface::current_is_wayland,
        ui_interface::modify_default_login, // Linux: modify default login
        ui_interface::get_options,
        ui_interface::get_option,
        ui_interface::get_local_option,
        ui_interface::set_local_option,
        ui_interface::get_peer_option,
        ui_interface::forget_password,
        ui_interface::set_peer_option,
        ui_interface::has_rendezvous_service,
        ui_interface::get_license,
        ui_interface::test_if_valid_server,
        ui_interface::set_options, //"custom-rendezvous-server" "relay-server" "api-server" "key"
        ui_interface::set_option,
        ui_interface::get_software_update_url,
        ui_interface::get_new_version,
        ui_interface::get_version,
        ui_interface::update_me,
        ui_interface::show_run_without_install,
        ui_interface::run_without_install,
        ui_interface::get_software_store_path,
        ui_interface::get_software_ext,
        ui_interface::get_async_job_status,
        ui_interface::post_request,
        ui_interface::is_ok_change_id,
        ui_interface::create_shortcut,
        ui_interface::get_lan_peers,
        ui_interface::get_uuid,
        ui_interface::has_hwcodec,
        ui_interface::get_langs,
        ui_interface::default_video_save_directory,
        tauri_session_handler::get_audit_server,
        tauri_session_handler::send_note,
        tauri_session_handler::get_session_id,
        tauri_session_handler::get_default_pi,
        tauri_session_handler::get_session_option,
        tauri_session_handler::set_session_option,
        tauri_session_handler::input_os_password,
        tauri_session_handler::save_close_state,
        tauri_session_handler::is_file_transfer,
        tauri_session_handler::is_port_forward,
        tauri_session_handler::is_rdp,
        tauri_session_handler::login,
        tauri_session_handler::new_rdp,
        tauri_session_handler::send_mouse,
        tauri_session_handler::enter,
        tauri_session_handler::leave,
        tauri_session_handler::ctrl_alt_del,
        tauri_session_handler::transfer_file,
        tauri_session_handler::tunnel,
        tauri_session_handler::lock_screen,
        tauri_session_handler::reconnect,
        tauri_session_handler::get_chatbox,
        tauri_session_handler::get_home_dir,
        // client::read_dir,
        // client::remove_dir,
        // client::create_dir,
        // client::remove_file,
        // client::read_remote_dir,
        tauri_session_handler::send_chat,
        tauri_session_handler::switch_display,
        // tauri_session_handler::remove_dir_all,
        // tauri_session_handler::confirm_delete_files,
        // tauri_session_handler::set_no_confirm,
        // client::cancel_job,
        // client::send_files,
        // client::add_job,
        // client::resume_job,
        tauri_session_handler::get_platform,
        tauri_session_handler::get_path_sep,
        tauri_session_handler::get_icon_path,
        tauri_session_handler::get_char,
        tauri_session_handler::get_session_size,
        tauri_session_handler::get_port_forwards,
        tauri_session_handler::remove_port_forward,
        tauri_session_handler::get_args,
        tauri_session_handler::add_port_forward,
        tauri_session_handler::save_size,
        tauri_session_handler::get_view_style,
        tauri_session_handler::get_image_quality,
        tauri_session_handler::get_custom_image_quality,
        tauri_session_handler::save_view_style,
        tauri_session_handler::save_image_quality,
        tauri_session_handler::save_custom_image_quality,
        tauri_session_handler::refresh_video,
        tauri_session_handler::record_screen,
        tauri_session_handler::get_toggle_option,
        tauri_session_handler::is_privacy_mode_supported,
        tauri_session_handler::toggle_option,
        tauri_session_handler::get_remember,
        tauri_session_handler::peer_platform,
        tauri_session_handler::set_write_override,
        tauri_session_handler::get_keyboard_mode,
        tauri_session_handler::save_keyboard_mode,
        tauri_session_handler::supported_hwcodec,
        tauri_session_handler::change_prefer_codec,
        tauri_session_handler::restart_remote_device,
        ui_cm_interface::check_click_time,
        ui_cm_interface::get_click_time,
        ui_cm_interface::close,
        remove_disconnected_connection,
        quit,
        ui_cm_interface::authorize,
        ui_cm_interface::switch_permission,
        send_msg,
        ])
}
