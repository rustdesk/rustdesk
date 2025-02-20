#[cfg(not(any(target_os = "android", target_os = "ios")))]
use crate::keyboard::input_source::{change_input_source, get_cur_session_input_source};
use crate::{
    client::file_trait::FileManager,
    common::{make_fd_to_json, make_vec_fd_to_json},
    flutter::{
        self, session_add, session_add_existed, session_start_, sessions, try_sync_peer_option,
    },
    input::*,
    ui_interface::{self, *},
};
use flutter_rust_bridge::{StreamSink, SyncReturn};
#[cfg(feature = "plugin_framework")]
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use hbb_common::allow_err;
use hbb_common::{
    config::{self, LocalConfig, PeerConfig, PeerInfoSerde},
    fs, lazy_static, log,
    rendezvous_proto::ConnType,
    ResultType,
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    },
    time::SystemTime,
};

pub type SessionID = uuid::Uuid;

lazy_static::lazy_static! {
    static ref TEXTURE_RENDER_KEY: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
}

fn initialize(app_dir: &str, custom_client_config: &str) {
    flutter::async_tasks::start_flutter_async_runner();
    *config::APP_DIR.write().unwrap() = app_dir.to_owned();
    // core_main's load_custom_client does not work for flutter since it is only applied to its load_library in main.c
    if custom_client_config.is_empty() {
        crate::load_custom_client();
    } else {
        crate::read_custom_client(custom_client_config);
    }
    #[cfg(target_os = "android")]
    {
        // flexi_logger can't work when android_logger initialized.
        #[cfg(debug_assertions)]
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug) // limit log level
                .with_tag("ffi"), // logs will show under mytag tag
        );
        #[cfg(not(debug_assertions))]
        hbb_common::init_log(false, "");
        #[cfg(feature = "mediacodec")]
        scrap::mediacodec::check_mediacodec();
        crate::common::test_rendezvous_server();
        crate::common::test_nat_type();
    }
    #[cfg(target_os = "ios")]
    {
        use hbb_common::env_logger::*;
        init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // core_main's init_log does not work for flutter since it is only applied to its load_library in main.c
        hbb_common::init_log(false, "flutter_ffi");
    }
}

#[inline]
pub fn start_global_event_stream(s: StreamSink<String>, app_type: String) -> ResultType<()> {
    super::flutter::start_global_event_stream(s, app_type)
}

#[inline]
pub fn stop_global_event_stream(app_type: String) {
    super::flutter::stop_global_event_stream(app_type)
}
pub enum EventToUI {
    Event(String),
    Rgba(usize),
    Texture(usize, bool), // (display, gpu_texture)
}

pub fn host_stop_system_key_propagate(_stopped: bool) {
    #[cfg(windows)]
    crate::platform::windows::stop_system_key_propagate(_stopped);
}

// This function is only used to count the number of control sessions.
pub fn peer_get_default_sessions_count(id: String) -> SyncReturn<usize> {
    SyncReturn(sessions::get_session_count(id, ConnType::DEFAULT_CONN))
}

pub fn session_add_existed_sync(
    id: String,
    session_id: SessionID,
    displays: Vec<i32>,
) -> SyncReturn<String> {
    if let Err(e) = session_add_existed(id.clone(), session_id, displays) {
        SyncReturn(format!("Failed to add session with id {}, {}", &id, e))
    } else {
        SyncReturn("".to_owned())
    }
}

pub fn session_add_sync(
    session_id: SessionID,
    id: String,
    is_file_transfer: bool,
    is_port_forward: bool,
    is_rdp: bool,
    switch_uuid: String,
    force_relay: bool,
    password: String,
    is_shared_password: bool,
    conn_token: Option<String>,
) -> SyncReturn<String> {
    if let Err(e) = session_add(
        &session_id,
        &id,
        is_file_transfer,
        is_port_forward,
        is_rdp,
        &switch_uuid,
        force_relay,
        password,
        is_shared_password,
        conn_token,
    ) {
        SyncReturn(format!("Failed to add session with id {}, {}", &id, e))
    } else {
        SyncReturn("".to_owned())
    }
}

pub fn session_start(
    events2ui: StreamSink<EventToUI>,
    session_id: SessionID,
    id: String,
) -> ResultType<()> {
    session_start_(&session_id, &id, events2ui)
}

pub fn session_start_with_displays(
    events2ui: StreamSink<EventToUI>,
    session_id: SessionID,
    id: String,
    displays: Vec<i32>,
) -> ResultType<()> {
    session_start_(&session_id, &id, events2ui)?;

    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.capture_displays(displays.clone(), vec![], vec![]);
        for display in displays {
            session.refresh_video(display as _);
        }
    }
    Ok(())
}

pub fn session_get_remember(session_id: SessionID) -> Option<bool> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_remember())
    } else {
        None
    }
}

pub fn session_get_toggle_option(session_id: SessionID, arg: String) -> Option<bool> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_toggle_option(arg))
    } else {
        None
    }
}

pub fn session_get_toggle_option_sync(session_id: SessionID, arg: String) -> SyncReturn<bool> {
    let res = session_get_toggle_option(session_id, arg) == Some(true);
    SyncReturn(res)
}

pub fn session_get_option(session_id: SessionID, arg: String) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_option(arg))
    } else {
        None
    }
}

pub fn session_login(
    session_id: SessionID,
    os_username: String,
    os_password: String,
    password: String,
    remember: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.login(os_username, os_password, password, remember);
    }
}

pub fn session_send2fa(session_id: SessionID, code: String, trust_this_device: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.send2fa(code, trust_this_device);
    }
}

pub fn session_get_enable_trusted_devices(session_id: SessionID) -> SyncReturn<bool> {
    let v = if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.get_enable_trusted_devices()
    } else {
        false
    };
    SyncReturn(v)
}

pub fn session_close(session_id: SessionID) {
    if let Some(session) = sessions::remove_session_by_session_id(&session_id) {
        // `release_remote_keys` is not required for mobile platforms in common cases.
        // But we still call it to make the code more stable.
        #[cfg(any(target_os = "android", target_os = "ios"))]
        crate::keyboard::release_remote_keys("map");
        session.close_event_stream(session_id);
        session.close();
    }
}

pub fn session_refresh(session_id: SessionID, display: usize) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.refresh_video(display as _);
    }
}

pub fn session_is_multi_ui_session(session_id: SessionID) -> SyncReturn<bool> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(session.is_multi_ui_session())
    } else {
        SyncReturn(false)
    }
}

pub fn session_record_screen(session_id: SessionID, start: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.record_screen(start);
    }
}

pub fn session_get_is_recording(session_id: SessionID) -> SyncReturn<bool> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(session.is_recording())
    } else {
        SyncReturn(false)
    }
}

pub fn session_reconnect(session_id: SessionID, force_relay: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.reconnect(force_relay);
    }
    session_on_waiting_for_image_dialog_show(session_id);
}

pub fn session_toggle_option(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        log::warn!("toggle option {}", &value);
        session.toggle_option(value.clone());
        try_sync_peer_option(&session, &session_id, &value, None);
    }
    #[cfg(not(target_os = "ios"))]
    if sessions::get_session_by_session_id(&session_id).is_some() && value == "disable-clipboard" {
        crate::flutter::update_text_clipboard_required();
    }
    #[cfg(feature = "unix-file-copy-paste")]
    if sessions::get_session_by_session_id(&session_id).is_some()
        && value == config::keys::OPTION_ENABLE_FILE_COPY_PASTE
    {
        crate::flutter::update_file_clipboard_required();
    }
}

pub fn session_toggle_privacy_mode(session_id: SessionID, impl_key: String, on: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.toggle_privacy_mode(impl_key, on);
    }
}

pub fn session_get_flutter_option(session_id: SessionID, k: String) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_flutter_option(k))
    } else {
        None
    }
}

pub fn session_set_flutter_option(session_id: SessionID, k: String, v: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_flutter_option(k, v);
    }
}

