use super::{CursorData, ResultType};
use crate::ipc;
use hbb_common::{
    allow_err, bail,
    config::{Config, APP_NAME},
    log, sleep, timeout, tokio,
};
use std::io::prelude::*;
use std::{
    ffi::OsString,
    io, mem,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use winapi::{
    shared::{minwindef::*, ntdef::NULL, windef::*},
    um::{
        errhandlingapi::GetLastError, handleapi::CloseHandle, minwinbase::STILL_ACTIVE,
        processthreadsapi::GetExitCodeProcess, winbase::*, wingdi::*, winnt::HANDLE, winuser::*,
    },
};
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
};

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        let mut out = mem::MaybeUninit::uninit().assume_init();
        if GetCursorPos(&mut out) == FALSE {
            return None;
        }
        return Some((out.x, out.y));
    }
}

pub fn reset_input_cache() {}

pub fn get_cursor() -> ResultType<Option<u64>> {
    unsafe {
        let mut ci: CURSORINFO = mem::MaybeUninit::uninit().assume_init();
        ci.cbSize = std::mem::size_of::<CURSORINFO>() as _;
        if GetCursorInfo(&mut ci) == FALSE {
            return Err(io::Error::last_os_error().into());
        }
        if ci.flags & CURSOR_SHOWING == 0 {
            Ok(None)
        } else {
            Ok(Some(ci.hCursor as _))
        }
    }
}

struct IconInfo(ICONINFO);

impl IconInfo {
    fn new(icon: HICON) -> ResultType<Self> {
        unsafe {
            let mut ii = mem::MaybeUninit::uninit().assume_init();
            if GetIconInfo(icon, &mut ii) == FALSE {
                Err(io::Error::last_os_error().into())
            } else {
                let ii = Self(ii);
                if ii.0.hbmMask.is_null() {
                    bail!("Cursor bitmap handle is NULL");
                }
                return Ok(ii);
            }
        }
    }

    fn is_color(&self) -> bool {
        !self.0.hbmColor.is_null()
    }
}

impl Drop for IconInfo {
    fn drop(&mut self) {
        unsafe {
            if !self.0.hbmColor.is_null() {
                DeleteObject(self.0.hbmColor as _);
            }
            if !self.0.hbmMask.is_null() {
                DeleteObject(self.0.hbmMask as _);
            }
        }
    }
}

// https://github.com/TurboVNC/tightvnc/blob/a235bae328c12fd1c3aed6f3f034a37a6ffbbd22/vnc_winsrc/winvnc/vncEncoder.cpp
// https://github.com/TigerVNC/tigervnc/blob/master/win/rfb_win32/DeviceFrameBuffer.cxx
pub fn get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
    unsafe {
        let mut ii = IconInfo::new(hcursor as _)?;
        let bm_mask = get_bitmap(ii.0.hbmMask)?;
        let mut width = bm_mask.bmWidth;
        let mut height = if ii.is_color() {
            bm_mask.bmHeight
        } else {
            bm_mask.bmHeight / 2
        };
        let cbits_size = width * height * 4;
        if cbits_size < 16 {
            bail!("Invalid icon: too small"); // solve some crash
        }
        let mut cbits: Vec<u8> = Vec::new();
        cbits.resize(cbits_size as _, 0);
        let mut mbits: Vec<u8> = Vec::new();
        mbits.resize((bm_mask.bmWidthBytes * bm_mask.bmHeight) as _, 0);
        let r = GetBitmapBits(ii.0.hbmMask, mbits.len() as _, mbits.as_mut_ptr() as _);
        if r == 0 {
            bail!("Failed to copy bitmap data");
        }
        if r != (mbits.len() as i32) {
            bail!(
                "Invalid mask cursor buffer size, got {} bytes, expected {}",
                r,
                mbits.len()
            );
        }
        let do_outline;
        if ii.is_color() {
            get_rich_cursor_data(ii.0.hbmColor, width, height, &mut cbits)?;
            do_outline = fix_cursor_mask(
                &mut mbits,
                &mut cbits,
                width as _,
                height as _,
                bm_mask.bmWidthBytes as _,
            );
        } else {
            do_outline = handleMask(
                cbits.as_mut_ptr(),
                mbits.as_ptr(),
                width,
                height,
                bm_mask.bmWidthBytes,
                bm_mask.bmHeight,
            ) > 0;
        }
        if do_outline {
            let mut outline = Vec::new();
            outline.resize(((width + 2) * (height + 2) * 4) as _, 0);
            drawOutline(outline.as_mut_ptr(), cbits.as_ptr(), width, height, outline.len() as _);
            cbits = outline;
            width += 2;
            height += 2;
            ii.0.xHotspot += 1;
            ii.0.yHotspot += 1;
        }

        Ok(CursorData {
            id: hcursor,
            colors: cbits,
            hotx: ii.0.xHotspot as _,
            hoty: ii.0.yHotspot as _,
            width: width as _,
            height: height as _,
            ..Default::default()
        })
    }
}

