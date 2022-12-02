use hbb_common::tokio::sync::mpsc;

use crate::{
    ui::{remote::{TauriSession, PortForwards}, cm::TauriHandler}, 
    ui_session_interface::{InvokeUiSession, Session}, 
    client::{Interface, file_trait::FileManager}, 
    ui_cm_interface::ConnectionManager, ipc::Data, 
    
};

use std::{sync::Mutex};

#[tauri::command]
pub fn test_tauri(app: tauri::AppHandle,){
    
    // DEBUG //        
    let new_cm = ConnectionManager {
        ui_handler: TauriHandler::default(),
    };
    let (tx, rx) = mpsc::unbounded_channel::<Data>();
    new_cm.add_connection(&app, 1, false, "port_forward".to_string(), "peer_id".to_string(), "name".to_string(), true, true, true, true, false, true, true, tx.clone());
    // DEBUG //
}

#[tauri::command(async)]
pub fn get_audit_server(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_audit_server()
}

#[tauri::command(async)]
pub fn send_note(note: String, tauri_session: tauri::State<Mutex<TauriSession>>) {
    tauri_session.lock().unwrap().send_note(note)
}


#[tauri::command(async)]
pub fn get_session_id(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_id()
}

#[tauri::command(async)]
pub fn get_default_pi(tauri_session: tauri::State<Mutex<TauriSession>>) -> hbb_common::config::PeerInfoSerde {
    tauri_session.lock().unwrap().get_default_pi()
}

// get_option
#[tauri::command(async)]
pub fn get_session_option(key: String, tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_option(key)
}

// set_option
#[tauri::command(async)]
pub fn set_session_option(key: String, value: String, tauri_session: tauri::State<Mutex<TauriSession>>) {
    tauri_session.lock().unwrap().set_option(key, value)
}

// input_os_password,
#[tauri::command(async)]
pub fn input_os_password( pass: String, activate: bool, tauri_session: tauri::State<Mutex<TauriSession>>) {
    tauri_session.lock().unwrap().input_os_password(pass, activate)
}

#[tauri::command(async)]
pub fn save_close_state(k: String, v: String, tauri_session: tauri::State<Mutex<TauriSession>>) {
    tauri_session.lock().unwrap().save_close_state(k, v)
}

#[tauri::command(async)]
pub fn is_file_transfer(tauri_session: tauri::State<Mutex<TauriSession>>) -> bool {
    tauri_session.lock().unwrap().is_file_transfer()
}

#[tauri::command(async)]
pub fn is_port_forward(tauri_session: tauri::State<Mutex<TauriSession>>) -> bool {
    tauri_session.lock().unwrap().is_port_forward()
}

#[tauri::command(async)]
pub fn is_rdp(tauri_session: tauri::State<Mutex<TauriSession>>) -> bool {
    tauri_session.lock().unwrap().is_rdp()
}

#[tauri::command(async)]
pub fn login(
    password: String, 
    remember: bool,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().login(password, remember)
}

#[tauri::command(async)]
pub fn new_rdp(
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().new_rdp()
}

#[tauri::command(async)]
pub fn send_mouse(
    mask: i32, x: i32, y: i32, alt: bool, ctrl: bool, shift: bool, command: bool, tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().send_mouse(mask, x, y, alt, ctrl, shift, command)
}
#[tauri::command(async)]
pub fn enter(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().enter()
}

#[tauri::command(async)]
pub fn leave(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().leave()
}

#[tauri::command(async)]
pub fn ctrl_alt_del(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().ctrl_alt_del()
}

#[tauri::command(async)]
pub fn transfer_file(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().transfer_file()
}

# [tauri::command(async)]
pub fn tunnel(
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().tunnel()
}

#[tauri::command(async)]
pub fn lock_screen(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().lock_screen()
}
#[tauri::command(async)]
pub fn reconnect(app: tauri::AppHandle, tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().reconnect(app)
}

#[tauri::command(async)]
pub fn get_chatbox(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_chatbox()
}

#[tauri::command(async)]
pub fn get_home_dir(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_home_dir()
}

// client::read_dir, - scitter dependents
// client::remove_dir,- scitter dependents
// client::create_dir,- scitter dependents
// client::remove_file,- scitter dependents
// client::read_remote_dir, - dependents?
// client::send_chat,
#[tauri::command(async)]
pub fn send_chat(
    msg: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().send_chat(msg)
}

// client::switch_display,
#[tauri::command(async)]
pub fn switch_display(
    display: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().switch_display(display)
}

// client::remove_dir_all, - dependents?
// #[tauri::command(async)]
// pub fn remove_dir_all(
//     id: i32,
//     path: String,
//     is_remote: bool, 
//     include_hidden: bool,
//     tauri_session: tauri::State<Mutex<TauriSession>>,
// ){
//     tauri_session.lock().unwrap().remove_dir_all(id, path, is_remote, include_hidden)
// }

// client::confirm_delete_files, - dependents?

// client::set_no_confirm, - dependents?

// client::cancel_job, - dependents?

// client::send_files,  - dependents?

// client::add_job, - dependents?
// #[tauri::command(async)]
// pub fn add_job(
//     id: i32, 
//     path: String, 
//     to: String, 
//     file_num: i32, 
//     include_hidden: bool, 
//     is_remote: bool,
//     tauri_session: tauri::State<Mutex<TauriSession>>,
// ){
//     tauri_session.lock().unwrap().add_job(id, path, to, file_num, include_hidden, is_remote)
// }

// client::resume_job, - dependents?