pub fn get_next_texture_key() -> SyncReturn<i32> {
    let k = TEXTURE_RENDER_KEY.fetch_add(1, Ordering::SeqCst) + 1;
    SyncReturn(k)
}

pub fn get_local_flutter_option(k: String) -> SyncReturn<String> {
    SyncReturn(ui_interface::get_local_flutter_option(k))
}

pub fn set_local_flutter_option(k: String, v: String) {
    ui_interface::set_local_flutter_option(k, v);
}

pub fn get_local_kb_layout_type() -> SyncReturn<String> {
    SyncReturn(ui_interface::get_kb_layout_type())
}

pub fn set_local_kb_layout_type(kb_layout_type: String) {
    ui_interface::set_kb_layout_type(kb_layout_type)
}

pub fn session_get_view_style(session_id: SessionID) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_view_style())
    } else {
        None
    }
}

pub fn session_set_view_style(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_view_style(value);
    }
}

pub fn session_get_scroll_style(session_id: SessionID) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_scroll_style())
    } else {
        None
    }
}

pub fn session_set_scroll_style(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_scroll_style(value);
    }
}

pub fn session_get_image_quality(session_id: SessionID) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_image_quality())
    } else {
        None
    }
}

pub fn session_set_image_quality(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_image_quality(value);
    }
}

pub fn session_get_keyboard_mode(session_id: SessionID) -> Option<String> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_keyboard_mode())
    } else {
        None
    }
}

pub fn session_set_keyboard_mode(session_id: SessionID, value: String) {
    let mut _mode_updated = false;
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_keyboard_mode(value.clone());
        _mode_updated = true;
        try_sync_peer_option(&session, &session_id, "keyboard_mode", None);
    }
    #[cfg(windows)]
    if _mode_updated {
        crate::keyboard::update_grab_get_key_name(&value);
    }
}

pub fn session_get_reverse_mouse_wheel_sync(session_id: SessionID) -> SyncReturn<Option<String>> {
    let res = if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_reverse_mouse_wheel())
    } else {
        None
    };
    SyncReturn(res)
}

pub fn session_set_reverse_mouse_wheel(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_reverse_mouse_wheel(value);
    }
}

pub fn session_get_displays_as_individual_windows(
    session_id: SessionID,
) -> SyncReturn<Option<String>> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(Some(session.get_displays_as_individual_windows()))
    } else {
        SyncReturn(None)
    }
}

pub fn session_set_displays_as_individual_windows(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_displays_as_individual_windows(value);
    }
}

pub fn session_get_use_all_my_displays_for_the_remote_session(
    session_id: SessionID,
) -> SyncReturn<Option<String>> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(Some(
            session.get_use_all_my_displays_for_the_remote_session(),
        ))
    } else {
        SyncReturn(None)
    }
}

pub fn session_set_use_all_my_displays_for_the_remote_session(
    session_id: SessionID,
    value: String,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_use_all_my_displays_for_the_remote_session(value);
    }
}

pub fn session_get_custom_image_quality(session_id: SessionID) -> Option<Vec<i32>> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        Some(session.get_custom_image_quality())
    } else {
        None
    }
}

pub fn session_is_keyboard_mode_supported(session_id: SessionID, mode: String) -> SyncReturn<bool> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(session.is_keyboard_mode_supported(mode))
    } else {
        SyncReturn(false)
    }
}

pub fn session_set_custom_image_quality(session_id: SessionID, value: i32) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.save_custom_image_quality(value);
    }
}

pub fn session_set_custom_fps(session_id: SessionID, fps: i32) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.set_custom_fps(fps);
    }
}

pub fn session_lock_screen(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.lock_screen();
    }
}

pub fn session_ctrl_alt_del(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.ctrl_alt_del();
    }
}

pub fn session_switch_display(is_desktop: bool, session_id: SessionID, value: Vec<i32>) {
    sessions::session_switch_display(is_desktop, session_id, value);
}

pub fn session_handle_flutter_key_event(
    session_id: SessionID,
    character: String,
    usb_hid: i32,
    lock_modes: i32,
    down_or_up: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        let keyboard_mode = session.get_keyboard_mode();
        session.handle_flutter_key_event(
            &keyboard_mode,
            &character,
            usb_hid,
            lock_modes,
            down_or_up,
        );
    }
}

pub fn session_handle_flutter_raw_key_event(
    session_id: SessionID,
    name: String,
    platform_code: i32,
    position_code: i32,
    lock_modes: i32,
    down_or_up: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        let keyboard_mode = session.get_keyboard_mode();
        session.handle_flutter_raw_key_event(
            &keyboard_mode,
            &name,
            platform_code,
            position_code,
            lock_modes,
            down_or_up,
        );
    }
}

// SyncReturn<()> is used to make sure enter() and leave() are executed in the sequence this function is called.
//
// If the cursor jumps between remote page of two connections, leave view and enter view will be called.
// session_enter_or_leave() will be called then.
// As rust is multi-thread, it is possible that enter() is called before leave().
// This will cause the keyboard input to take no effect.
pub fn session_enter_or_leave(_session_id: SessionID, _enter: bool) -> SyncReturn<()> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if let Some(session) = sessions::get_session_by_session_id(&_session_id) {
        let keyboard_mode = session.get_keyboard_mode();
        if _enter {
            set_cur_session_id_(_session_id, &keyboard_mode);
            session.enter(keyboard_mode);
        } else {
            session.leave(keyboard_mode);
        }
    }
    SyncReturn(())
}

pub fn session_input_key(
    session_id: SessionID,
    name: String,
    down: bool,
    press: bool,
    alt: bool,
    ctrl: bool,
    shift: bool,
    command: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        // #[cfg(any(target_os = "android", target_os = "ios"))]
        session.input_key(&name, down, press, alt, ctrl, shift, command);
    }
}

pub fn session_input_string(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        // #[cfg(any(target_os = "android", target_os = "ios"))]
        session.input_string(&value);
    }
}

// chat_client_mode
pub fn session_send_chat(session_id: SessionID, text: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.send_chat(text);
    }
}

pub fn session_peer_option(session_id: SessionID, name: String, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.set_option(name, value);
    }
}

pub fn session_get_peer_option(session_id: SessionID, name: String) -> String {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        return session.get_option(name);
    }
    "".to_string()
}

pub fn session_input_os_password(session_id: SessionID, value: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.input_os_password(value, true);
    }
}

// File Action
pub fn session_read_remote_dir(session_id: SessionID, path: String, include_hidden: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.read_remote_dir(path, include_hidden);
    }
}

pub fn session_send_files(
    session_id: SessionID,
    act_id: i32,
    path: String,
    to: String,
    file_num: i32,
    include_hidden: bool,
    is_remote: bool,
    _is_dir: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.send_files(act_id, path, to, file_num, include_hidden, is_remote);
    }
}

pub fn session_set_confirm_override_file(
    session_id: SessionID,
    act_id: i32,
    file_num: i32,
    need_override: bool,
    remember: bool,
    is_upload: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.set_confirm_override_file(act_id, file_num, need_override, remember, is_upload);
    }
}

pub fn session_remove_file(
    session_id: SessionID,
    act_id: i32,
    path: String,
    file_num: i32,
    is_remote: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.remove_file(act_id, path, file_num, is_remote);
    }
}

pub fn session_read_dir_to_remove_recursive(
    session_id: SessionID,
    act_id: i32,
    path: String,
    is_remote: bool,
    show_hidden: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.remove_dir_all(act_id, path, is_remote, show_hidden);
    }
}

pub fn session_remove_all_empty_dirs(
    session_id: SessionID,
    act_id: i32,
    path: String,
    is_remote: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.remove_dir(act_id, path, is_remote);
    }
}

pub fn session_cancel_job(session_id: SessionID, act_id: i32) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.cancel_job(act_id);
    }
}

pub fn session_create_dir(session_id: SessionID, act_id: i32, path: String, is_remote: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.create_dir(act_id, path, is_remote);
    }
}

