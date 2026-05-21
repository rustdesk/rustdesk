// https://developer.apple.com/documentation/appkit/nscursor
// https://github.com/servo/core-foundation-rs
// https://github.com/rust-windowing/winit

use super::{CursorData, ResultType};
use cocoa::{
    appkit::{NSApp, NSApplication, NSApplicationActivationPolicy::*},
    base::{id, nil, BOOL, NO, YES},
    foundation::{NSDictionary, NSPoint, NSSize, NSString},
};
use core_foundation::{
    array::{CFArrayGetCount, CFArrayGetValueAtIndex},
    dictionary::CFDictionaryRef,
    string::CFStringRef,
};
use core_graphics::{
    display::{kCGNullWindowID, kCGWindowListOptionOnScreenOnly, CGWindowListCopyWindowInfo},
    window::{kCGWindowName, kCGWindowOwnerPID},
};
use hbb_common::{
    anyhow::anyhow,
    bail, log,
    message_proto::{DisplayInfo, Resolution},
    sysinfo::{Pid, Process, ProcessRefreshKind, System},
};
use include_dir::{include_dir, Dir};
use objc::rc::autoreleasepool;
use objc::{class, msg_send, sel, sel_impl};
use scrap::{libc::c_void, quartz::ffi::*};
use std::{
    io::{Read, Seek, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
};

// macOS boolean_t is defined as `int` in <mach/boolean.h>
type BooleanT = hbb_common::libc::c_int;

static PRIVILEGES_SCRIPTS_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/platform/privileges_scripts");
static mut LATEST_SEED: i32 = 0;
static UPDATE_TEMP_DIR: OnceLock<PathBuf> = OnceLock::new();
const UPDATE_TEMP_DMG_CREATE_ATTEMPTS: usize = 16;
const UPDATE_DMG_MOUNT_POINT: &str = "/Volumes/RustDeskUpdate";

#[inline]
fn get_update_temp_dir() -> PathBuf {
    UPDATE_TEMP_DIR.get_or_init(new_update_temp_dir).clone()
}

fn new_update_temp_dir() -> PathBuf {
    let euid = unsafe { hbb_common::libc::geteuid() };
    Path::new("/tmp").join(format!(
        ".rustdeskupdate-{}-{}-{}",
        euid,
        std::process::id(),
        hbb_common::rand::random::<u64>()
    ))
}

#[inline]
fn get_update_temp_dir_string() -> String {
    get_update_temp_dir().to_string_lossy().into_owned()
}

fn get_update_temp_dmg_dir() -> PathBuf {
    get_update_temp_dir().join("dmgdir")
}

fn create_update_temp_dmg_file() -> ResultType<(std::fs::File, PathBuf)> {
    let update_temp_dir = get_update_temp_dir();
    std::fs::create_dir_all(&update_temp_dir)?;
    std::fs::set_permissions(&update_temp_dir, std::fs::Permissions::from_mode(0o700))?;

    let dmg_dir = update_temp_dir.join("dmgdir");
    std::fs::create_dir_all(&dmg_dir)?;
    std::fs::set_permissions(&dmg_dir, std::fs::Permissions::from_mode(0o700))?;

    for _ in 0..UPDATE_TEMP_DMG_CREATE_ATTEMPTS {
        let file_path = dmg_dir.join(format!(
            "{}-{}-{}.dmg",
            crate::get_app_name(),
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let file_res = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&file_path);
        match file_res {
            Ok(file) => {
                return Ok((file, file_path));
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
    }

    bail!("Failed to create update DMG file");
}

/// Global mutex to serialize CoreGraphics cursor operations.
/// This prevents race conditions between cursor visibility (hide depth tracking)
/// and cursor positioning/clipping operations.
static CG_CURSOR_MUTEX: Mutex<()> = Mutex::new(());

extern "C" {
    fn CGSCurrentCursorSeed() -> i32;
    fn CGEventCreate(r: *const c_void) -> *const c_void;
    fn CGEventGetLocation(e: *const c_void) -> CGPoint;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> BOOL;
    fn InputMonitoringAuthStatus(_: BOOL) -> BOOL;
    fn IsCanScreenRecording(_: BOOL) -> BOOL;
    fn CanUseNewApiForScreenCaptureCheck() -> BOOL;
    fn MacCheckAdminAuthorization() -> BOOL;
    fn MacGetModeNum(display: u32, numModes: *mut u32) -> BOOL;
    fn MacGetModes(
        display: u32,
        widths: *mut u32,
        heights: *mut u32,
        hidpis: *mut BOOL,
        max: u32,
        numModes: *mut u32,
    ) -> BOOL;
    fn majorVersion() -> u32;
    fn MacGetMode(display: u32, width: *mut u32, height: *mut u32) -> BOOL;
    fn MacSetMode(display: u32, width: u32, height: u32, tryHiDPI: bool) -> BOOL;
    fn CGWarpMouseCursorPosition(newCursorPosition: CGPoint) -> CGError;
    fn CGAssociateMouseAndMouseCursorPosition(connected: BooleanT) -> CGError;
}

pub fn major_version() -> u32 {
    unsafe { majorVersion() }
}

pub fn is_process_trusted(prompt: bool) -> bool {
    autoreleasepool(|| unsafe_is_process_trusted(prompt))
}

fn unsafe_is_process_trusted(prompt: bool) -> bool {
    unsafe {
        let value = if prompt { YES } else { NO };
        let value: id = msg_send![class!(NSNumber), numberWithBool: value];
        let options = NSDictionary::dictionaryWithObject_forKey_(
            nil,
            value,
            kAXTrustedCheckOptionPrompt as _,
        );
        AXIsProcessTrustedWithOptions(options as _) == YES
    }
}

pub fn is_can_input_monitoring(prompt: bool) -> bool {
    unsafe {
        let value = if prompt { YES } else { NO };
        InputMonitoringAuthStatus(value) == YES
    }
}

pub fn is_can_screen_recording(prompt: bool) -> bool {
    autoreleasepool(|| unsafe_is_can_screen_recording(prompt))
}

// macOS >= 10.15
// https://stackoverflow.com/questions/56597221/detecting-screen-recording-settings-on-macos-catalina/
// remove just one app from all the permissions: tccutil reset All com.carriez.rustdesk
fn unsafe_is_can_screen_recording(prompt: bool) -> bool {
    // we got some report that we show no permission even after set it, so we try to use new api for screen recording check
    // the new api is only available on macOS >= 10.15, but on stackoverflow, some people said it works on >= 10.16 (crash on 10.15),
    // but also some said it has bug on 10.16, so we just use it on 11.0.
    unsafe {
        if CanUseNewApiForScreenCaptureCheck() == YES {
            return IsCanScreenRecording(if prompt { YES } else { NO }) == YES;
        }
    }
    let mut can_record_screen: bool = false;
    unsafe {
        let our_pid: i32 = std::process::id() as _;
        let our_pid: id = msg_send![class!(NSNumber), numberWithInteger: our_pid];
        let window_list =
            CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID);
        let n = CFArrayGetCount(window_list);
        let dock = NSString::alloc(nil).init_str("Dock");
        for i in 0..n {
            let w: id = CFArrayGetValueAtIndex(window_list, i) as _;
            let name: id = msg_send![w, valueForKey: kCGWindowName as id];
            if name.is_null() {
                continue;
            }
            let pid: id = msg_send![w, valueForKey: kCGWindowOwnerPID as id];
            let is_me: BOOL = msg_send![pid, isEqual: our_pid];
            if is_me == YES {
                continue;
            }
            let pid: i32 = msg_send![pid, intValue];
            let p: id = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if p.is_null() {
                // ignore processes we don't have access to, such as WindowServer, which manages the windows named "Menubar" and "Backstop Menubar"
                continue;
            }
            let url: id = msg_send![p, executableURL];
            let exe_name: id = msg_send![url, lastPathComponent];
            if exe_name.is_null() {
                continue;
            }
            let is_dock: BOOL = msg_send![exe_name, isEqual: dock];
            if is_dock == YES {
                // ignore the Dock, which provides the desktop picture
                continue;
            }
            can_record_screen = true;
            break;
        }
    }
    if !can_record_screen && prompt {
        use scrap::{Capturer, Display};
        if let Ok(d) = Display::primary() {
            Capturer::new(d).ok();
        }
    }
    can_record_screen
}

pub fn install_service() -> bool {
    is_installed_daemon(false)
}

// Remember to check if `update_daemon_agent()` need to be changed if changing `is_installed_daemon()`.
// No need to merge the existing dup code, because the code in these two functions are too critical.
// New code should be written in a common function.
pub fn is_installed_daemon(prompt: bool) -> bool {
    let daemon = format!("{}_service.plist", crate::get_full_name());
    let agent = format!("{}_server.plist", crate::get_full_name());
    let agent_plist_file = format!("/Library/LaunchAgents/{}", agent);
    if !prompt {
        // in macos 13, there is new way to check if they are running or enabled, https://developer.apple.com/documentation/servicemanagement/updating-helper-executables-from-earlier-versions-of-macos#Respond-to-changes-in-System-Settings
        if !std::path::Path::new(&format!("/Library/LaunchDaemons/{}", daemon)).exists() {
            return false;
        }
        if !std::path::Path::new(&agent_plist_file).exists() {
            return false;
        }
        return true;
    }

    let Some(install_script) = PRIVILEGES_SCRIPTS_DIR.get_file("install.scpt") else {
        return false;
    };
    let Some(install_script_body) = install_script.contents_utf8().map(correct_app_name) else {
        return false;
    };

    let Some(daemon_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("daemon.plist") else {
        return false;
    };
    let Some(daemon_plist_body) = daemon_plist.contents_utf8().map(correct_app_name) else {
        return false;
    };

    let Some(agent_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("agent.plist") else {
        return false;
    };
    let Some(agent_plist_body) = agent_plist.contents_utf8().map(correct_app_name) else {
        return false;
    };

    std::thread::spawn(move || {
        match std::process::Command::new("osascript")
            .arg("-e")
            .arg(install_script_body)
            .arg(daemon_plist_body)
            .arg(agent_plist_body)
            .arg(&get_active_username())
            .status()
        {
            Err(e) => {
                log::error!("run osascript failed: {}", e);
            }
            _ => {
                let installed = std::path::Path::new(&agent_plist_file).exists();
                log::info!("Agent file {} installed: {}", agent_plist_file, installed);
                if installed {
                    log::info!("launch server");
                    std::process::Command::new("launchctl")
                        .args(&["load", "-w", &agent_plist_file])
                        .status()
                        .ok();
                }
            }
        }
    });
    false
}

fn update_daemon_agent(
    agent_plist_file: String,
    update_source_dir: String,
    sync: bool,
    keep_current_process: bool,
) -> ResultType<()> {
    let update_script_file = "update.scpt";
    let Some(update_script) = PRIVILEGES_SCRIPTS_DIR.get_file(update_script_file) else {
        bail!("Failed to find {}", update_script_file);
    };
    let Some(update_script_body) = update_script.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read {}", update_script_file);
    };

    let Some(daemon_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("daemon.plist") else {
        bail!("Failed to find daemon.plist");
    };
    let Some(daemon_plist_body) = daemon_plist.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read daemon.plist");
    };
    let Some(agent_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("agent.plist") else {
        bail!("Failed to find agent.plist");
    };
    let Some(agent_plist_body) = agent_plist.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read agent.plist");
    };

    let current_pid = current_pid_for_update_script(keep_current_process);
    let func = move || -> ResultType<()> {
        let mut binding = std::process::Command::new("osascript");
        let cmd = binding
            .arg("-e")
            .arg(update_script_body)
            .arg(daemon_plist_body)
            .arg(agent_plist_body)
            .arg(&get_active_username())
            .arg(&current_pid)
            .arg(update_source_dir);
        match cmd.output() {
            Err(e) => {
                log::error!("run osascript failed: {}", e);
                bail!("run osascript failed: {}", e);
            }
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::warn!(
                    "run osascript failed with status: {}, stderr: {}",
                    output.status,
                    stderr.trim()
                );
                bail!(
                    "run osascript failed with status: {}, stderr: {}",
                    output.status,
                    stderr.trim()
                );
            }
            _ => {
                let installed = std::path::Path::new(&agent_plist_file).exists();
                log::info!("Agent file {} installed: {}", &agent_plist_file, installed);
            }
        }
        Ok(())
    };
    if sync {
        func()
    } else {
        std::thread::spawn(move || {
            hbb_common::allow_err!(func());
        });
        Ok(())
    }
}

fn correct_app_name(s: &str) -> String {
    let mut s = s.to_owned();
    if let Some(bundleid) = get_bundle_id() {
        s = s.replace("com.carriez.rustdesk", &bundleid);
    }
    s = s.replace("rustdesk", &crate::get_app_name().to_lowercase());
    s = s.replace("RustDesk", &crate::get_app_name());
    s
}

pub fn uninstall_service(show_new_window: bool, sync: bool) -> bool {
    // to-do: do together with win/linux about refactory start/stop service
    if !is_installed_daemon(false) {
        return false;
    }

    let Some(script_file) = PRIVILEGES_SCRIPTS_DIR.get_file("uninstall.scpt") else {
        return false;
    };
    let Some(script_body) = script_file.contents_utf8().map(correct_app_name) else {
        return false;
    };

    let func = move || {
        match std::process::Command::new("osascript")
            .arg("-e")
            .arg(script_body)
            .status()
        {
            Err(e) => {
                log::error!("run osascript failed: {}", e);
            }
            _ => {
                let agent = format!("{}_server.plist", crate::get_full_name());
                let agent_plist_file = format!("/Library/LaunchAgents/{}", agent);
                let uninstalled = !std::path::Path::new(&agent_plist_file).exists();
                log::info!(
                    "Agent file {} uninstalled: {}",
                    agent_plist_file,
                    uninstalled
                );
                if uninstalled {
                    if !show_new_window {
                        let _ = crate::ipc::close_all_instances();
                        // leave ipc a little time
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                    crate::ipc::set_option("stop-service", "Y");
                    std::process::Command::new("launchctl")
                        .args(&["remove", &format!("{}_server", crate::get_full_name())])
                        .status()
                        .ok();
                    if show_new_window {
                        std::process::Command::new("open")
                            .arg("-n")
                            .arg(&format!("/Applications/{}.app", crate::get_app_name()))
                            .spawn()
                            .ok();
                        // leave open a little time
                        std::thread::sleep(std::time::Duration::from_millis(300));
                    }
                    quit_gui();
                }
            }
        }
    };
    if sync {
        func();
    } else {
        std::thread::spawn(func);
    }
    true
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    unsafe {
        let e = CGEventCreate(0 as _);
        let point = CGEventGetLocation(e);
        CFRelease(e);
        Some((point.x as _, point.y as _))
    }
    /*
    let mut pt: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
    let screen: id = unsafe { msg_send![class!(NSScreen), currentScreenForMouseLocation] };
    let frame: NSRect = unsafe { msg_send![screen, frame] };
    pt.x -= frame.origin.x;
    pt.y -= frame.origin.y;
    Some((pt.x as _, pt.y as _))
    */
}

/// Warp the mouse cursor to the specified screen position.
///
/// # Thread Safety
/// This function affects global cursor state and acquires `CG_CURSOR_MUTEX`.
/// Callers must ensure no nested calls occur while the mutex is held.
///
/// # Arguments
/// * `x` - X coordinate in screen points (macOS uses points, not pixels)
/// * `y` - Y coordinate in screen points
pub fn set_cursor_pos(x: i32, y: i32) -> bool {
    // Acquire lock with deadlock detection in debug builds.
    // In debug builds, try_lock detects re-entrant calls early; on failure we return immediately.
    // In release builds, we use blocking lock() which will wait if contended.
    #[cfg(debug_assertions)]
    let _guard = match CG_CURSOR_MUTEX.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            log::error!(
                "[BUG] set_cursor_pos: CG_CURSOR_MUTEX is already held - potential deadlock!"
            );
            debug_assert!(false, "Re-entrant call to set_cursor_pos detected");
            return false;
        }
        Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
    };
    #[cfg(not(debug_assertions))]
    let _guard = CG_CURSOR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let result = CGWarpMouseCursorPosition(CGPoint {
            x: x as f64,
            y: y as f64,
        });
        if result != CGError::Success {
            log::error!(
                "CGWarpMouseCursorPosition({}, {}) returned error: {:?}",
                x,
                y,
                result
            );
        }
        result == CGError::Success
    }
}

/// Toggle pointer lock (dissociate/associate mouse from cursor position).
///
/// On macOS, cursor clipping is not supported directly like Windows ClipCursor.
/// Instead, we use CGAssociateMouseAndMouseCursorPosition to dissociate mouse
/// movement from cursor position, achieving a "pointer lock" effect.
///
/// # Thread Safety
/// This function affects global cursor state and acquires `CG_CURSOR_MUTEX`.
/// Callers must ensure only one owner toggles pointer lock at a time;
/// nested Some/None transitions from different call sites may cause unexpected behavior.
///
/// # Arguments
/// * `rect` - When `Some(_)`, dissociates mouse from cursor (enables pointer lock).
///            When `None`, re-associates mouse with cursor (disables pointer lock).
///            The rect coordinate values are ignored on macOS; only `Some`/`None` matters.
///            The parameter signature matches Windows for API consistency.
pub fn clip_cursor(rect: Option<(i32, i32, i32, i32)>) -> bool {
    // Acquire lock with deadlock detection in debug builds.
    // In debug builds, try_lock detects re-entrant calls early; on failure we return immediately.
    // In release builds, we use blocking lock() which will wait if contended.
    #[cfg(debug_assertions)]
    let _guard = match CG_CURSOR_MUTEX.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            log::error!("[BUG] clip_cursor: CG_CURSOR_MUTEX is already held - potential deadlock!");
            debug_assert!(false, "Re-entrant call to clip_cursor detected");
            return false;
        }
        Err(std::sync::TryLockError::Poisoned(e)) => e.into_inner(),
    };
    #[cfg(not(debug_assertions))]
    let _guard = CG_CURSOR_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    // CGAssociateMouseAndMouseCursorPosition takes a boolean_t:
    //   1 (true)  = associate mouse with cursor position (normal mode)
    //   0 (false) = dissociate mouse from cursor position (pointer lock mode)
    // When rect is Some, we want pointer lock (dissociate), so associate = false (0).
    // When rect is None, we want normal mode (associate), so associate = true (1).
    let associate: BooleanT = if rect.is_some() { 0 } else { 1 };
    unsafe {
        let result = CGAssociateMouseAndMouseCursorPosition(associate);
        if result != CGError::Success {
            log::warn!(
                "CGAssociateMouseAndMouseCursorPosition({}) returned error: {:?}",
                associate,
                result
            );
        }
        result == CGError::Success
    }
}