// client::get_platform,
#[tauri::command(async)]
pub fn get_platform(is_remote: bool, tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_platform(is_remote)
}

// client::get_path_sep,
#[tauri::command(async)]
pub fn get_path_sep(is_remote: bool, tauri_session: tauri::State<Mutex<TauriSession>>) -> &'static str {
    tauri_session.lock().unwrap().get_path_sep(is_remote)
}

// client::get_icon_path,
#[tauri::command(async)]
pub fn get_icon_path(file_type: i32, ext: String, tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_icon_path(file_type, ext)
}

// client::get_char,
#[tauri::command(async)]
pub fn get_char(name: String, file_type: i32, tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_char(name, file_type)
}

// client::get_size,
#[tauri::command(async)]
pub fn get_session_size(tauri_session: tauri::State<Mutex<TauriSession>>) -> Vec<i32> {
    tauri_session.lock().unwrap().get_size()
}

// client::get_port_forwards,
#[tauri::command(async)]
pub fn get_port_forwards(tauri_session: tauri::State<Mutex<TauriSession>>) -> Vec<Vec<PortForwards>> {
    tauri_session.lock().unwrap().get_port_forwards()
}


// client::remove_port_forward,
#[tauri::command(async)]
pub fn remove_port_forward(
    id: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().remove_port_forward(id)
}

// client::get_args,
#[tauri::command(async)]
pub fn get_args(tauri_session: tauri::State<Mutex<TauriSession>>) -> Vec<String> {
    tauri_session.lock().unwrap().get_args()
}

// client::add_port_forward,
#[tauri::command(async)]
pub fn add_port_forward(
    id: i32,
    remote_host: String,
    remote_port: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().add_port_forward(id, remote_host, remote_port)
}

// client::save_size,
#[tauri::command(async)]
pub fn save_size(
    width: i32,
    height: i32,
    w: i32, 
    h: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().save_size(width, height, w, h)
}

// client::get_view_style,
#[tauri::command(async)]
pub fn get_view_style(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_view_style()
}

// client::get_image_quality,
#[tauri::command(async)]
pub fn get_image_quality(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_image_quality()
}

// client::get_custom_image_quality,
#[tauri::command(async)]
pub fn get_custom_image_quality(tauri_session: tauri::State<Mutex<TauriSession>>) -> Vec<i32> {
    tauri_session.lock().unwrap().get_custom_image_quality()
}

// client::save_view_style,
#[tauri::command(async)]
pub fn save_view_style(
    value: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().save_view_style(value)
}

// client::save_image_quality,
#[tauri::command(async)]
pub fn save_image_quality(
    value: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().save_image_quality(value)
}

// client::save_custom_image_quality,
#[tauri::command(async)]
pub fn save_custom_image_quality(
    custom_image_quality: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().save_custom_image_quality(custom_image_quality)
}

// client::refresh_video,
#[tauri::command(async)]
pub fn refresh_video(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().refresh_video()
}

// client::record_screen,
#[tauri::command(async)]
pub fn record_screen(
    start: bool, 
    w: i32, 
    h: i32,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().record_screen(start, w, h)
}

// client::get_toggle_option,
#[tauri::command(async)]
pub fn get_toggle_option(
    name: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
) -> bool {
    tauri_session.lock().unwrap().get_toggle_option(name)
}

// client::is_privacy_mode_supported,
#[tauri::command(async)]
pub fn is_privacy_mode_supported(tauri_session: tauri::State<Mutex<TauriSession>>) -> bool {
    tauri_session.lock().unwrap().is_privacy_mode_supported()
}

// client::toggle_option,
#[tauri::command(async)]
pub fn toggle_option(
    name: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().toggle_option(name)
}

// client::get_remember,
#[tauri::command(async)]
pub fn get_remember(tauri_session: tauri::State<Mutex<TauriSession>>) -> bool {
    tauri_session.lock().unwrap().get_remember()
}

// client::peer_platform,
#[tauri::command(async)]
pub fn peer_platform(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().peer_platform()
}

// client::set_write_override,
#[tauri::command(async)]
pub fn set_write_override(
    job_id: i32, 
    file_num: i32, 
    is_override: bool, 
    remember: bool, 
    is_upload: bool,
    tauri_session: tauri::State<Mutex<TauriSession>>,
) -> bool {
    tauri_session.lock().unwrap().set_write_override(job_id, file_num, is_override, remember, is_upload)
}

// client::get_keyboard_mode,
#[tauri::command(async)]
pub fn get_keyboard_mode(tauri_session: tauri::State<Mutex<TauriSession>>) -> String {
    tauri_session.lock().unwrap().get_keyboard_mode()
}

// client::save_keyboard_mode,
#[tauri::command(async)]
pub fn save_keyboard_mode(
    value: String,
    tauri_session: tauri::State<Mutex<TauriSession>>,
){
    tauri_session.lock().unwrap().save_keyboard_mode(value)
}

// client::supported_hwcodec,
#[tauri::command(async)]
pub fn supported_hwcodec(tauri_session: tauri::State<Mutex<TauriSession>>) -> (bool, bool) {
    tauri_session.lock().unwrap().supported_hwcodec()
}

// client::change_prefer_codec,
#[tauri::command(async)]
pub fn change_prefer_codec(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().change_prefer_codec()
}

// client::restart_remote_device,
#[tauri::command(async)]
pub fn restart_remote_device(tauri_session: tauri::State<Mutex<TauriSession>>){
    tauri_session.lock().unwrap().restart_remote_device()
}