pub fn session_read_local_dir_sync(
    _session_id: SessionID,
    path: String,
    show_hidden: bool,
) -> String {
    if let Ok(fd) = fs::read_dir(&fs::get_path(&path), show_hidden) {
        return make_fd_to_json(fd.id, path, &fd.entries);
    }
    "".to_string()
}

pub fn session_read_local_empty_dirs_recursive_sync(
    _session_id: SessionID,
    path: String,
    include_hidden: bool,
) -> String {
    if let Ok(fds) = fs::get_empty_dirs_recursive(&path, include_hidden) {
        return make_vec_fd_to_json(&fds);
    }
    "".to_string()
}

pub fn session_read_remote_empty_dirs_recursive_sync(
    session_id: SessionID,
    path: String,
    include_hidden: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.read_empty_dirs(path, include_hidden);
    }
}

pub fn session_get_platform(session_id: SessionID, is_remote: bool) -> String {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        return session.get_platform(is_remote);
    }
    "".to_string()
}

pub fn session_load_last_transfer_jobs(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        return session.load_last_jobs();
    } else {
        // a tip for flutter dev
        eprintln!(
            "cannot load last transfer job from non-existed session. Please ensure session \
        is connected before calling load last transfer jobs."
        );
    }
}

pub fn session_add_job(
    session_id: SessionID,
    act_id: i32,
    path: String,
    to: String,
    file_num: i32,
    include_hidden: bool,
    is_remote: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.add_job(act_id, path, to, file_num, include_hidden, is_remote);
    }
}

pub fn session_resume_job(session_id: SessionID, act_id: i32, is_remote: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.resume_job(act_id, is_remote);
    }
}

pub fn session_rename_file(
    session_id: SessionID,
    act_id: i32,
    path: String,
    new_name: String,
    is_remote: bool,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.rename_file(act_id, path, new_name, is_remote);
    }
}

pub fn session_elevate_direct(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.elevate_direct();
    }
}

pub fn session_elevate_with_logon(session_id: SessionID, username: String, password: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.elevate_with_logon(username, password);
    }
}

pub fn session_switch_sides(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.switch_sides();
    }
}

pub fn session_change_resolution(session_id: SessionID, display: i32, width: i32, height: i32) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.change_resolution(display, width, height);
    }
}

pub fn session_set_size(session_id: SessionID, display: usize, width: usize, height: usize) {
    super::flutter::session_set_size(session_id, display, width, height)
}

pub fn session_send_selected_session_id(session_id: SessionID, sid: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.send_selected_session_id(sid);
    }
}

pub fn main_get_sound_inputs() -> Vec<String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    return get_sound_inputs();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    vec![String::from("")]
}

pub fn main_get_login_device_info() -> SyncReturn<String> {
    SyncReturn(get_login_device_info_json())
}

pub fn main_change_id(new_id: String) {
    change_id(new_id)
}

pub fn main_get_async_status() -> String {
    get_async_job_status()
}

pub fn main_get_http_status(url: String) -> Option<String> {
    get_async_http_status(url)
}

pub fn main_get_option(key: String) -> String {
    get_option(key)
}

pub fn main_get_option_sync(key: String) -> SyncReturn<String> {
    SyncReturn(get_option(key))
}

pub fn main_get_error() -> String {
    get_error()
}

pub fn main_show_option(_key: String) -> SyncReturn<bool> {
    #[cfg(target_os = "linux")]
    if _key.eq(config::keys::OPTION_ALLOW_LINUX_HEADLESS) {
        return SyncReturn(true);
    }
    SyncReturn(false)
}

pub fn main_set_option(key: String, value: String) {
    #[cfg(target_os = "android")]
    if key.eq(config::keys::OPTION_ENABLE_KEYBOARD) {
        crate::ui_cm_interface::switch_permission_all(
            "keyboard".to_owned(),
            config::option2bool(&key, &value),
        );
    }
    #[cfg(target_os = "android")]
    if key.eq(config::keys::OPTION_ENABLE_CLIPBOARD) {
        crate::ui_cm_interface::switch_permission_all(
            "clipboard".to_owned(),
            config::option2bool(&key, &value),
        );
    }

    if key.eq("custom-rendezvous-server") {
        set_option(key, value.clone());
        #[cfg(target_os = "android")]
        crate::rendezvous_mediator::RendezvousMediator::restart();
        #[cfg(any(target_os = "android", target_os = "ios", feature = "cli"))]
        crate::common::test_rendezvous_server();
    } else {
        set_option(key, value.clone());
    }
}

pub fn main_get_options() -> String {
    get_options()
}

pub fn main_get_options_sync() -> SyncReturn<String> {
    SyncReturn(get_options())
}

pub fn main_set_options(json: String) {
    let map: HashMap<String, String> = serde_json::from_str(&json).unwrap_or(HashMap::new());
    if !map.is_empty() {
        set_options(map)
    }
}

pub fn main_test_if_valid_server(server: String, test_with_proxy: bool) -> String {
    test_if_valid_server(server, test_with_proxy)
}

pub fn main_set_socks(proxy: String, username: String, password: String) {
    set_socks(proxy, username, password)
}

pub fn main_get_proxy_status() -> bool {
    get_proxy_status()
}

pub fn main_get_socks() -> Vec<String> {
    get_socks()
}

pub fn main_get_app_name() -> String {
    get_app_name()
}

pub fn main_get_app_name_sync() -> SyncReturn<String> {
    SyncReturn(get_app_name())
}

pub fn main_uri_prefix_sync() -> SyncReturn<String> {
    SyncReturn(crate::get_uri_prefix())
}

pub fn main_get_license() -> String {
    get_license()
}

pub fn main_get_version() -> String {
    get_version()
}

pub fn main_get_fav() -> Vec<String> {
    get_fav()
}

pub fn main_store_fav(favs: Vec<String>) {
    store_fav(favs)
}

pub fn main_get_peer_sync(id: String) -> SyncReturn<String> {
    let conf = get_peer(id);
    SyncReturn(serde_json::to_string(&conf).unwrap_or("".to_string()))
}

pub fn main_get_lan_peers() -> String {
    serde_json::to_string(&get_lan_peers()).unwrap_or_default()
}

pub fn main_get_connect_status() -> String {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        serde_json::to_string(&get_connect_status()).unwrap_or("".to_string())
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let mut state = hbb_common::config::get_online_state();
        if state > 0 {
            state = 1;
        }
        serde_json::json!({ "status_num": state }).to_string()
    }
}

pub fn main_check_connect_status() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    start_option_status_sync(); // avoid multi calls
}

pub fn main_is_using_public_server() -> bool {
    crate::using_public_server()
}

pub fn main_discover() {
    discover();
}

pub fn main_get_api_server() -> String {
    get_api_server()
}

pub fn main_http_request(url: String, method: String, body: Option<String>, header: String) {
    http_request(url, method, body, header)
}

pub fn main_get_local_option(key: String) -> SyncReturn<String> {
    SyncReturn(get_local_option(key))
}

pub fn main_get_use_texture_render() -> SyncReturn<bool> {
    SyncReturn(use_texture_render())
}

pub fn main_get_env(key: String) -> SyncReturn<String> {
    SyncReturn(std::env::var(key).unwrap_or_default())
}

pub fn main_set_local_option(key: String, value: String) {
    let is_texture_render_key = key.eq(config::keys::OPTION_TEXTURE_RENDER);
    set_local_option(key, value.clone());
    if is_texture_render_key {
        let session_event = [("v", &value)];
        for session in sessions::get_sessions() {
            session.push_event("use_texture_render", &session_event, &[]);
            session.use_texture_render_changed();
            session.ui_handler.update_use_texture_render();
        }
    }
}