pub fn get_focused_display(displays: Vec<DisplayInfo>) -> Option<usize> {
    autoreleasepool(|| unsafe_get_focused_display(displays))
}

fn unsafe_get_focused_display(displays: Vec<DisplayInfo>) -> Option<usize> {
    unsafe {
        let main_screen: id = msg_send![class!(NSScreen), mainScreen];
        let screen: id = msg_send![main_screen, deviceDescription];
        let id: id =
            msg_send![screen, objectForKey: NSString::alloc(nil).init_str("NSScreenNumber")];
        let display_name: u32 = msg_send![id, unsignedIntValue];

        displays
            .iter()
            .position(|d| d.name == display_name.to_string())
    }
}

pub fn get_cursor() -> ResultType<Option<u64>> {
    autoreleasepool(|| unsafe_get_cursor())
}

fn unsafe_get_cursor() -> ResultType<Option<u64>> {
    unsafe {
        let seed = CGSCurrentCursorSeed();
        if seed == LATEST_SEED {
            return Ok(None);
        }
        LATEST_SEED = seed;
    }
    let c = get_cursor_id()?;
    Ok(Some(c.1))
}

pub fn reset_input_cache() {
    unsafe {
        LATEST_SEED = 0;
    }
}

fn get_cursor_id() -> ResultType<(id, u64)> {
    unsafe {
        let c: id = msg_send![class!(NSCursor), currentSystemCursor];
        if c == nil {
            bail!("Failed to call [NSCursor currentSystemCursor]");
        }
        let hotspot: NSPoint = msg_send![c, hotSpot];
        let img: id = msg_send![c, image];
        if img == nil {
            bail!("Failed to call [NSCursor image]");
        }
        let size: NSSize = msg_send![img, size];
        let tif: id = msg_send![img, TIFFRepresentation];
        if tif == nil {
            bail!("Failed to call [NSImage TIFFRepresentation]");
        }
        let rep: id = msg_send![class!(NSBitmapImageRep), imageRepWithData: tif];
        if rep == nil {
            bail!("Failed to call [NSBitmapImageRep imageRepWithData]");
        }
        let rep_size: NSSize = msg_send![rep, size];
        let mut hcursor =
            size.width + size.height + hotspot.x + hotspot.y + rep_size.width + rep_size.height;
        let x = (rep_size.width * hotspot.x / size.width) as usize;
        let y = (rep_size.height * hotspot.y / size.height) as usize;
        for i in 0..2 {
            let mut x2 = x + i;
            if x2 >= rep_size.width as usize {
                x2 = rep_size.width as usize - 1;
            }
            let mut y2 = y + i;
            if y2 >= rep_size.height as usize {
                y2 = rep_size.height as usize - 1;
            }
            let color: id = msg_send![rep, colorAtX:x2 y:y2];
            if color != nil {
                let r: f64 = msg_send![color, redComponent];
                let g: f64 = msg_send![color, greenComponent];
                let b: f64 = msg_send![color, blueComponent];
                let a: f64 = msg_send![color, alphaComponent];
                hcursor += (r + g + b + a) * (255 << i) as f64;
            }
        }
        Ok((c, hcursor as _))
    }
}