#[inline]
fn get_bitmap(handle: HBITMAP) -> ResultType<BITMAP> {
    unsafe {
        let mut bm: BITMAP = mem::zeroed();
        if GetObjectA(
            handle as _,
            std::mem::size_of::<BITMAP>() as _,
            &mut bm as *mut BITMAP as *mut _,
        ) == FALSE
        {
            return Err(io::Error::last_os_error().into());
        }
        if bm.bmPlanes != 1 {
            bail!("Unsupported multi-plane cursor");
        }
        if bm.bmBitsPixel != 1 {
            bail!("Unsupported cursor mask format");
        }
        Ok(bm)
    }
}

struct DC(HDC);

impl DC {
    fn new() -> ResultType<Self> {
        unsafe {
            let dc = GetDC(0 as _);
            if dc.is_null() {
                bail!("Failed to get a drawing context");
            }
            Ok(Self(dc))
        }
    }
}

impl Drop for DC {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                ReleaseDC(0 as _, self.0);
            }
        }
    }
}

struct CompatibleDC(HDC);

impl CompatibleDC {
    fn new(existing: HDC) -> ResultType<Self> {
        unsafe {
            let dc = CreateCompatibleDC(existing);
            if dc.is_null() {
                bail!("Failed to get a compatible drawing context");
            }
            Ok(Self(dc))
        }
    }
}

impl Drop for CompatibleDC {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                DeleteDC(self.0);
            }
        }
    }
}

struct BitmapDC(CompatibleDC, HBITMAP);

impl BitmapDC {
    fn new(hdc: HDC, hbitmap: HBITMAP) -> ResultType<Self> {
        unsafe {
            let dc = CompatibleDC::new(hdc)?;
            let oldbitmap = SelectObject(dc.0, hbitmap as _) as HBITMAP;
            if oldbitmap.is_null() {
                bail!("Failed to select CompatibleDC");
            }
            Ok(Self(dc, oldbitmap))
        }
    }

    fn dc(&self) -> HDC {
        (self.0).0
    }
}

impl Drop for BitmapDC {
    fn drop(&mut self) {
        unsafe {
            if !self.1.is_null() {
                SelectObject((self.0).0, self.1 as _);
            }
        }
    }
}

#[inline]
fn get_rich_cursor_data(
    hbm_color: HBITMAP,
    width: i32,
    height: i32,
    out: &mut Vec<u8>,
) -> ResultType<()> {
    unsafe {
        let dc = DC::new()?;
        let bitmap_dc = BitmapDC::new(dc.0, hbm_color)?;
        if get_di_bits(out.as_mut_ptr(), bitmap_dc.dc(), hbm_color, width, height) > 0 {
            bail!("Failed to get di bits: {}", get_error());
        }
    }
    Ok(())
}