// We do use use `main_get_local_option` and `main_set_local_option`.
//
// 1. For get, the value is stored in the server process.
// 2. For clear, we need to need to return the error mmsg from the server process to flutter.
pub fn main_handle_wayland_screencast_restore_token(_key: String, _value: String) -> String {
    #[cfg(not(target_os = "linux"))]
    {
        return "".to_owned();
    }
    #[cfg(target_os = "linux")]
    if _value == "get" {
        match crate::ipc::get_wayland_screencast_restore_token(_key) {
            Ok(v) => v,
            Err(e) => {
                log::error!("Failed to get wayland screencast restore token, {}", e);
                "".to_owned()
            }
        }
    } else if _value == "clear" {
        match crate::ipc::clear_wayland_screencast_restore_token(_key.clone()) {
            Ok(true) => {
                set_local_option(_key, "".to_owned());
                "".to_owned()
            }
            Ok(false) => "Failed to clear, please try again.".to_owned(),
            Err(e) => format!("Failed to clear, {}", e),
        }
    } else {
        "".to_owned()
    }
}

pub fn main_get_input_source() -> SyncReturn<String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let input_source = get_cur_session_input_source();
    #[cfg(any(target_os = "android", target_os = "ios"))]
    let input_source = "".to_owned();
    SyncReturn(input_source)
}

pub fn main_set_input_source(session_id: SessionID, value: String) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        change_input_source(session_id, value);
        if let Some(session) = sessions::get_session_by_session_id(&session_id) {
            try_sync_peer_option(&session, &session_id, "input_source", None);
        }
    }
}

pub fn main_get_my_id() -> String {
    get_id()
}

pub fn main_get_uuid() -> String {
    get_uuid()
}

pub fn main_get_peer_option(id: String, key: String) -> String {
    get_peer_option(id, key)
}

pub fn main_get_peer_option_sync(id: String, key: String) -> SyncReturn<String> {
    SyncReturn(get_peer_option(id, key))
}

// Sometimes we need to get the flutter option of a peer by reading the file.
// Because the session may not be established yet.
pub fn main_get_peer_flutter_option_sync(id: String, k: String) -> SyncReturn<String> {
    SyncReturn(get_peer_flutter_option(id, k))
}

pub fn main_set_peer_flutter_option_sync(id: String, k: String, v: String) -> SyncReturn<()> {
    set_peer_flutter_option(id, k, v);
    SyncReturn(())
}

pub fn main_set_peer_option(id: String, key: String, value: String) {
    set_peer_option(id, key, value)
}

pub fn main_set_peer_option_sync(id: String, key: String, value: String) -> SyncReturn<bool> {
    set_peer_option(id, key, value);
    SyncReturn(true)
}

pub fn main_set_peer_alias(id: String, alias: String) {
    set_peer_option(id, "alias".to_owned(), alias)
}

pub fn main_get_new_stored_peers() -> String {
    let peers: Vec<String> = config::NEW_STORED_PEER_CONFIG
        .lock()
        .unwrap()
        .drain()
        .collect();
    serde_json::to_string(&peers).unwrap_or_default()
}

pub fn main_forget_password(id: String) {
    forget_password(id)
}

pub fn main_peer_has_password(id: String) -> bool {
    peer_has_password(id)
}

pub fn main_peer_exists(id: String) -> bool {
    peer_exists(&id)
}

fn load_recent_peers(
    vec_id_modified_time_path: &Vec<(String, SystemTime, std::path::PathBuf)>,
    to_end: bool,
    all_peers: &mut Vec<HashMap<&str, String>>,
    from: usize,
) -> usize {
    let to = if to_end {
        Some(vec_id_modified_time_path.len())
    } else {
        None
    };
    let mut peers_next = PeerConfig::batch_peers(vec_id_modified_time_path, from, to);
    // There may be less peers than the batch size.
    // But no need to consider this case, because it is a rare case.
    let peers = peers_next.0.drain(..).map(|(id, _, p)| peer_to_map(id, p));
    all_peers.extend(peers);
    peers_next.1
}

pub fn main_load_recent_peers() {
    let push_to_flutter = |peers, ids| {
        let mut data = HashMap::from([("name", "load_recent_peers".to_owned()), ("peers", peers)]);
        if let Some(ids) = ids {
            data.insert("ids", ids);
        }
        let _res = flutter::push_global_event(
            flutter::APP_TYPE_MAIN,
            serde_json::ser::to_string(&data).unwrap_or("".to_owned()),
        );
    };

    if !config::APP_DIR.read().unwrap().is_empty() {
        let vec_id_modified_time_path = PeerConfig::get_vec_id_modified_time_path(&None);
        if vec_id_modified_time_path.is_empty() {
            push_to_flutter("".to_owned(), None);
            return;
        }

        let load_two_times = vec_id_modified_time_path.len() > PeerConfig::BATCH_LOADING_COUNT
            && cfg!(target_os = "windows");
        let mut all_peers = vec![];
        if load_two_times {
            let next_from = load_recent_peers(&vec_id_modified_time_path, false, &mut all_peers, 0);
            let rest_ids = if next_from < vec_id_modified_time_path.len() {
                Some(
                    vec_id_modified_time_path[next_from..]
                        .iter()
                        .map(|(id, _, _)| id.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
            } else {
                None
            };
            push_to_flutter(
                serde_json::ser::to_string(&all_peers).unwrap_or("".to_owned()),
                rest_ids,
            );
            let _ = load_recent_peers(&vec_id_modified_time_path, true, &mut all_peers, next_from);
        } else {
            let _ = load_recent_peers(&vec_id_modified_time_path, true, &mut all_peers, 0);
        }
        // Don't check if `all_peers` is empty, because we need this message to update the state in the flutter side.
        push_to_flutter(
            serde_json::ser::to_string(&all_peers).unwrap_or("".to_owned()),
            None,
        );
    } else {
        push_to_flutter("".to_owned(), None)
    }
}

pub fn main_load_recent_peers_for_ab(filter: String) -> String {
    let id_filters = serde_json::from_str::<Vec<String>>(&filter).unwrap_or_default();
    let id_filters = if id_filters.is_empty() {
        None
    } else {
        Some(id_filters)
    };
    if !config::APP_DIR.read().unwrap().is_empty() {
        let peers: Vec<HashMap<&str, String>> = PeerConfig::peers(id_filters)
            .drain(..)
            .map(|(id, _, p)| peer_to_map(id, p))
            .collect();
        return serde_json::ser::to_string(&peers).unwrap_or("".to_owned());
    }
    "".to_string()
}

pub fn main_load_fav_peers() {
    let push_to_flutter = |peers| {
        let data = HashMap::from([("name", "load_fav_peers".to_owned()), ("peers", peers)]);
        let _res = flutter::push_global_event(
            flutter::APP_TYPE_MAIN,
            serde_json::ser::to_string(&data).unwrap_or("".to_owned()),
        );
    };
    if !config::APP_DIR.read().unwrap().is_empty() {
        let favs = get_fav();
        let mut recent = PeerConfig::peers(Some(favs.clone()));
        let mut lan = config::LanPeers::load()
            .peers
            .iter()
            .filter(|d| favs.contains(&d.id) && recent.iter().all(|r| r.0 != d.id))
            .map(|d| {
                (
                    d.id.clone(),
                    SystemTime::UNIX_EPOCH,
                    PeerConfig {
                        info: PeerInfoSerde {
                            username: d.username.clone(),
                            hostname: d.hostname.clone(),
                            platform: d.platform.clone(),
                        },
                        ..Default::default()
                    },
                )
            })
            .collect();
        recent.append(&mut lan);
        let peers: Vec<HashMap<&str, String>> = recent
            .into_iter()
            .map(|(id, _, p)| peer_to_map(id, p))
            .collect();

        push_to_flutter(serde_json::ser::to_string(&peers).unwrap_or("".to_owned()));
    } else {
        push_to_flutter("".to_owned());
    }
}

pub fn main_load_lan_peers() {
    let data = HashMap::from([
        ("name", "load_lan_peers".to_owned()),
        (
            "peers",
            serde_json::to_string(&get_lan_peers()).unwrap_or_default(),
        ),
    ]);
    let _res = flutter::push_global_event(
        flutter::APP_TYPE_MAIN,
        serde_json::ser::to_string(&data).unwrap_or("".to_owned()),
    );
}

pub fn main_remove_discovered(id: String) {
    remove_discovered(id);
}

fn main_broadcast_message(data: &HashMap<&str, &str>) {
    let event = serde_json::ser::to_string(&data).unwrap_or("".to_owned());
    for app in flutter::get_global_event_channels() {
        if app == flutter::APP_TYPE_MAIN || app == flutter::APP_TYPE_CM {
            continue;
        }
        let _res = flutter::push_global_event(&app, event.clone());
    }
}

pub fn main_change_theme(dark: String) {
    main_broadcast_message(&HashMap::from([("name", "theme"), ("dark", &dark)]));
    #[cfg(not(any(target_os = "ios")))]
    send_to_cm(&crate::ipc::Data::Theme(dark));
}

pub fn main_change_language(lang: String) {
    main_broadcast_message(&HashMap::from([("name", "language"), ("lang", &lang)]));
    #[cfg(not(any(target_os = "ios")))]
    send_to_cm(&crate::ipc::Data::Language(lang));
}

pub fn main_video_save_directory(root: bool) -> SyncReturn<String> {
    SyncReturn(video_save_directory(root))
}

pub fn main_set_user_default_option(key: String, value: String) {
    set_user_default_option(key, value);
}

pub fn main_get_user_default_option(key: String) -> SyncReturn<String> {
    SyncReturn(get_user_default_option(key))
}

pub fn main_handle_relay_id(id: String) -> String {
    handle_relay_id(&id).to_owned()
}

pub fn main_is_option_fixed(key: String) -> SyncReturn<bool> {
    SyncReturn(
        config::OVERWRITE_DISPLAY_SETTINGS
            .read()
            .unwrap()
            .contains_key(&key)
            || config::OVERWRITE_LOCAL_SETTINGS
                .read()
                .unwrap()
                .contains_key(&key)
            || config::OVERWRITE_SETTINGS
                .read()
                .unwrap()
                .contains_key(&key),
    )
}

pub fn main_get_main_display() -> SyncReturn<String> {
    #[cfg(target_os = "ios")]
    let display_info = "".to_owned();
    #[cfg(not(target_os = "ios"))]
    let mut display_info = "".to_owned();
    #[cfg(not(target_os = "ios"))]
    if let Ok(displays) = crate::display_service::try_get_displays() {
        // to-do: Need to detect current display index.
        if let Some(display) = displays.iter().next() {
            display_info = serde_json::to_string(&HashMap::from([
                ("w", display.width()),
                ("h", display.height()),
            ]))
            .unwrap_or_default();
        }
    }
    SyncReturn(display_info)
}

pub fn main_get_displays() -> SyncReturn<String> {
    #[cfg(target_os = "ios")]
    let display_info = "".to_owned();
    #[cfg(not(target_os = "ios"))]
    let mut display_info = "".to_owned();
    #[cfg(not(target_os = "ios"))]
    if let Ok(displays) = crate::display_service::try_get_displays() {
        let displays = displays
            .iter()
            .map(|d| {
                HashMap::from([
                    ("x", d.origin().0),
                    ("y", d.origin().1),
                    ("w", d.width() as i32),
                    ("h", d.height() as i32),
                ])
            })
            .collect::<Vec<_>>();
        display_info = serde_json::to_string(&displays).unwrap_or_default();
    }
    SyncReturn(display_info)
}

pub fn session_add_port_forward(
    session_id: SessionID,
    local_port: i32,
    remote_host: String,
    remote_port: i32,
) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.add_port_forward(local_port, remote_host, remote_port);
    }
}