pub fn get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
    autoreleasepool(|| unsafe_get_cursor_data(hcursor))
}

// https://github.com/stweil/OSXvnc/blob/master/OSXvnc-server/mousecursor.c
fn unsafe_get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
    unsafe {
        let (c, hcursor2) = get_cursor_id()?;
        if hcursor != hcursor2 {
            bail!("cursor changed");
        }
        let hotspot: NSPoint = msg_send![c, hotSpot];
        let img: id = msg_send![c, image];
        let size: NSSize = msg_send![img, size];
        let reps: id = msg_send![img, representations];
        if reps == nil {
            bail!("Failed to call [NSImage representations]");
        }
        let nreps: usize = msg_send![reps, count];
        if nreps == 0 {
            bail!("Get empty [NSImage representations]");
        }
        let rep: id = msg_send![reps, objectAtIndex: 0];
        /*
        let n: id = msg_send![class!(NSNumber), numberWithFloat:1.0];
        let props: id = msg_send![class!(NSDictionary), dictionaryWithObject:n forKey:NSString::alloc(nil).init_str("NSImageCompressionFactor")];
        let image_data: id = msg_send![rep, representationUsingType:2 properties:props];
        let () = msg_send![image_data, writeToFile:NSString::alloc(nil).init_str("cursor.jpg") atomically:0];
        */
        let mut colors: Vec<u8> = Vec::new();
        colors.reserve((size.height * size.width) as usize * 4);
        // TIFF is rgb colorspace, no need to convert
        // let cs: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        for y in 0..(size.height as _) {
            for x in 0..(size.width as _) {
                let color: id = msg_send![rep, colorAtX:x as cocoa::foundation::NSInteger y:y as cocoa::foundation::NSInteger];
                // let color: id = msg_send![color, colorUsingColorSpace: cs];
                if color == nil {
                    continue;
                }
                let r: f64 = msg_send![color, redComponent];
                let g: f64 = msg_send![color, greenComponent];
                let b: f64 = msg_send![color, blueComponent];
                let a: f64 = msg_send![color, alphaComponent];
                colors.push((r * 255.) as _);
                colors.push((g * 255.) as _);
                colors.push((b * 255.) as _);
                colors.push((a * 255.) as _);
            }
        }
        Ok(CursorData {
            id: hcursor,
            colors: colors.into(),
            hotx: hotspot.x as _,
            hoty: hotspot.y as _,
            width: size.width as _,
            height: size.height as _,
            ..Default::default()
        })
    }
}