fn fix_cursor_mask(
    mbits: &mut Vec<u8>,
    cbits: &mut Vec<u8>,
    width: usize,
    height: usize,
    bm_width_bytes: usize,
) -> bool {
    let mut pix_idx = 0;
    for _ in 0..height {
        for _ in 0..width {
            if cbits[pix_idx + 3] != 0 {
                return false;
            }
            pix_idx += 4;
        }
    }

    let packed_width_bytes = (width + 7) >> 3;
    let bm_size = mbits.len();
    let c_size = cbits.len();

    // Pack and invert bitmap data (mbits)
    // borrow from tigervnc
    for y in 0..height {
        for x in 0..packed_width_bytes {
            let a = y * packed_width_bytes + x;
            let b = y * bm_width_bytes + x;
            if a < bm_size && b < bm_size {
                mbits[a] = !mbits[b];
            }
        }
    }

    // Replace "inverted background" bits with black color to ensure
    // cross-platform interoperability. Not beautiful but necessary code.
    // borrow from tigervnc
    let bytes_row = width << 2;
    for y in 0..height {
        let mut bitmask: u8 = 0x80;
        for x in 0..width {
            let mask_idx = y * packed_width_bytes + (x >> 3);
            if mask_idx < bm_size {
                let pix_idx = y * bytes_row + (x << 2);
                if (mbits[mask_idx] & bitmask) == 0 {
                    for b1 in 0..4 {
                        let a = pix_idx + b1;
                        if a < c_size {
                            if cbits[a] != 0 {
                                mbits[mask_idx] ^= bitmask;
                                for b2 in b1..4 {
                                    let b = pix_idx + b2;
                                    if b < c_size {
                                        cbits[b] = 0x00;
                                    }
                                }
                                break;
                            }
                        }
                    }
                }
            }
            bitmask >>= 1;
            if bitmask == 0 {
                bitmask = 0x80;
            }
        }
    }

    // borrow from noVNC
    let mut pix_idx = 0;
    for y in 0..height {
        for x in 0..width {
            let mask_idx = y * packed_width_bytes + (x >> 3);
            let mut alpha = 255;
            if mask_idx < bm_size {
                if (mbits[mask_idx] << (x & 0x7)) & 0x80 == 0 {
                   alpha =  0;
                }
            }
            let a = cbits[pix_idx + 2];
            let b = cbits[pix_idx + 1];
            let c = cbits[pix_idx];
            cbits[pix_idx] = a;
            cbits[pix_idx + 1] = b;
            cbits[pix_idx + 2] = c;
            cbits[pix_idx + 3] = alpha;
            pix_idx += 4;
        }
    }
    return true;
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(arguments: Vec<OsString>) {
    if let Err(e) = run_service(arguments) {
        log::error!("run_service failed: {}", e);
    }
}

pub fn start_os_service() {
    if let Err(e) = windows_service::service_dispatcher::start(APP_NAME, ffi_service_main) {
        log::error!("start_service failed: {}", e);
    }
}

const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

extern "C" {
    fn LaunchProcessWin(cmd: *const u16, session_id: DWORD, as_user: BOOL) -> HANDLE;
    fn selectInputDesktop() -> BOOL;
    fn inputDesktopSelected() -> BOOL;
    fn handleMask(out: *mut u8, mask: *const u8, width: i32, height: i32, bmWidthBytes: i32, bmHeight: i32)
        -> i32;
    fn drawOutline(out: *mut u8, in_: *const u8, width: i32, height: i32, out_size: i32);
    fn get_di_bits(out: *mut u8, dc: HDC, hbmColor: HBITMAP, width: i32, height: i32) -> i32;
    fn blank_screen(v: BOOL);
}

extern "system" {
    fn BlockInput(v: BOOL) -> BOOL;
}

#[tokio::main(flavor = "current_thread")]
async fn run_service(_arguments: Vec<OsString>) -> ResultType<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        log::info!("Got service control event: {:?}", control_event);
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            ServiceControl::Stop => {
                send_close(crate::POSTFIX_SERVICE).ok();
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register system service event handler
    let status_handle = service_control_handler::register(APP_NAME, event_handler)?;

    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: SERVICE_TYPE,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        process_id: None,
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let mut session_id = unsafe { WTSGetActiveConsoleSessionId() };
    log::info!("session id {}", session_id);
    let mut h_process = launch_server(session_id, true).await.unwrap_or(NULL);
    let mut incoming = ipc::new_listener(crate::POSTFIX_SERVICE).await?;
    loop {
        let res = timeout(super::SERVICE_INTERVAL, incoming.next()).await;
        match res {
            Ok(res) => match res {
                Some(Ok(stream)) => {
                    let mut stream = ipc::Connection::new(stream);
                    if let Ok(Some(data)) = stream.next_timeout(1000).await {
                        match data {
                            ipc::Data::Close => {
                                log::info!("close received");
                                break;
                            }
                            ipc::Data::SAS => {
                                send_sas();
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Err(_) => {
                // timeout
                unsafe {
                    let tmp = WTSGetActiveConsoleSessionId();
                    if tmp == 0xFFFFFFFF {
                        continue;
                    }
                    let mut close_sent = false;
                    if tmp != session_id {
                        log::info!("session changed from {} to {}", session_id, tmp);
                        session_id = tmp;
                        send_close_async("").await.ok();
                        close_sent = true;
                    }
                    let mut exit_code: DWORD = 0;
                    if h_process.is_null()
                        || (GetExitCodeProcess(h_process, &mut exit_code) == TRUE
                            && exit_code != STILL_ACTIVE
                            && CloseHandle(h_process) == TRUE)
                    {
                        match launch_server(session_id, !close_sent).await {
                            Ok(ptr) => {
                                h_process = ptr;
                            }
                            Err(err) => {
                                log::error!("Failed to launch server: {}", err);
                            }
                        }
                    }
                }
            }
        }
    }

    if !h_process.is_null() {
        send_close_async("").await.ok();
        unsafe { CloseHandle(h_process) };
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

async fn launch_server(session_id: DWORD, close_first: bool) -> ResultType<HANDLE> {
    if close_first {
        // in case started some elsewhere
        send_close_async("").await.ok();
    }
    let cmd = format!(
        "\"{}\" --server",
        std::env::current_exe()?.to_str().unwrap_or("")
    );
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(&cmd)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    let h = unsafe { LaunchProcessWin(wstr, session_id, FALSE) };
    if h.is_null() {
        log::error!("Failed to luanch server: {}", get_error());
    }
    Ok(h)
}

pub fn run_as_user(arg: &str) -> ResultType<Option<std::process::Child>> {
    let cmd = format!(
        "\"{}\" {}",
        std::env::current_exe()?.to_str().unwrap_or(""),
        arg,
    );
    let session_id = unsafe { WTSGetActiveConsoleSessionId() };
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(&cmd)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    let h = unsafe { LaunchProcessWin(wstr, session_id, TRUE) };
    if h.is_null() {
        bail!(
            "Failed to launch {} with session id {}: {}",
            arg,
            session_id,
            get_error()
        );
    }
    Ok(None)
}

#[tokio::main(flavor = "current_thread")]
async fn send_close(postfix: &str) -> ResultType<()> {
    send_close_async(postfix).await
}

async fn send_close_async(postfix: &str) -> ResultType<()> {
    ipc::connect(1000, postfix)
        .await?
        .send(&ipc::Data::Close)
        .await?;
    // sleep a while to wait for closing and exit
    sleep(0.1).await;
    Ok(())
}

// https://docs.microsoft.com/en-us/windows/win32/api/sas/nf-sas-sendsas
// https://www.cnblogs.com/doutu/p/4892726.html
fn send_sas() {
    #[link(name = "sas")]
    extern "system" {
        pub fn SendSAS(AsUser: BOOL);
    }
    unsafe {
        log::info!("SAS received");
        SendSAS(FALSE);
    }
}

lazy_static::lazy_static! {
    static ref SUPRESS: Arc<Mutex<Instant>> = Arc::new(Mutex::new(Instant::now()));
}

pub fn desktop_changed() -> bool {
    unsafe { inputDesktopSelected() == FALSE }
}

pub fn try_change_desktop() -> bool {
    unsafe {
        if inputDesktopSelected() == FALSE {
            let res = selectInputDesktop() == TRUE;
            if !res {
                let mut s = SUPRESS.lock().unwrap();
                if s.elapsed() > std::time::Duration::from_secs(3) {
                    log::error!("Failed to switch desktop: {}", get_error());
                    *s = Instant::now();
                }
            } else {
                log::info!("Desktop switched");
            }
            return res;
        }
    }
    return false;
}

fn get_error() -> String {
    unsafe {
        let buff_size = 256;
        let mut buff: Vec<u16> = Vec::with_capacity(buff_size);
        buff.resize(buff_size, 0);
        let errno = GetLastError();
        let chars_copied = FormatMessageW(
            FORMAT_MESSAGE_IGNORE_INSERTS
                | FORMAT_MESSAGE_FROM_SYSTEM
                | FORMAT_MESSAGE_ARGUMENT_ARRAY,
            std::ptr::null(),
            errno,
            0,
            buff.as_mut_ptr(),
            (buff_size + 1) as u32,
            std::ptr::null_mut(),
        );
        if chars_copied == 0 {
            return "".to_owned();
        }
        let mut curr_char: usize = chars_copied as usize;
        while curr_char > 0 {
            let ch = buff[curr_char];

            if ch >= ' ' as u16 {
                break;
            }
            curr_char -= 1;
        }
        let sl = std::slice::from_raw_parts(buff.as_ptr(), curr_char);
        let err_msg = String::from_utf16(sl);
        return err_msg.unwrap_or("".to_owned());
    }
}

pub fn get_active_username() -> String {
    let name = crate::username();
    if name != "SYSTEM" {
        return name;
    }
    extern "C" {
        fn get_active_user(path: *mut u16, n: u32) -> u32;
    }
    let buff_size = 256;
    let mut buff: Vec<u16> = Vec::with_capacity(buff_size);
    buff.resize(buff_size, 0);
    let n = unsafe { get_active_user(buff.as_mut_ptr(), buff_size as _) };
    if n == 0 {
        return "".to_owned();
    }
    let sl = unsafe { std::slice::from_raw_parts(buff.as_ptr(), n as _) };
    String::from_utf16(sl)
        .unwrap_or("??".to_owned())
        .trim_end_matches('\0')
        .to_owned()
}

/*
pub fn get_active_username() -> String {
    use std::os::windows::process::CommandExt;
    let name = crate::username();
    if name != "SYSTEM" {
        return name;
    }
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = std::process::Command::new("query");
    cmd.arg("user");
    cmd.creation_flags(CREATE_NO_WINDOW);
    if let Ok(output) = cmd.output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(name) = line.split_whitespace().next() {
                if name.starts_with(">") {
                    return name.replace(">", "");
                }
            }
        }
    }
    return "".to_owned();
}
*/

pub fn is_prelogin() -> bool {
    let username = get_active_username();
    username.is_empty() || username == "SYSTEM"
}

pub fn is_root() -> bool {
    crate::username() == "SYSTEM"
}

pub fn lock_screen() {
    extern "system" {
        pub fn LockWorkStation() -> BOOL;
    }
    unsafe {
        LockWorkStation();
    }
}

pub fn get_install_info() -> (String, String, String, String) {
    let subkey = format!(
        "HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{}",
        APP_NAME
    );
    let mut pf = "C:\\Program Files".to_owned();
    if let Ok(output) = std::process::Command::new("echo")
        .arg("%ProgramFiles%")
        .output()
    {
        let tmp = String::from_utf8_lossy(&output.stdout);
        if !tmp.starts_with("%") {
            pf = tmp.to_string();
        }
    }
    let path = format!("{}\\{}", pf, APP_NAME);
    let start_menu = format!(
        "%ProgramData%\\Microsoft\\Windows\\Start Menu\\Programs\\{}",
        APP_NAME
    );
    let exe = format!("{}\\{}.exe", path, APP_NAME);
    (subkey, path, start_menu, exe)
}

pub fn update_me() -> ResultType<()> {
    let (_, _, _, exe) = get_install_info();
    let src_exe = std::env::current_exe()?.to_str().unwrap_or("").to_owned();
    let cmds = format!(
        "
        chcp 65001
        sc stop {app_name}
        taskkill /F /IM {app_name}.exe
        copy /Y \"{src_exe}\" \"{exe}\"
        sc start {app_name}
    ",
        src_exe = src_exe,
        exe = exe,
        app_name = APP_NAME,
    );
    std::thread::sleep(std::time::Duration::from_millis(1000));
    run_cmds(cmds, false)?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    std::process::Command::new(&exe).spawn()?;
    std::process::Command::new(&exe)
        .args(&["--remove", &src_exe])
        .spawn()?;
    Ok(())
}

pub fn install_me(options: &str) -> ResultType<()> {
    let (subkey, path, start_menu, exe) = get_install_info();
    let mut version_major = "0";
    let mut version_minor = "0";
    let mut version_build = "0";
    let versions: Vec<&str> = crate::VERSION.split(".").collect();
    if versions.len() > 0 {
        version_major = versions[0];
    }
    if versions.len() > 1 {
        version_minor = versions[1];
    }
    if versions.len() > 2 {
        version_build = versions[2];
    }

    let tmp_path = "C:\\Windows\\temp";
    let mk_shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\{app_name}.lnk\"

Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
oLink.Save
        ",
            tmp_path = tmp_path,
            app_name = APP_NAME,
            exe = exe,
        ),
        "vbs",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    // https://superuser.com/questions/392061/how-to-make-a-shortcut-from-cmd
    let uninstall_shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\Uninstall {app_name}.lnk\"
Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--uninstall\"
    oLink.IconLocation = \"msiexec.exe\"
oLink.Save
        ",
            tmp_path = tmp_path,
            app_name = APP_NAME,
            exe = exe,
        ),
        "vbs",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    let tray_shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
sLinkFile = \"{tmp_path}\\{app_name} Tray.lnk\"

Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--tray\"
oLink.Save
        ",
            tmp_path = tmp_path,
            app_name = APP_NAME,
            exe = exe,
        ),
        "vbs",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    let mut shortcuts = Default::default();
    if options.contains("desktopicon") {
        shortcuts = format!(
            "copy /Y \"{}\\{}.lnk\" \"%PUBLIC%\\Desktop\\\"",
            tmp_path, APP_NAME
        );
    }
    if options.contains("startmenu") {
        shortcuts = format!(
            "{}
md \"{start_menu}\"
copy /Y \"{tmp_path}\\{app_name}.lnk\" \"{start_menu}\\\"
copy /Y \"{tmp_path}\\Uninstall {app_name}.lnk\" \"{start_menu}\\\"
     ",
            shortcuts,
            start_menu = start_menu,
            tmp_path = tmp_path,
            app_name = APP_NAME
        );
    }

    let meta = std::fs::symlink_metadata(std::env::current_exe()?)?;
    let size = meta.len() / 1024;
    // save_tmp is for ensuring not copying file while writing
    let config_path = Config::save_tmp();
    let ext = APP_NAME.to_lowercase();
    // https://docs.microsoft.com/zh-cn/windows/win32/msi/uninstall-registry-key?redirectedfrom=MSDNa
    // https://www.windowscentral.com/how-edit-registry-using-command-prompt-windows-10
    // https://www.tenforums.com/tutorials/70903-add-remove-allowed-apps-through-windows-firewall-windows-10-a.html
    let cmds = format!(
        "
chcp 65001
md \"{path}\"
copy /Y \"{src_exe}\" \"{exe}\"
reg add {subkey} /f
reg add {subkey} /f /v DisplayIcon /t REG_SZ /d \"{exe}\"
reg add {subkey} /f /v DisplayName /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v Version /t REG_SZ /d \"{version}\"
reg add {subkey} /f /v InstallLocation /t REG_SZ /d \"{path}\"
reg add {subkey} /f /v Publisher /t REG_SZ /d \"{app_name}\"
reg add {subkey} /f /v VersionMajor /t REG_DWORD /d {major}
reg add {subkey} /f /v VersionMinor /t REG_DWORD /d {minor}
reg add {subkey} /f /v VersionBuild /t REG_DWORD /d {build}
reg add {subkey} /f /v UninstallString /t REG_SZ /d \"\\\"{exe}\\\" --uninstall\"
reg add {subkey} /f /v EstimatedSize /t REG_DWORD /d {size}
reg add {subkey} /f /v WindowsInstaller /t REG_DWORD /d 0
reg add HKEY_LOCAL_MACHINE\\Software\\Microsoft\\Windows\\CurrentVersion\\Policies\\System /f /v SoftwareSASGeneration /t REG_DWORD /d 1
\"{mk_shortcut}\"
\"{uninstall_shortcut}\"
\"{tray_shortcut}\"
copy /Y \"{tmp_path}\\{app_name} Tray.lnk\" \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\\"
{shortcuts}
del /f \"{mk_shortcut}\"
del /f \"{uninstall_shortcut}\"
del /f \"{tray_shortcut}\"
del /f \"{tmp_path}\\{app_name}.lnk\"
del /f \"{tmp_path}\\Uninstall {app_name}.lnk\"
del /f \"{tmp_path}\\{app_name} Tray.lnk\"
reg add HKEY_CLASSES_ROOT\\.{ext} /f
reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f
reg add HKEY_CLASSES_ROOT\\.{ext}\\DefaultIcon /f /ve /t REG_SZ  /d \"\\\"{exe}\\\",0\"
reg add HKEY_CLASSES_ROOT\\.{ext}\\shell /f
reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open /f
reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f
reg add HKEY_CLASSES_ROOT\\.{ext}\\shell\\open\\command /f /ve /t REG_SZ /d \"\\\"{exe}\\\" --play \\\"%%1\\\"\"
sc create {app_name} binpath= \"\\\"{exe}\\\" --import-config \\\"{config_path}\\\"\" start= auto DisplayName= \"{app_name} Service\"
sc start {app_name}
sc stop {app_name}
sc delete {app_name}
sc create {app_name} binpath= \"\\\"{exe}\\\" --service\" start= auto DisplayName= \"{app_name} Service\"
netsh advfirewall firewall add rule name=\"{app_name} Service\" dir=in action=allow program=\"{exe}\" enable=yes
del /f \"{config_path}\"
del /f \"{config2_path}\"
sc start {app_name}
    ",
        path=path,
        src_exe=std::env::current_exe()?.to_str().unwrap_or(""),
        exe=exe,
        subkey=subkey,
        app_name=APP_NAME,
        version=crate::VERSION,
        major=version_major,
        minor=version_minor,
        build=version_build,
        size=size,
        mk_shortcut=mk_shortcut,
        uninstall_shortcut=uninstall_shortcut,
        tray_shortcut=tray_shortcut,
        tmp_path=tmp_path,
        shortcuts=shortcuts,
        config_path=config_path,
        config2_path=config_path.replace(".toml", "2.toml"),
        ext=ext,
    );
    run_cmds(cmds, false)?;
    std::thread::sleep(std::time::Duration::from_millis(2000));
    std::process::Command::new(&exe).spawn()?;
    std::process::Command::new(&exe).arg("--tray").spawn()?;
    std::thread::sleep(std::time::Duration::from_millis(1000));
    Ok(())
}

pub fn uninstall_me() -> ResultType<()> {
    let (subkey, path, start_menu, _) = get_install_info();
    let ext = APP_NAME.to_lowercase();
    let cmds = format!(
        "
chcp 65001
sc stop {app_name}
sc delete {app_name}
taskkill /F /IM {app_name}.exe
reg delete {subkey} /f
reg delete HKEY_CLASSES_ROOT\\.{ext} /f
rd /s /q \"{path}\"
rd /s /q \"{start_menu}\"
del /f /q \"%PUBLIC%\\Desktop\\{app_name}*\"
del /f /q \"C:\\ProgramData\\Microsoft\\Windows\\Start Menu\\Programs\\Startup\\{app_name} Tray.lnk\"
netsh advfirewall firewall delete rule name=\"{app_name} Service\"
    ",
        app_name = APP_NAME,
        path = path,
        subkey = subkey,
        start_menu = start_menu,
        ext = ext,
    );
    run_cmds(cmds, true)
}

fn write_cmds(cmds: String, ext: &str) -> ResultType<std::path::PathBuf> {
    let mut tmp = std::env::temp_dir();
    tmp.push(format!("{}_{:?}.{}", APP_NAME, cmds.as_ptr(), ext));
    let mut cmds = cmds;
    if ext == "cmd" {
        cmds = format!("{}\ndel /f \"{}\"", cmds, tmp.to_str().unwrap_or(""));
    }
    let mut file = std::fs::File::create(&tmp)?;
    // in case cmds mixed with \r\n and \n, make sure all ending with \r\n
    // in some windows, \r\n required for cmd file to run
    let cmds = cmds.replace("\r\n", "\n").replace("\n", "\r\n");
    file.write_all(cmds.as_bytes())?;
    file.sync_all()?;
    return Ok(tmp);
}

fn run_cmds(cmds: String, show: bool) -> ResultType<()> {
    let tmp = write_cmds(cmds, "cmd")?;
    let res = runas::Command::new(tmp.to_str().unwrap_or(""))
        .show(show)
        .force_prompt(true)
        .status();
    // double confirm delete, because below delete not work if program
    // exit immediately such as --uninstall
    allow_err!(std::fs::remove_file(tmp));
    let _ = res?;
    Ok(())
}

pub fn toggle_privacy_mode(v: bool) {
    let v = if v { TRUE } else { FALSE };
    unsafe {
        blank_screen(v);
        BlockInput(v);
    }
}

pub fn block_input(v: bool) {
    let v = if v { TRUE } else { FALSE };
    unsafe {
        BlockInput(v);
    }
}

pub fn add_recent_document(path: &str) {
    extern "C" {
        fn AddRecentDocument(path: *const u16);
    }
    use std::os::windows::ffi::OsStrExt;
    let wstr: Vec<u16> = std::ffi::OsStr::new(path)
        .encode_wide()
        .chain(Some(0).into_iter())
        .collect();
    let wstr = wstr.as_ptr();
    unsafe {
        AddRecentDocument(wstr);
    }
}

pub fn is_installed() -> bool {
    use windows_service::{
        service::ServiceAccess,
        service_manager::{ServiceManager, ServiceManagerAccess},
    };
    let (_, _, _, exe) = get_install_info();
    if !std::fs::metadata(exe).is_ok() {
        return false;
    }
    let manager_access = ServiceManagerAccess::CONNECT;
    if let Ok(service_manager) = ServiceManager::local_computer(None::<&str>, manager_access) {
        if let Ok(_) = service_manager.open_service(APP_NAME, ServiceAccess::QUERY_CONFIG) {
            return true;
        }
    }
    return false;
}

pub fn get_installed_version() -> String {
    let (_, _, _, exe) = get_install_info();
    if let Ok(output) = std::process::Command::new(exe).arg("--version").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            return line.to_owned();
        }
    }
    "".to_owned()
}

pub fn create_shortcut(id: &str) -> ResultType<()> {
    let exe = std::env::current_exe()?.to_str().unwrap_or("").to_owned();
    let shortcut = write_cmds(
        format!(
            "
Set oWS = WScript.CreateObject(\"WScript.Shell\")
strDesktop = oWS.SpecialFolders(\"Desktop\")
Set objFSO = CreateObject(\"Scripting.FileSystemObject\")
sLinkFile = objFSO.BuildPath(strDesktop, \"{id}.lnk\")
Set oLink = oWS.CreateShortcut(sLinkFile)
    oLink.TargetPath = \"{exe}\"
    oLink.Arguments = \"--connect {id}\"
oLink.Save
        ",
            exe = exe,
            id = id,
        ),
        "vbs",
    )?
    .to_str()
    .unwrap_or("")
    .to_owned();
    std::process::Command::new("cscript")
        .arg(&shortcut)
        .output()?;
    allow_err!(std::fs::remove_file(shortcut));
    Ok(())
}