pub fn session_remove_port_forward(session_id: SessionID, local_port: i32) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.remove_port_forward(local_port);
    }
}

pub fn session_new_rdp(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.new_rdp();
    }
}

pub fn session_request_voice_call(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.request_voice_call();
    }
}

pub fn session_close_voice_call(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.close_voice_call();
    }
}

pub fn session_get_conn_token(session_id: SessionID) -> SyncReturn<Option<String>> {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        SyncReturn(session.get_conn_token())
    } else {
        SyncReturn(None)
    }
}

pub fn cm_handle_incoming_voice_call(id: i32, accept: bool) {
    crate::ui_cm_interface::handle_incoming_voice_call(id, accept);
}

pub fn cm_close_voice_call(id: i32) {
    crate::ui_cm_interface::close_voice_call(id);
}

pub fn set_voice_call_input_device(_is_cm: bool, _device: String) {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if _is_cm {
        let _ = crate::ipc::set_config("voice-call-input", _device);
    } else {
        crate::audio_service::set_voice_call_input_device(Some(_device), true);
    }
}

pub fn get_voice_call_input_device(_is_cm: bool) -> String {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    if _is_cm {
        match crate::ipc::get_config("voice-call-input") {
            Ok(Some(device)) => device,
            _ => "".to_owned(),
        }
    } else {
        crate::audio_service::get_voice_call_input_device().unwrap_or_default()
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    "".to_owned()
}

pub fn main_get_last_remote_id() -> String {
    LocalConfig::get_remote_id()
}

pub fn main_get_software_update_url() {
    let opt = get_local_option(config::keys::OPTION_ENABLE_CHECK_UPDATE.to_string());
    if config::option2bool(config::keys::OPTION_ENABLE_CHECK_UPDATE, &opt) {
        crate::common::check_software_update();
    }
}

pub fn main_get_home_dir() -> String {
    fs::get_home_as_string()
}

pub fn main_get_langs() -> String {
    get_langs()
}

pub fn main_get_temporary_password() -> String {
    ui_interface::temporary_password()
}

pub fn main_get_permanent_password() -> String {
    ui_interface::permanent_password()
}

pub fn main_get_fingerprint() -> String {
    get_fingerprint()
}

pub fn cm_get_clients_state() -> String {
    crate::ui_cm_interface::get_clients_state()
}

pub fn cm_check_clients_length(length: usize) -> Option<String> {
    if length != crate::ui_cm_interface::get_clients_length() {
        Some(crate::ui_cm_interface::get_clients_state())
    } else {
        None
    }
}

pub fn cm_get_clients_length() -> usize {
    crate::ui_cm_interface::get_clients_length()
}

pub fn main_init(app_dir: String, custom_client_config: String) {
    initialize(&app_dir, &custom_client_config);
}

pub fn main_device_id(id: String) {
    *crate::common::DEVICE_ID.lock().unwrap() = id;
}

pub fn main_device_name(name: String) {
    *crate::common::DEVICE_NAME.lock().unwrap() = name;
}

pub fn main_remove_peer(id: String) {
    PeerConfig::remove(&id);
}

pub fn main_has_hwcodec() -> SyncReturn<bool> {
    SyncReturn(has_hwcodec())
}

pub fn main_has_vram() -> SyncReturn<bool> {
    SyncReturn(has_vram())
}

pub fn main_supported_hwdecodings() -> SyncReturn<String> {
    let decoding = supported_hwdecodings();
    let msg = HashMap::from([("h264", decoding.0), ("h265", decoding.1)]);

    SyncReturn(serde_json::ser::to_string(&msg).unwrap_or("".to_owned()))
}

pub fn main_is_root() -> bool {
    is_root()
}

pub fn get_double_click_time() -> SyncReturn<i32> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        return SyncReturn(crate::platform::get_double_click_time() as _);
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    SyncReturn(500i32)
}

pub fn main_start_dbus_server() {
    #[cfg(target_os = "linux")]
    {
        use crate::dbus::start_dbus_server;
        // spawn new thread to start dbus server
        std::thread::spawn(|| {
            let _ = start_dbus_server();
        });
    }
}

pub fn main_save_ab(json: String) {
    if json.len() > 1024 {
        std::thread::spawn(|| {
            config::Ab::store(json);
        });
    } else {
        config::Ab::store(json);
    }
}

pub fn main_clear_ab() {
    config::Ab::remove();
}