fn get_active_user(t: &str) -> String {
    if let Ok(output) = std::process::Command::new("ls")
        .args(vec![t, "/dev/console"])
        .output()
    {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(n) = line.split_whitespace().nth(2) {
                return n.to_owned();
            }
        }
    }
    "".to_owned()
}

pub fn get_active_username() -> String {
    get_active_user("-l")
}

pub fn get_active_userid() -> String {
    get_active_user("-n")
}

pub fn get_active_user_home() -> Option<PathBuf> {
    let username = get_active_username();
    if !username.is_empty() {
        let home = PathBuf::from(format!("/Users/{}", username));
        if home.exists() {
            return Some(home);
        }
    }
    None
}

pub fn is_prelogin() -> bool {
    get_active_userid() == "0"
}

// https://stackoverflow.com/questions/11505255/osx-check-if-the-screen-is-locked
// No "CGSSessionScreenIsLocked" can be found when macOS is not locked.
//
// `ioreg -n Root -d1` returns `"CGSSessionScreenIsLocked"=Yes`
// `ioreg -n Root -d1 -a` returns
// ```
// ...
//    <key>CGSSessionScreenIsLocked</key>
//    <true/>
// ...
// ```
pub fn is_locked() -> bool {
    match std::process::Command::new("ioreg")
        .arg("-n")
        .arg("Root")
        .arg("-d1")
        .output()
    {
        Ok(output) => {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Although `"CGSSessionScreenIsLocked"=Yes` was printed on my macOS,
            // I also check `"CGSSessionScreenIsLocked"=true` for better compability.
            output_str.contains("\"CGSSessionScreenIsLocked\"=Yes")
                || output_str.contains("\"CGSSessionScreenIsLocked\"=true")
        }
        Err(e) => {
            log::error!("Failed to query ioreg for the lock state: {}", e);
            false
        }
    }
}