pub fn main_load_ab() -> String {
    serde_json::to_string(&config::Ab::load()).unwrap_or_default()
}

pub fn main_save_group(json: String) {
    if json.len() > 1024 {
        std::thread::spawn(|| {
            config::Group::store(json);
        });
    } else {
        config::Group::store(json);
    }
}

pub fn main_clear_group() {
    config::Group::remove();
}

pub fn main_load_group() -> String {
    serde_json::to_string(&config::Group::load()).unwrap_or_default()
}

pub fn session_send_pointer(session_id: SessionID, msg: String) {
    super::flutter::session_send_pointer(session_id, msg);
}

pub fn session_send_mouse(session_id: SessionID, msg: String) {
    if let Ok(m) = serde_json::from_str::<HashMap<String, String>>(&msg) {
        let alt = m.get("alt").is_some();
        let ctrl = m.get("ctrl").is_some();
        let shift = m.get("shift").is_some();
        let command = m.get("command").is_some();
        let x = m
            .get("x")
            .map(|x| x.parse::<i32>().unwrap_or(0))
            .unwrap_or(0);
        let y = m
            .get("y")
            .map(|x| x.parse::<i32>().unwrap_or(0))
            .unwrap_or(0);
        let mut mask = 0;
        if let Some(_type) = m.get("type") {
            mask = match _type.as_str() {
                "down" => MOUSE_TYPE_DOWN,
                "up" => MOUSE_TYPE_UP,
                "wheel" => MOUSE_TYPE_WHEEL,
                "trackpad" => MOUSE_TYPE_TRACKPAD,
                _ => 0,
            };
        }
        if let Some(buttons) = m.get("buttons") {
            mask |= match buttons.as_str() {
                "left" => MOUSE_BUTTON_LEFT,
                "right" => MOUSE_BUTTON_RIGHT,
                "wheel" => MOUSE_BUTTON_WHEEL,
                "back" => MOUSE_BUTTON_BACK,
                "forward" => MOUSE_BUTTON_FORWARD,
                _ => 0,
            } << 3;
        }
        if let Some(session) = sessions::get_session_by_session_id(&session_id) {
            session.send_mouse(mask, x, y, alt, ctrl, shift, command);
        }
    }
}

pub fn session_restart_remote_device(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.restart_remote_device();
    }
}

pub fn session_get_audit_server_sync(session_id: SessionID, typ: String) -> SyncReturn<String> {
    let res = if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.get_audit_server(typ)
    } else {
        "".to_owned()
    };
    SyncReturn(res)
}

pub fn session_send_note(session_id: SessionID, note: String) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.send_note(note)
    }
}

pub fn session_alternative_codecs(session_id: SessionID) -> String {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        let (vp8, av1, h264, h265) = session.alternative_codecs();
        let msg = HashMap::from([("vp8", vp8), ("av1", av1), ("h264", h264), ("h265", h265)]);
        serde_json::ser::to_string(&msg).unwrap_or("".to_owned())
    } else {
        String::new()
    }
}

pub fn session_change_prefer_codec(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.change_prefer_codec();
    }
}

pub fn session_on_waiting_for_image_dialog_show(session_id: SessionID) {
    super::flutter::session_on_waiting_for_image_dialog_show(session_id);
}

pub fn session_toggle_virtual_display(session_id: SessionID, index: i32, on: bool) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.toggle_virtual_display(index, on);
        flutter::session_update_virtual_display(&session, index, on);
    }
}

pub fn main_set_home_dir(_home: String) {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        *config::APP_HOME_DIR.write().unwrap() = _home;
    }
}

// This is a temporary method to get data dir for ios
pub fn main_get_data_dir_ios() -> SyncReturn<String> {
    let data_dir = config::Config::path("data");
    if !data_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&data_dir) {
            log::warn!("Failed to create data dir {}", e);
        }
    }
    SyncReturn(data_dir.to_string_lossy().to_string())
}

pub fn main_stop_service() {
    #[cfg(target_os = "android")]
    {
        config::Config::set_option("stop-service".into(), "Y".into());
        crate::rendezvous_mediator::RendezvousMediator::restart();
    }
}

pub fn main_start_service() {
    #[cfg(target_os = "android")]
    {
        config::Config::set_option("stop-service".into(), "".into());
        crate::rendezvous_mediator::RendezvousMediator::restart();
    }
}

pub fn main_update_temporary_password() {
    update_temporary_password();
}

pub fn main_set_permanent_password(password: String) {
    set_permanent_password(password);
}

pub fn main_check_super_user_permission() -> bool {
    check_super_user_permission()
}

pub fn main_get_unlock_pin() -> SyncReturn<String> {
    SyncReturn(get_unlock_pin())
}

pub fn main_set_unlock_pin(pin: String) -> SyncReturn<String> {
    SyncReturn(set_unlock_pin(pin))
}

pub fn main_check_mouse_time() {
    check_mouse_time();
}

pub fn main_get_mouse_time() -> f64 {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        get_mouse_time()
    }
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        0.0
    }
}

pub fn main_wol(id: String) {
    // TODO: move send_wol outside.
    #[cfg(not(any(target_os = "ios")))]
    crate::lan::send_wol(id)
}

pub fn main_create_shortcut(_id: String) {
    #[cfg(windows)]
    create_shortcut(_id);
}

pub fn cm_send_chat(conn_id: i32, msg: String) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::send_chat(conn_id, msg);
}

pub fn cm_login_res(conn_id: i32, res: bool) {
    #[cfg(not(any(target_os = "ios")))]
    if res {
        crate::ui_cm_interface::authorize(conn_id);
    } else {
        crate::ui_cm_interface::close(conn_id);
    }
}

pub fn cm_close_connection(conn_id: i32) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::close(conn_id);
}

pub fn cm_remove_disconnected_connection(conn_id: i32) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::remove(conn_id);
}

pub fn cm_check_click_time(conn_id: i32) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::check_click_time(conn_id)
}

pub fn cm_get_click_time() -> f64 {
    #[cfg(not(any(target_os = "ios")))]
    return crate::ui_cm_interface::get_click_time() as _;
    #[cfg(any(target_os = "ios"))]
    return 0 as _;
}

pub fn cm_switch_permission(conn_id: i32, name: String, enabled: bool) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::switch_permission(conn_id, name, enabled)
}

pub fn cm_can_elevate() -> SyncReturn<bool> {
    SyncReturn(crate::ui_cm_interface::can_elevate())
}

pub fn cm_elevate_portable(conn_id: i32) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::elevate_portable(conn_id);
}

pub fn cm_switch_back(conn_id: i32) {
    #[cfg(not(any(target_os = "ios")))]
    crate::ui_cm_interface::switch_back(conn_id);
}

pub fn cm_get_config(name: String) -> String {
    #[cfg(not(target_os = "ios"))]
    {
        if let Ok(Some(v)) = crate::ipc::get_config(&name) {
            v
        } else {
            "".to_string()
        }
    }
    #[cfg(target_os = "ios")]
    {
        "".to_string()
    }
}

pub fn main_get_build_date() -> String {
    crate::BUILD_DATE.to_string()
}

pub fn translate(name: String, locale: String) -> SyncReturn<String> {
    SyncReturn(crate::client::translate_locale(name, &locale))
}

pub fn session_get_rgba_size(session_id: SessionID, display: usize) -> SyncReturn<usize> {
    SyncReturn(super::flutter::session_get_rgba_size(session_id, display))
}

pub fn session_next_rgba(session_id: SessionID, display: usize) -> SyncReturn<()> {
    SyncReturn(super::flutter::session_next_rgba(session_id, display))
}

pub fn session_register_pixelbuffer_texture(
    session_id: SessionID,
    display: usize,
    ptr: usize,
) -> SyncReturn<()> {
    SyncReturn(super::flutter::session_register_pixelbuffer_texture(
        session_id, display, ptr,
    ))
}

pub fn session_register_gpu_texture(
    session_id: SessionID,
    display: usize,
    ptr: usize,
) -> SyncReturn<()> {
    SyncReturn(super::flutter::session_register_gpu_texture(
        session_id, display, ptr,
    ))
}

pub fn query_onlines(ids: Vec<String>) {
    let _ = flutter::async_tasks::query_onlines(ids);
}

pub fn version_to_number(v: String) -> SyncReturn<i64> {
    SyncReturn(hbb_common::get_version_number(&v))
}

pub fn option_synced() -> bool {
    crate::ui_interface::option_synced()
}

pub fn main_is_installed() -> SyncReturn<bool> {
    SyncReturn(is_installed())
}

pub fn main_init_input_source() -> SyncReturn<()> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::keyboard::input_source::init_input_source();
    SyncReturn(())
}

pub fn main_is_installed_lower_version() -> SyncReturn<bool> {
    SyncReturn(is_installed_lower_version())
}

pub fn main_is_installed_daemon(prompt: bool) -> SyncReturn<bool> {
    SyncReturn(is_installed_daemon(prompt))
}

pub fn main_is_process_trusted(prompt: bool) -> SyncReturn<bool> {
    SyncReturn(is_process_trusted(prompt))
}

pub fn main_is_can_screen_recording(prompt: bool) -> SyncReturn<bool> {
    SyncReturn(is_can_screen_recording(prompt))
}

pub fn main_is_can_input_monitoring(prompt: bool) -> SyncReturn<bool> {
    SyncReturn(is_can_input_monitoring(prompt))
}

pub fn main_is_share_rdp() -> SyncReturn<bool> {
    SyncReturn(is_share_rdp())
}

pub fn main_set_share_rdp(enable: bool) {
    set_share_rdp(enable)
}

pub fn main_goto_install() -> SyncReturn<bool> {
    goto_install();
    SyncReturn(true)
}

pub fn main_get_new_version() -> SyncReturn<String> {
    SyncReturn(get_new_version())
}

pub fn main_update_me() -> SyncReturn<bool> {
    update_me("".to_owned());
    SyncReturn(true)
}

pub fn set_cur_session_id(session_id: SessionID) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        set_cur_session_id_(session_id, &session.get_keyboard_mode())
    }
}

fn set_cur_session_id_(session_id: SessionID, _keyboard_mode: &str) {
    super::flutter::set_cur_session_id(session_id);
    #[cfg(windows)]
    crate::keyboard::update_grab_get_key_name(_keyboard_mode);
}

pub fn install_show_run_without_install() -> SyncReturn<bool> {
    SyncReturn(show_run_without_install())
}

pub fn install_run_without_install() {
    run_without_install();
}

pub fn install_install_me(options: String, path: String) {
    install_me(options, path, false, false);
}

pub fn install_install_path() -> SyncReturn<String> {
    SyncReturn(install_path())
}

pub fn install_install_options() -> SyncReturn<String> {
    SyncReturn(install_options())
}

pub fn main_account_auth(op: String, remember_me: bool) {
    let id = get_id();
    let uuid = get_uuid();
    account_auth(op, id, uuid, remember_me);
}

pub fn main_account_auth_cancel() {
    account_auth_cancel()
}

pub fn main_account_auth_result() -> String {
    account_auth_result()
}

pub fn main_on_main_window_close() {
    // may called more than one times
    #[cfg(windows)]
    crate::portable_service::client::drop_portable_service_shared_memory();
}

pub fn main_current_is_wayland() -> SyncReturn<bool> {
    SyncReturn(current_is_wayland())
}

pub fn main_is_login_wayland() -> SyncReturn<bool> {
    SyncReturn(is_login_wayland())
}

pub fn main_hide_dock() -> SyncReturn<bool> {
    #[cfg(target_os = "macos")]
    crate::platform::macos::hide_dock();
    SyncReturn(true)
}

pub fn main_has_file_clipboard() -> SyncReturn<bool> {
    let ret = cfg!(any(target_os = "windows", feature = "unix-file-copy-paste",));
    SyncReturn(ret)
}

pub fn main_has_gpu_texture_render() -> SyncReturn<bool> {
    SyncReturn(cfg!(feature = "vram"))
}

pub fn cm_init() {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    crate::flutter::connection_manager::cm_init();
}

/// Start an ipc server for receiving the url scheme.
///
/// * Should only be called in the main flutter window.
/// * macOS only
pub fn main_start_ipc_url_server() {
    #[cfg(target_os = "macos")]
    std::thread::spawn(move || crate::server::start_ipc_url_server());
}

pub fn main_test_wallpaper(_second: u64) {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    std::thread::spawn(move || match crate::platform::WallPaperRemover::new() {
        Ok(_remover) => {
            std::thread::sleep(std::time::Duration::from_secs(_second));
        }
        Err(e) => {
            log::info!("create wallpaper remover failed: {:?}", e);
        }
    });
}

pub fn main_support_remove_wallpaper() -> bool {
    support_remove_wallpaper()
}

pub fn is_incoming_only() -> SyncReturn<bool> {
    SyncReturn(config::is_incoming_only())
}

pub fn is_outgoing_only() -> SyncReturn<bool> {
    SyncReturn(config::is_outgoing_only())
}

pub fn is_custom_client() -> SyncReturn<bool> {
    SyncReturn(get_app_name() != "RustDesk")
}

pub fn is_disable_settings() -> SyncReturn<bool> {
    SyncReturn(config::is_disable_settings())
}

pub fn is_disable_ab() -> SyncReturn<bool> {
    SyncReturn(config::is_disable_ab())
}

pub fn is_disable_account() -> SyncReturn<bool> {
    SyncReturn(config::is_disable_account())
}

pub fn is_disable_group_panel() -> SyncReturn<bool> {
    SyncReturn(LocalConfig::get_option("disable-group-panel") == "Y")
}

// windows only
pub fn is_disable_installation() -> SyncReturn<bool> {
    SyncReturn(config::is_disable_installation())
}

pub fn is_preset_password() -> bool {
    config::HARD_SETTINGS
        .read()
        .unwrap()
        .get("password")
        .map_or(false, |p| {
            #[cfg(not(any(target_os = "android", target_os = "ios")))]
            return p == &crate::ipc::get_permanent_password();
            #[cfg(any(target_os = "android", target_os = "ios"))]
            return p == &config::Config::get_permanent_password();
        })
}

// Don't call this function for desktop version.
// We need this function because we want a sync return for mobile version.
pub fn is_preset_password_mobile_only() -> SyncReturn<bool> {
    SyncReturn(is_preset_password())
}

/// Send a url scheme through the ipc.
///
/// * macOS only
#[allow(unused_variables)]
pub fn send_url_scheme(_url: String) {
    #[cfg(target_os = "macos")]
    std::thread::spawn(move || crate::handle_url_scheme(_url));
}

#[inline]
pub fn plugin_event(_id: String, _peer: String, _event: Vec<u8>) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        allow_err!(crate::plugin::handle_ui_event(&_id, &_peer, &_event));
    }
}

pub fn plugin_register_event_stream(_id: String, _event2ui: StreamSink<EventToUI>) {
    #[cfg(feature = "plugin_framework")]
    {
        crate::plugin::native_handlers::session::session_register_event_stream(_id, _event2ui);
    }
}

#[inline]
pub fn plugin_get_session_option(
    _id: String,
    _peer: String,
    _key: String,
) -> SyncReturn<Option<String>> {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        SyncReturn(crate::plugin::PeerConfig::get(&_id, &_peer, &_key))
    }
    #[cfg(any(
        not(feature = "plugin_framework"),
        target_os = "android",
        target_os = "ios"
    ))]
    {
        SyncReturn(None)
    }
}

#[inline]
pub fn plugin_set_session_option(_id: String, _peer: String, _key: String, _value: String) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let _res = crate::plugin::PeerConfig::set(&_id, &_peer, &_key, &_value);
    }
}