pub fn is_root() -> bool {
    crate::username() == "root"
}

pub fn run_as_user(arg: Vec<&str>) -> ResultType<Option<std::process::Child>> {
    let uid = get_active_userid();
    let cmd = std::env::current_exe()?;
    let mut args = vec!["asuser", &uid, cmd.to_str().unwrap_or("")];
    args.append(&mut arg.clone());
    let task = std::process::Command::new("launchctl").args(args).spawn()?;
    Ok(Some(task))
}

pub fn lock_screen() {
    std::process::Command::new(
        "/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession",
    )
    .arg("-suspend")
    .output()
    .ok();
}

pub fn start_os_service() {
    log::info!("Username: {}", crate::username());
    if let Err(err) = crate::ipc::start("_service") {
        log::error!("Failed to start ipc_service: {}", err);
    }

    /* // mouse/keyboard works in prelogin now with launchctl asuser.
       // below can avoid multi-users logged in problem, but having its own below problem.
       // Not find a good way to start --cm without root privilege (affect file transfer).
       // one way is to start with `launchctl asuser <uid> open -n -a /Applications/RustDesk.app/ --args --cm`,
       // this way --cm is started with the user privilege, but we will have problem to start another RustDesk.app
       // with open in explorer.
        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        };
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();
        let mut uid = "".to_owned();
        let mut server: Option<std::process::Child> = None;
        if let Err(err) = ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        }) {
            println!("Failed to set Ctrl-C handler: {}", err);
        }
        while running.load(Ordering::SeqCst) {
            let tmp = get_active_userid();
            let mut start_new = false;
            if tmp != uid && !tmp.is_empty() {
                uid = tmp;
                log::info!("active uid: {}", uid);
                if let Some(ps) = server.as_mut() {
                    hbb_common::allow_err!(ps.kill());
                }
            }
            if let Some(ps) = server.as_mut() {
                match ps.try_wait() {
                    Ok(Some(_)) => {
                        server = None;
                        start_new = true;
                    }
                    _ => {}
                }
            } else {
                start_new = true;
            }
            if start_new {
                match run_as_user("--server") {
                    Ok(Some(ps)) => server = Some(ps),
                    Err(err) => {
                        log::error!("Failed to start server: {}", err);
                    }
                    _ => { /*no happen*/ }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(super::SERVICE_INTERVAL));
        }

        if let Some(ps) = server.take().as_mut() {
            hbb_common::allow_err!(ps.kill());
        }
        log::info!("Exit");
    */
}

pub fn toggle_blank_screen(_v: bool) {
    // https://unix.stackexchange.com/questions/17115/disable-keyboard-mouse-temporarily
}

pub fn block_input(_v: bool) -> (bool, String) {
    (true, "".to_owned())
}

pub fn is_installed() -> bool {
    if let Ok(p) = std::env::current_exe() {
        return p
            .to_str()
            .unwrap_or_default()
            .starts_with(&format!("/Applications/{}.app", crate::get_app_name()));
    }
    false
}

pub fn quit_gui() {
    unsafe {
        let () = msg_send!(NSApp(), terminate: nil);
    };
}

#[inline]
pub fn try_remove_temp_update_dir(dir: Option<&str>) {
    if let Some(dir) = dir {
        remove_temp_update_dir(Path::new(dir));
    } else {
        // `None` is startup stale-dir cleanup. Concurrent local updates are not supported here.
        remove_current_user_temp_update_dirs();
    }
}

fn remove_temp_update_dir(path: &Path) {
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.raw_os_error() == Some(hbb_common::libc::ENOTDIR) => {}
        Err(e) => {
            log::warn!("Failed to remove update temp dir {}: {}", path.display(), e);
        }
    }
}

fn remove_current_user_temp_update_dirs() {
    let Ok(entries) = std::fs::read_dir("/tmp") else {
        return;
    };
    let current_temp_dir = get_update_temp_dir();
    for entry in entries.flatten() {
        let path = entry.path();
        if path == current_temp_dir {
            continue;
        }
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            continue;
        };
        if is_current_user_update_temp_dir_name(file_name) {
            remove_temp_update_dir(&path);
        }
    }
}

fn is_current_user_update_temp_dir_name(file_name: &str) -> bool {
    let euid = unsafe { hbb_common::libc::geteuid() };
    let prefix = format!(".rustdeskupdate-{}", euid);
    file_name == prefix || file_name.starts_with(&format!("{}-", prefix))
}

pub fn update_me() -> ResultType<()> {
    let cmd = std::env::current_exe()?;
    // RustDesk.app/Contents/MacOS/RustDesk
    let app_dir = cmd
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|d| d.to_string_lossy().to_string());
    let Some(app_dir) = app_dir else {
        bail!("Unknown app directory of current exe file: {:?}", cmd);
    };
    update_me_from_app_dir(app_dir, true)
}

fn current_pid_for_update_script(keep_current_process: bool) -> String {
    if keep_current_process {
        std::process::id().to_string()
    } else {
        "0".to_owned()
    }
}