#[inline]
pub fn plugin_get_shared_option(_id: String, _key: String) -> SyncReturn<Option<String>> {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        SyncReturn(crate::plugin::ipc::get_config(&_id, &_key).unwrap_or(None))
    }
    #[cfg(any(
        not(feature = "plugin_framework"),
        target_os = "android",
        target_os = "ios"
    ))]
    {
        SyncReturn(None)
    }
}

#[inline]
pub fn plugin_set_shared_option(_id: String, _key: String, _value: String) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        allow_err!(crate::plugin::ipc::set_config(&_id, &_key, _value));
    }
}

#[inline]
pub fn plugin_reload(_id: String) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        allow_err!(crate::plugin::ipc::reload_plugin(&_id,));
        allow_err!(crate::plugin::reload_plugin(&_id));
    }
}

#[inline]
pub fn plugin_enable(_id: String, _v: bool) -> SyncReturn<()> {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        allow_err!(crate::plugin::ipc::set_manager_plugin_config(
            &_id,
            "enabled",
            _v.to_string()
        ));
        if _v {
            allow_err!(crate::plugin::load_plugin(&_id));
        } else {
            crate::plugin::unload_plugin(&_id);
        }
    }
    SyncReturn(())
}

pub fn plugin_is_enabled(_id: String) -> SyncReturn<bool> {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        SyncReturn(
            match crate::plugin::ipc::get_manager_plugin_config(&_id, "enabled") {
                Ok(Some(enabled)) => bool::from_str(&enabled).unwrap_or(false),
                _ => false,
            },
        )
    }
    #[cfg(any(
        not(feature = "plugin_framework"),
        target_os = "android",
        target_os = "ios"
    ))]
    {
        SyncReturn(false)
    }
}

pub fn plugin_feature_is_enabled() -> SyncReturn<bool> {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        #[cfg(debug_assertions)]
        let enabled = true;
        #[cfg(not(debug_assertions))]
        let enabled = is_installed();
        SyncReturn(enabled)
    }
    #[cfg(any(
        not(feature = "plugin_framework"),
        target_os = "android",
        target_os = "ios"
    ))]
    {
        SyncReturn(false)
    }
}

pub fn plugin_sync_ui(_sync_to: String) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if plugin_feature_is_enabled().0 {
            crate::plugin::sync_ui(_sync_to);
        }
    }
}

pub fn plugin_list_reload() {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        crate::plugin::load_plugin_list();
    }
}

pub fn plugin_install(_id: String, _b: bool) {
    #[cfg(feature = "plugin_framework")]
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        if _b {
            if let Err(e) = crate::plugin::install_plugin(&_id) {
                log::error!("Failed to install plugin '{}': {}", _id, e);
            }
        } else {
            crate::plugin::uninstall_plugin(&_id, true);
        }
    }
}

pub fn is_support_multi_ui_session(version: String) -> SyncReturn<bool> {
    SyncReturn(crate::common::is_support_multi_ui_session(&version))
}

pub fn is_selinux_enforcing() -> SyncReturn<bool> {
    #[cfg(target_os = "linux")]
    {
        SyncReturn(crate::platform::linux::is_selinux_enforcing())
    }
    #[cfg(not(target_os = "linux"))]
    {
        SyncReturn(false)
    }
}

pub fn main_default_privacy_mode_impl() -> SyncReturn<String> {
    SyncReturn(crate::privacy_mode::DEFAULT_PRIVACY_MODE_IMPL.to_owned())
}

pub fn main_supported_privacy_mode_impls() -> SyncReturn<String> {
    SyncReturn(
        serde_json::to_string(&crate::privacy_mode::get_supported_privacy_mode_impl())
            .unwrap_or_default(),
    )
}

pub fn main_supported_input_source() -> SyncReturn<String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        SyncReturn("".to_owned())
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        SyncReturn(
            serde_json::to_string(&crate::keyboard::input_source::get_supported_input_source())
                .unwrap_or_default(),
        )
    }
}

pub fn main_generate2fa() -> String {
    generate2fa()
}

pub fn main_verify2fa(code: String) -> bool {
    verify2fa(code)
}

pub fn main_has_valid_2fa_sync() -> SyncReturn<bool> {
    SyncReturn(has_valid_2fa())
}

pub fn main_verify_bot(token: String) -> String {
    verify_bot(token)
}

pub fn main_has_valid_bot_sync() -> SyncReturn<bool> {
    SyncReturn(has_valid_bot())
}

pub fn main_get_hard_option(key: String) -> SyncReturn<String> {
    SyncReturn(get_hard_option(key))
}

pub fn main_get_buildin_option(key: String) -> SyncReturn<String> {
    SyncReturn(get_builtin_option(&key))
}

pub fn main_check_hwcodec() {
    check_hwcodec()
}

pub fn main_get_trusted_devices() -> String {
    get_trusted_devices()
}

pub fn main_remove_trusted_devices(json: String) {
    remove_trusted_devices(&json)
}

pub fn main_clear_trusted_devices() {
    clear_trusted_devices()
}

pub fn main_max_encrypt_len() -> SyncReturn<usize> {
    SyncReturn(max_encrypt_len())
}

pub fn session_request_new_display_init_msgs(session_id: SessionID, display: usize) {
    if let Some(session) = sessions::get_session_by_session_id(&session_id) {
        session.request_init_msgs(display);
    }
}

pub fn main_audio_support_loopback() -> SyncReturn<bool> {
    #[cfg(target_os = "windows")]
    let is_surpport = true;
    #[cfg(feature = "screencapturekit")]
    let is_surpport = crate::audio_service::is_screen_capture_kit_available();
    #[cfg(not(any(target_os = "windows", feature = "screencapturekit")))]
    let is_surpport = false;
    SyncReturn(is_surpport)
}

#[cfg(target_os = "android")]
pub mod server_side {
    use hbb_common::{config, log};
    use jni::{
        errors::{Error as JniError, Result as JniResult},
        objects::{JClass, JObject, JString},
        sys::{jboolean, jstring},
        JNIEnv,
    };

    use crate::start_server;

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_startServer(
        env: JNIEnv,
        _class: JClass,
        app_dir: JString,
        custom_client_config: JString,
    ) {
        log::debug!("startServer from jvm");
        let mut env = env;
        if let Ok(app_dir) = env.get_string(&app_dir) {
            *config::APP_DIR.write().unwrap() = app_dir.into();
        }
        if let Ok(custom_client_config) = env.get_string(&custom_client_config) {
            if !custom_client_config.is_empty() {
                let custom_client_config: String = custom_client_config.into();
                crate::read_custom_client(&custom_client_config);
            }
        }
        std::thread::spawn(move || start_server(true));
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_startService(_env: JNIEnv, _class: JClass) {
        log::debug!("startService from jvm");
        config::Config::set_option("stop-service".into(), "".into());
        crate::rendezvous_mediator::RendezvousMediator::restart();
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_translateLocale(
        env: JNIEnv,
        _class: JClass,
        locale: JString,
        input: JString,
    ) -> jstring {
        let mut env = env;
        let res = if let (Ok(input), Ok(locale)) = (env.get_string(&input), env.get_string(&locale))
        {
            let input: String = input.into();
            let locale: String = locale.into();
            crate::client::translate_locale(input, &locale)
        } else {
            "".into()
        };
        return env.new_string(res).unwrap_or(input).into_raw();
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_refreshScreen(_env: JNIEnv, _class: JClass) {
        crate::server::video_service::refresh()
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_getLocalOption(
        env: JNIEnv,
        _class: JClass,
        key: JString,
    ) -> jstring {
        let mut env = env;
        let res = if let Ok(key) = env.get_string(&key) {
            let key: String = key.into();
            super::get_local_option(key)
        } else {
            "".into()
        };
        return env.new_string(res).unwrap_or_default().into_raw();
    }

    #[no_mangle]
    pub unsafe extern "system" fn Java_ffi_FFI_isServiceClipboardEnabled(
        env: JNIEnv,
        _class: JClass,
    ) -> jboolean {
        jboolean::from(crate::server::is_clipboard_service_ok())
    }
}