fn update_me_from_app_dir(app_dir: String, keep_current_process: bool) -> ResultType<()> {
    let is_installed_daemon = is_installed_daemon(false);
    let option_stop_service = "stop-service";
    let is_service_stopped = hbb_common::config::option2bool(
        option_stop_service,
        &crate::ui_interface::get_option(option_stop_service),
    );

    let app_name = crate::get_app_name();
    if is_installed_daemon && !is_service_stopped {
        let agent = format!("{}_server.plist", crate::get_full_name());
        let agent_plist_file = format!("/Library/LaunchAgents/{}", agent);
        update_daemon_agent(agent_plist_file, app_dir, true, keep_current_process)?;
    } else {
        // `kill -9` may not work without "administrator privileges"
        let update_body = r#"
	on run {app_name, cur_pid, app_dir, user_name, restore_owner}
	    set app_bundle to "/Applications/" & app_name & ".app"
	    set app_bundle_q to quoted form of app_bundle
	    set app_dir_q to quoted form of app_dir
	    set user_name_q to quoted form of user_name

	    set check_source to "test -d " & app_dir_q & " || exit 1;"
	    set kill_others to "pids=$(pgrep -x '" & app_name & "' | grep -vx " & cur_pid & " || true); if [ -n \"$pids\" ]; then echo \"$pids\" | xargs kill -9 || true; fi;"
	    set trusted_signer to "trusted_signer() { codesign --verify --deep --strict \"$1\"; };"
	    set verify_source to "trusted_signer " & app_dir_q & ";"
	    set prepare_verified to "verified_dir=$(mktemp -d /tmp/.rustdeskupdate-verified.XXXXXX); verified_app=\"$verified_dir/" & app_name & ".app\"; ditto " & app_dir_q & " \"$verified_app\" && chown -R root:wheel \"$verified_app\" && chmod -R go-w \"$verified_app\" && trusted_signer \"$verified_app\";"
	    set prepare_swap_paths to "temp_bundle=" & app_bundle_q & ".new.$$; old_bundle=" & app_bundle_q & ".old.$$;"
	    set cleanup_swap_paths to "rm -rf \"$temp_bundle\" \"$old_bundle\";"
	    set stage_bundle to "ditto \"$verified_app\" \"$temp_bundle\";"
	    set protect_staged_bundle to "chown -R root:wheel \"$temp_bundle\"; chmod -R go-w \"$temp_bundle\"; (xattr -r -d com.apple.quarantine \"$temp_bundle\" || true); trusted_signer \"$temp_bundle\";"
	    set move_current_bundle to "if [ -e " & app_bundle_q & " ]; then mv " & app_bundle_q & " \"$old_bundle\"; fi;"
	    set install_staged_bundle to "if mv \"$temp_bundle\" " & app_bundle_q & "; then rm -rf \"$old_bundle\"; else if [ -e \"$old_bundle\" ]; then mv \"$old_bundle\" " & app_bundle_q & "; fi; exit 1; fi;"
	    set restore_installed_owner to "if [ " & quoted form of restore_owner & " = '1' ]; then chown -R " & user_name_q & ":staff " & app_bundle_q & "; fi; trusted_signer " & app_bundle_q & ";"
	    set copy_files to prepare_swap_paths & cleanup_swap_paths & stage_bundle & protect_staged_bundle & move_current_bundle & install_staged_bundle & restore_installed_owner
	    set cleanup_verified to "if [ -n \"${temp_bundle:-}\" ]; then rm -rf \"$temp_bundle\"; fi; if [ -n \"${verified_dir:-}\" ]; then rm -rf \"$verified_dir\"; fi;"
	    set sh to "set -e; trap " & quoted form of cleanup_verified & " EXIT;" & trusted_signer & check_source & verify_source & kill_others & prepare_verified & copy_files

	    do shell script sh with prompt app_name & " wants to update itself" with administrator privileges
	end run
	        "#;
        let output = Command::new("osascript")
            .arg("-e")
            .arg(update_body)
            .arg(app_name.to_string())
            .arg(current_pid_for_update_script(keep_current_process))
            .arg(app_dir)
            .arg(get_active_username())
            .arg(if is_installed_daemon { "0" } else { "1" })
            .output();
        match output {
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!(
                    "osascript execution failed with status: {}, stderr: {}",
                    output.status,
                    stderr.trim()
                );
                bail!(
                    "osascript execution failed with status: {}, stderr: {}",
                    output.status,
                    stderr.trim()
                );
            }
            Err(e) => {
                log::error!("run osascript failed: {}", e);
                bail!("run osascript failed: {}", e);
            }
            _ => {}
        }
    }
    std::process::Command::new("open")
        .arg("-n")
        .arg(&format!("/Applications/{}.app", app_name))
        .spawn()
        .ok();
    // leave open a little time
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

pub fn update_from_dmg(dmg_path: &str) -> ResultType<()> {
    let update_temp_dir = get_update_temp_dir_string();
    println!("Starting update from DMG: {}", dmg_path);
    extract_dmg(dmg_path, &update_temp_dir)?;
    println!("DMG extracted");
    update_extracted(&update_temp_dir)?;
    println!("Update process started");
    Ok(())
}

pub fn update_to_verified_dmg(
    file: &str,
    expected_sha256: &str,
    before_prompt: Option<fn()>,
) -> ResultType<()> {
    let update_temp_dir = get_update_temp_dir_string();
    let update_res = update_from_verified_dmg(file, expected_sha256, before_prompt);
    try_remove_temp_update_dir(Some(&update_temp_dir));
    update_res?;
    quit_gui();
    Ok(())
}

fn update_from_verified_dmg(
    dmg_path: &str,
    expected_sha256: &str,
    before_prompt: Option<fn()>,
) -> ResultType<()> {
    let (mut dmg_file, temp_dmg_path) = copy_dmg_to_update_temp_file(dmg_path)?;
    verify_dmg_file_sha256(&mut dmg_file, expected_sha256, dmg_path)?;
    update_from_mounted_dmg(&temp_dmg_path.to_string_lossy(), before_prompt)
}

fn copy_dmg_to_update_temp_file(dmg_path: &str) -> ResultType<(std::fs::File, PathBuf)> {
    let mut source_file = std::fs::File::open(dmg_path)?;
    let (mut dmg_file, file_path) = create_update_temp_dmg_file()?;
    std::io::copy(&mut source_file, &mut dmg_file)?;
    dmg_file.flush()?;
    dmg_file.seek(std::io::SeekFrom::Start(0))?;
    Ok((dmg_file, file_path))
}

fn verify_dmg_file_sha256(
    dmg_file: &mut std::fs::File,
    expected_sha256: &str,
    display_path: &str,
) -> ResultType<()> {
    let expected_sha256 = expected_sha256.trim();
    if expected_sha256.len() != 64 || !expected_sha256.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("Expected DMG SHA256 is malformed for {}", display_path);
    }

    dmg_file.seek(std::io::SeekFrom::Start(0))?;
    let mut hasher = sha2::Sha256::default();
    let mut buffer = [0_u8; 8192];
    loop {
        let count = dmg_file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        sha2::Digest::update(&mut hasher, &buffer[..count]);
    }

    let actual_sha256 = format!("{:x}", sha2::Digest::finalize(hasher));
    if actual_sha256 != expected_sha256.to_ascii_lowercase() {
        bail!(
            "SHA256 mismatch for {}: expected {}, got {}",
            display_path,
            expected_sha256,
            actual_sha256
        );
    }
    Ok(())
}

struct DmgGuard(&'static str);

impl Drop for DmgGuard {
    fn drop(&mut self) {
        let _ = Command::new("hdiutil")
            .args(&["detach", self.0, "-force"])
            .status();
    }
}

fn attach_dmg_failure_message(
    dmg_path: &str,
    mount_point: &str,
    status: impl std::fmt::Display,
) -> String {
    format!(
        "Failed to attach DMG image at {dmg_path}: {status}. A stale mount at {mount_point} may remain from a previous update; detach it with `hdiutil detach {mount_point}` or restart and retry."
    )
}

fn attach_dmg(dmg_path: &str, mount_point: &'static str) -> ResultType<DmgGuard> {
    let status = Command::new("hdiutil")
        .args(&["attach", "-nobrowse", "-mountpoint", mount_point, dmg_path])
        .status()?;

    if !status.success() {
        bail!(
            "{}",
            attach_dmg_failure_message(dmg_path, mount_point, status)
        );
    }

    Ok(DmgGuard(mount_point))
}

fn app_path_in_dmg_mount(mount_point: &str, app_name: &str) -> String {
    format!("{}/{}.app", mount_point, app_name)
}

fn update_from_mounted_dmg(dmg_path: &str, before_prompt: Option<fn()>) -> ResultType<()> {
    let _guard = attach_dmg(dmg_path, UPDATE_DMG_MOUNT_POINT)?;
    if let Some(before_prompt) = before_prompt {
        before_prompt();
    }
    update_me_from_app_dir(
        app_path_in_dmg_mount(UPDATE_DMG_MOUNT_POINT, &crate::get_app_name()),
        true,
    )
}

fn extract_dmg(dmg_path: &str, target_dir: &str) -> ResultType<()> {
    let target_path = Path::new(target_dir);

    if target_path.exists() {
        std::fs::remove_dir_all(target_path)?;
    }
    std::fs::create_dir_all(target_path)?;

    let _guard = attach_dmg(dmg_path, UPDATE_DMG_MOUNT_POINT)?;

    let app_name = format!("{}.app", crate::get_app_name());
    let src_path = format!("{}/{}", UPDATE_DMG_MOUNT_POINT, app_name);
    let dest_path = format!("{}/{}", target_dir, app_name);

    let copy_status = Command::new("ditto")
        .args(&[&src_path, &dest_path])
        .status()?;

    if !copy_status.success() {
        bail!(
            "Failed to copy application from {} to {}: {:?}",
            src_path,
            dest_path,
            copy_status
        );
    }

    if !Path::new(&dest_path).exists() {
        bail!(
            "Copy operation failed - destination not found at {}",
            dest_path
        );
    }

    Ok(())
}

fn update_extracted(target_dir: &str) -> ResultType<()> {
    update_me_from_app_dir(
        Path::new(target_dir)
            .join(format!("{}.app", crate::get_app_name()))
            .to_string_lossy()
            .to_string(),
        true,
    )?;
    try_remove_temp_update_dir(Some(target_dir));
    Ok(())
}

pub fn get_double_click_time() -> u32 {
    // to-do: https://github.com/servo/core-foundation-rs/blob/786895643140fa0ee4f913d7b4aeb0c4626b2085/cocoa/src/appkit.rs#L2823
    500 as _
}

pub fn hide_dock() {
    unsafe {
        NSApp().setActivationPolicy_(NSApplicationActivationPolicyAccessory);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_temp_dir_is_process_specific() {
        let euid = unsafe { hbb_common::libc::geteuid() };
        let old_fixed_dir = Path::new("/tmp").join(format!(".rustdeskupdate-{}", euid));
        let update_temp_dir = new_update_temp_dir();

        assert_ne!(update_temp_dir, old_fixed_dir);
        assert!(update_temp_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .starts_with(&format!(".rustdeskupdate-{}-", euid)));
    }

    #[test]
    fn test_remove_temp_update_dir_cleans_current_user_old_dirs() {
        let euid = unsafe { hbb_common::libc::geteuid() };
        let stale_dir = Path::new("/tmp").join(format!(
            ".rustdeskupdate-{}-cleanup-test-{}-{}",
            euid,
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let unrelated_dir = Path::new("/tmp").join(format!(
            ".rustdeskupdate-cleanup-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::create_dir_all(&unrelated_dir).unwrap();

        try_remove_temp_update_dir(None);

        assert!(!stale_dir.exists());
        assert!(unrelated_dir.exists());
        std::fs::remove_dir_all(&unrelated_dir).unwrap();
    }

    #[test]
    fn test_remove_temp_update_dir_removes_symlink_without_touching_target() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-macos-cleanup-symlink-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let target_dir = test_dir.join("target");
        let link_path = test_dir.join("link");
        let target_file = target_dir.join("file");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(&target_file, b"target").unwrap();
        std::os::unix::fs::symlink(&target_dir, &link_path).unwrap();

        remove_temp_update_dir(&link_path);

        assert!(!std::fs::symlink_metadata(&link_path).is_ok());
        assert!(target_file.exists());
        std::fs::remove_dir_all(&test_dir).unwrap();
    }

    #[test]
    fn test_update_script_pid_controls_current_process_preservation() {
        assert_eq!(
            current_pid_for_update_script(true),
            std::process::id().to_string()
        );
        assert_eq!(current_pid_for_update_script(false), "0");
    }

    #[test]
    fn test_verify_dmg_file_sha256_uses_open_file() {
        let file_path = std::env::temp_dir().join(format!(
            "rustdesk-macos-dmg-sha256-test-{}",
            std::process::id()
        ));
        std::fs::write(&file_path, b"rustdesk").unwrap();
        let mut file = std::fs::File::open(&file_path).unwrap();

        let result = verify_dmg_file_sha256(
            &mut file,
            "304ca1638c5effa6832e0e15b958a8f74847efe4df9c3f3187216e921c168fed",
            &file_path.to_string_lossy(),
        );

        std::fs::remove_file(&file_path).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_to_verified_dmg_cleans_temp_dir_on_sha256_failure() {
        let file_path = std::env::temp_dir().join(format!(
            "rustdesk-macos-dmg-cleanup-test-{}",
            std::process::id()
        ));
        std::fs::write(&file_path, b"rustdesk").unwrap();

        let result = update_to_verified_dmg(
            &file_path.to_string_lossy(),
            "0000000000000000000000000000000000000000000000000000000000000000",
            None,
        );

        std::fs::remove_file(&file_path).unwrap();
        assert!(result.is_err());
        assert!(!get_update_temp_dir().exists());
    }

    #[test]
    fn test_create_update_temp_dmg_file_keeps_named_file() {
        let (_file, file_path) = create_update_temp_dmg_file().unwrap();
        let dmg_dir = file_path.parent().unwrap();
        let mode = std::fs::metadata(dmg_dir).unwrap().permissions().mode() & 0o777;

        assert!(file_path.exists());
        assert_eq!(dmg_dir.parent(), Some(get_update_temp_dir().as_path()));
        assert_eq!(mode, 0o700);
        std::fs::remove_file(file_path).unwrap();
    }

    #[test]
    fn test_attach_dmg_failure_message_mentions_stale_mount_point() {
        let message =
            attach_dmg_failure_message("/tmp/RustDesk.dmg", UPDATE_DMG_MOUNT_POINT, "failed");

        assert!(message.contains("/tmp/RustDesk.dmg"));
        assert!(message.contains(UPDATE_DMG_MOUNT_POINT));
        assert!(message.contains("stale mount"));
        assert!(message.contains("hdiutil detach"));
    }
}

#[inline]
#[allow(dead_code)]
fn get_server_start_time_of(p: &Process, path: &Path) -> Option<i64> {
    let cmd = p.cmd();
    if cmd.len() <= 1 {
        return None;
    }
    if &cmd[1] != "--server" {
        return None;
    }
    let Ok(cur) = std::fs::canonicalize(p.exe()) else {
        return None;
    };
    if &cur != path {
        return None;
    }
    Some(p.start_time() as _)
}

#[inline]
#[allow(dead_code)]
fn get_server_start_time(sys: &mut System, path: &Path) -> Option<(i64, Pid)> {
    sys.refresh_processes_specifics(ProcessRefreshKind::new());
    for (_, p) in sys.processes() {
        if let Some(t) = get_server_start_time_of(p, path) {
            return Some((t, p.pid() as _));
        }
    }
    None
}

pub fn handle_application_should_open_untitled_file() {
    hbb_common::log::debug!("icon clicked on finder");
    let x = std::env::args().nth(1).unwrap_or_default();
    if x == "--server" || x == "--cm" || x == "--tray" {
        std::thread::spawn(move || crate::handle_url_scheme("".to_lowercase()));
    }
}

/// Get all resolutions of the display. The resolutions are:
/// 1. Sorted by width and height in descending order, with duplicates removed.
/// 2. Filtered out if the width is less than 800 (800x600) if there are too many (e.g., >15).
/// 3. Contain HiDPI resolutions and the real resolutions.
///
/// We don't need to distinguish between HiDPI and real resolutions.
/// When the controlling side changes the resolution, it will call `change_resolution_directly()`.
/// `change_resolution_directly()` will try to use the HiDPI resolution first.
/// This is how teamviewer does it for now.
///
/// If we need to distinguish HiDPI and real resolutions, we can add a flag to the `Resolution` struct.
pub fn resolutions(name: &str) -> Vec<Resolution> {
    let mut v = vec![];
    if let Ok(display) = name.parse::<u32>() {
        let mut num = 0;
        unsafe {
            if YES == MacGetModeNum(display, &mut num) {
                let (mut widths, mut heights, mut _hidpis) =
                    (vec![0; num as _], vec![0; num as _], vec![NO; num as _]);
                let mut real_num = 0;
                if YES
                    == MacGetModes(
                        display,
                        widths.as_mut_ptr(),
                        heights.as_mut_ptr(),
                        _hidpis.as_mut_ptr(),
                        num,
                        &mut real_num,
                    )
                {
                    if real_num <= num {
                        v = (0..real_num)
                            .map(|i| Resolution {
                                width: widths[i as usize] as _,
                                height: heights[i as usize] as _,
                                ..Default::default()
                            })
                            .collect::<Vec<_>>();
                        // Sort by (w, h), desc
                        v.sort_by(|a, b| {
                            if a.width == b.width {
                                b.height.cmp(&a.height)
                            } else {
                                b.width.cmp(&a.width)
                            }
                        });
                        // Remove duplicates
                        v.dedup_by(|a, b| a.width == b.width && a.height == b.height);
                        // Filter out the ones that are less than width 800 (800x600) if there are too many.
                        // We can also do this filtering on the client side, but it is better not to change the client side to reduce the impact.
                        if v.len() > 15 {
                            // Most width > 800, so it's ok to remove the small ones.
                            v.retain(|r| r.width >= 800);
                        }
                        if v.len() > 15 {
                            // Ignore if the length is still too long.
                        }
                    }
                }
            }
        }
    }
    v
}

pub fn current_resolution(name: &str) -> ResultType<Resolution> {
    let display = name.parse::<u32>().map_err(|e| anyhow!(e))?;
    unsafe {
        let (mut width, mut height) = (0, 0);
        if NO == MacGetMode(display, &mut width, &mut height) {
            bail!("MacGetMode failed");
        }
        Ok(Resolution {
            width: width as _,
            height: height as _,
            ..Default::default()
        })
    }
}

pub fn change_resolution_directly(name: &str, width: usize, height: usize) -> ResultType<()> {
    let display = name.parse::<u32>().map_err(|e| anyhow!(e))?;
    unsafe {
        if NO == MacSetMode(display, width as _, height as _, true) {
            bail!("MacSetMode failed");
        }
    }
    Ok(())
}

pub fn check_super_user_permission() -> ResultType<bool> {
    unsafe { Ok(MacCheckAdminAuthorization() == YES) }
}

pub fn elevate(args: Vec<&str>, prompt: &str) -> ResultType<bool> {
    let cmd = std::env::current_exe()?;
    match cmd.to_str() {
        Some(cmd) => {
            let mut cmd_with_args = cmd.to_string();
            for arg in args {
                cmd_with_args = format!("{} {}", cmd_with_args, arg);
            }
            let script = format!(
                r#"do shell script "{}" with prompt "{}" with administrator privileges"#,
                cmd_with_args, prompt
            );
            match std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .arg(&get_active_username())
                .status()
            {
                Err(e) => {
                    bail!("Failed to run osascript: {}", e);
                }
                Ok(status) => Ok(status.success() && status.code() == Some(0)),
            }
        }
        None => {
            bail!("Failed to get current exe str");
        }
    }
}

pub struct WakeLock(Option<keepawake::AwakeHandle>);

impl WakeLock {
    pub fn new(display: bool, idle: bool, sleep: bool) -> Self {
        WakeLock(
            keepawake::Builder::new()
                .display(display)
                .idle(idle)
                .sleep(sleep)
                .create()
                .ok(),
        )
    }

    pub fn set_display(&mut self, display: bool) -> ResultType<()> {
        self.0
            .as_mut()
            .map(|h| h.set_display(display))
            .ok_or(anyhow!("no AwakeHandle"))?
    }
}

fn get_bundle_id() -> Option<String> {
    unsafe {
        let bundle: id = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return None;
        }

        let bundle_id: id = msg_send![bundle, bundleIdentifier];
        if bundle_id.is_null() {
            return None;
        }

        let c_str: *const std::os::raw::c_char = msg_send![bundle_id, UTF8String];
        if c_str.is_null() {
            return None;
        }

        let bundle_id_str = std::ffi::CStr::from_ptr(c_str)
            .to_string_lossy()
            .to_string();
        Some(bundle_id_str)
    }
}
