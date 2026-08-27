// https://developer.apple.com/documentation/appkit/nscursor
// https://github.com/servo/core-foundation-rs
// https://github.com/rust-windowing/winit

mod update_temp;
mod verified_dmg;

#[cfg(test)]
#[path = "macos/verified_dmg_tests.rs"]
mod verified_dmg_tests;

use super::{validate_install_app_name, CursorData, ResultType};
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
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
};
pub use update_temp::try_remove_temp_update_dir;
use update_temp::{copy_dmg_to_update_temp_file, get_update_temp_dir_string};
use verified_dmg::{
    attach_dmg, copy_and_verify_dmg_file, extract_dmg_inner, verified_dmg_path, verify_stored_dmg,
    VerifiedDmg,
};

// macOS boolean_t is defined as `int` in <mach/boolean.h>
type BooleanT = hbb_common::libc::c_int;

static PRIVILEGES_SCRIPTS_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/platform/privileges_scripts");
static mut LATEST_SEED: i32 = 0;
const UPDATE_CLEANUP_FAILED_AFTER_COMMIT: &str = "UPDATE_CLEANUP_FAILED_AFTER_COMMIT";
// `kill -9` may not work without administrator privileges.
const PRIVILEGED_UPDATE_BODY: &str = r#"
	on run {app_name, cur_pid, source_path, user_name, restore_owner, expected_sha256}
	    set app_bundle to "/Applications/" & app_name & ".app"
	    set app_bundle_q to quoted form of app_bundle
	    set installed_info_q to quoted form of (app_bundle & "/Contents/Info.plist")
	    set source_path_q to quoted form of source_path
	    set user_name_q to quoted form of user_name
	    set expected_sha256_q to quoted form of expected_sha256

	    set check_source to "if [ -n " & expected_sha256_q & " ]; then test -f " & source_path_q & "; else test -d " & source_path_q & "; fi;"
	    set kill_others to "pids=$(pgrep -x '" & app_name & "' | grep -vx " & cur_pid & " || true); if [ -n \"$pids\" ]; then echo \"$pids\" | xargs kill -9 || true; fi;"
	    -- Rehash the root-owned copy in a clean environment before staging bytes.
	    set prepare_verified to "verified_dir=$(/usr/bin/mktemp -d /tmp/.rustdeskupdate-verified.XXXXXX); /bin/chmod 0700 \"$verified_dir\"; verified_app=\"$verified_dir/" & app_name & ".app\"; dmg_attached=0; if [ -n " & expected_sha256_q & " ]; then verified_dmg=\"$verified_dir/update.dmg\"; /bin/cp " & source_path_q & " \"$verified_dmg\"; /usr/sbin/chown root:wheel \"$verified_dmg\"; /bin/chmod 0400 \"$verified_dmg\"; actual_sha256=$(/usr/bin/env -i /usr/bin/shasum -a 256 \"$verified_dmg\"); actual_sha256=${actual_sha256%% *}; if [ \"$actual_sha256\" != " & expected_sha256_q & " ]; then echo 'Update DMG SHA256 mismatch' >&2; exit 1; fi; dmg_mount=\"$verified_dir/mount\"; /bin/mkdir \"$dmg_mount\"; dmg_attached=1; /usr/bin/hdiutil attach -readonly -nobrowse -mountpoint \"$dmg_mount\" \"$verified_dmg\" >/dev/null; /usr/bin/ditto \"$dmg_mount/" & app_name & ".app\" \"$verified_app\"; /usr/bin/hdiutil detach \"$dmg_mount\" -force >/dev/null; dmg_attached=0; /bin/rm -f \"$verified_dmg\"; else /usr/bin/ditto " & source_path_q & " \"$verified_app\"; fi; /usr/sbin/chown -R root:wheel \"$verified_app\"; /bin/chmod -R go-w \"$verified_app\";"
	    set validate_verified_app to "installed_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' " & installed_info_q & "); candidate_bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' \"$verified_app/Contents/Info.plist\"); if [ -z \"$installed_bundle_id\" ] || [ -z \"$candidate_bundle_id\" ] || [ \"$installed_bundle_id\" != \"$candidate_bundle_id\" ]; then echo 'Update app bundle identifier mismatch' >&2; exit 1; fi;"
	    set prepare_swap_paths to "temp_bundle=" & app_bundle_q & ".new.$$; old_bundle=" & app_bundle_q & ".old.$$;"
	    set cleanup_swap_paths to "rm -rf \"$temp_bundle\" \"$old_bundle\";"
	    set stage_bundle to "ditto \"$verified_app\" \"$temp_bundle\";"
	    set protect_staged_bundle to "chown -R root:wheel \"$temp_bundle\"; chmod -R go-w \"$temp_bundle\"; (xattr -r -d com.apple.quarantine \"$temp_bundle\" || true);"
	    set move_current_bundle to "if [ -e " & app_bundle_q & " ]; then mv " & app_bundle_q & " \"$old_bundle\"; bundle_backed_up=1; fi;"
	    set install_staged_bundle to "mv \"$temp_bundle\" " & app_bundle_q & "; bundle_swapped=1;"
	    set restore_installed_owner to "if [ " & quoted form of restore_owner & " = '1' ]; then chown -R " & user_name_q & ":staff " & app_bundle_q & "; fi;"
	    set rollback_bundle to "if [ \"${bundle_backed_up:-0}\" -eq 1 ]; then if [ ! -e \"$old_bundle\" ]; then rollback_status=1; elif ! rm -rf " & app_bundle_q & "; then rollback_status=1; elif ! mv \"$old_bundle\" " & app_bundle_q & "; then rollback_status=1; fi; elif [ \"${bundle_swapped:-0}\" -eq 1 ]; then rm -rf " & app_bundle_q & " || rollback_status=1; fi;"
	    set cleanup_verified to "if [ \"${dmg_attached:-0}\" -eq 1 ]; then /usr/bin/hdiutil detach \"$dmg_mount\" -force >/dev/null 2>&1 || cleanup_status=1; fi; if [ -n \"${temp_bundle:-}\" ]; then rm -rf \"$temp_bundle\" || cleanup_status=1; fi; if [ -n \"${verified_dir:-}\" ]; then rm -rf \"$verified_dir\" || cleanup_status=1; fi;"
	    set rollback_update to "status=$?; trap - EXIT; set +e; cleanup_status=0; if [ \"${transaction_started:-0}\" -eq 1 ] && [ \"${transaction_committed:-0}\" -ne 1 ]; then rollback_status=0;" & rollback_bundle & "if [ \"$rollback_status\" -ne 0 ]; then status=1; fi; fi; if [ \"${rollback_status:-0}\" -eq 0 ]; then " & cleanup_verified & "fi; if [ \"$cleanup_status\" -ne 0 ] && [ \"${transaction_committed:-0}\" -ne 1 ]; then status=1; elif [ \"$cleanup_status\" -ne 0 ]; then echo 'UPDATE_CLEANUP_FAILED_AFTER_COMMIT'; fi; exit \"$status\";"
	    set commit_update to "transaction_committed=1; if ! rm -rf \"$old_bundle\"; then echo 'UPDATE_CLEANUP_FAILED_AFTER_COMMIT'; fi;"
	    set copy_files to prepare_swap_paths & cleanup_swap_paths & stage_bundle & protect_staged_bundle & "transaction_started=1;" & move_current_bundle & install_staged_bundle & restore_installed_owner & commit_update
	    set sh to "set -e; transaction_started=0; transaction_committed=0; bundle_backed_up=0; bundle_swapped=0; trap " & quoted form of rollback_update & " EXIT;" & check_source & prepare_verified & validate_verified_app & kill_others & copy_files

	    do shell script sh with prompt app_name & " wants to update itself" with administrator privileges
	end run
	        "#;

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

enum UpdateSource {
    AppDir(String),
    VerifiedDmg {
        path: String,
        expected_sha256: String,
    },
}

fn log_update_cleanup_warning(output: &std::process::Output) {
    if String::from_utf8_lossy(&output.stdout).contains(UPDATE_CLEANUP_FAILED_AFTER_COMMIT) {
        log::warn!("Update committed, but temporary update cleanup failed");
    }
}

impl UpdateSource {
    fn into_script_args(self) -> (String, String) {
        match self {
            Self::AppDir(path) => (path, String::new()),
            Self::VerifiedDmg {
                path,
                expected_sha256,
            } => (path, expected_sha256),
        }
    }
}

fn update_daemon_agent(agent_plist_file: String, update_source: UpdateSource) -> ResultType<()> {
    let update_script_file = "update.scpt";
    let Some(update_script) = PRIVILEGES_SCRIPTS_DIR.get_file(update_script_file) else {
        bail!("Failed to find {}", update_script_file);
    };
    let Some(update_script_body) = update_script.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read {}", update_script_file);
    };

    let current_pid = std::process::id().to_string();
    let (update_source_path, expected_sha256) = update_source.into_script_args();
    let func = move || -> ResultType<()> {
        let mut binding = std::process::Command::new("osascript");
        let cmd = binding
            .arg("-e")
            .arg(update_script_body)
            .arg(&get_active_username())
            .arg(&current_pid)
            .arg(update_source_path)
            .arg(expected_sha256);
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
            Ok(output) => {
                log_update_cleanup_warning(&output);
                let installed = std::path::Path::new(&agent_plist_file).exists();
                log::info!("Agent file {} installed: {}", &agent_plist_file, installed);
            }
        }
        Ok(())
    };
    func()
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

fn write_plist_atomically(path: &str, body: &str) -> ResultType<()> {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    let temporary = format!("{}.tmp.{}", path, std::process::id());
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o644))?;
        file.write_all(body.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        Ok::<(), std::io::Error>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result.map_err(Into::into)
}

pub fn write_plists() -> ResultType<()> {
    let daemon_plist_path = format!(
        "/Library/LaunchDaemons/com.carriez.{}_service.plist",
        crate::get_app_name()
    );
    let agent_plist_path = format!(
        "/Library/LaunchAgents/com.carriez.{}_server.plist",
        crate::get_app_name()
    );
    let Some(daemon_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("daemon.plist") else {
        bail!("daemon.plist not found in embedded resources");
    };
    let Some(daemon_plist_body) = daemon_plist.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read daemon.plist");
    };
    let Some(agent_plist) = PRIVILEGES_SCRIPTS_DIR.get_file("agent.plist") else {
        bail!("agent.plist not found in embedded resources");
    };
    let Some(agent_plist_body) = agent_plist.contents_utf8().map(correct_app_name) else {
        bail!("Failed to read agent.plist");
    };
    write_plist_atomically(&daemon_plist_path, &daemon_plist_body)?;
    write_plist_atomically(&agent_plist_path, &agent_plist_body)?;
    log::info!("[write-plists] Wrote daemon and agent plists");
    Ok(())
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

/// Return every UID with a login-window/session entry. Fast user switching
/// can leave several GUI bootstrap domains alive at once, so updating only
/// the console user can leave another user's agent on the old bundle.
pub(crate) fn get_logged_in_uids() -> Vec<u32> {
    let mut uids = std::collections::BTreeSet::new();
    if let Ok(output) = std::process::Command::new("/usr/bin/who").output() {
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Some(username) = line.split_whitespace().next() else {
                continue;
            };
            let Ok(output) = std::process::Command::new("/usr/bin/id")
                .args(["-u", username])
                .output()
            else {
                continue;
            };
            let Ok(uid) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u32>()
            else {
                continue;
            };
            let gui_domain = format!("gui/{}", uid);
            if std::process::Command::new("/bin/launchctl")
                .args(["print", &gui_domain])
                .output()
                .is_ok_and(|output| output.status.success())
            {
                uids.insert(uid);
            }
        }
    }
    if let Ok(active_uid) = get_active_userid().parse::<u32>() {
        if active_uid == 0 {
            // UID 0 owns /dev/console while the LoginWindow session is active.
            // Query that server even when fast-switched GUI domains also exist.
            uids.insert(0);
        } else {
            let gui_domain = format!("gui/{}", active_uid);
            if std::process::Command::new("/bin/launchctl")
                .args(["print", &gui_domain])
                .output()
                .is_ok_and(|output| output.status.success())
            {
                uids.insert(active_uid);
            }
        }
    }
    if uids.is_empty() {
        // The login window has no ordinary gui/0 bootstrap domain.
        uids.insert(0);
    }
    uids.into_iter().collect()
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

/// Starts the macOS system service IPC listener and the background
/// silent auto-update thread.
pub fn start_os_service() {
    log::info!("Username: {}", crate::username());
    // Silent auto-update — runs as root via LaunchDaemon, no osascript dialog needed
    crate::updater::start_auto_update_macos();
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
    update_me_from_app_dir(app_dir)
}

fn update_me_from_app_dir(app_dir: String) -> ResultType<()> {
    update_me_from_source(UpdateSource::AppDir(app_dir))
}

fn update_me_from_source(update_source: UpdateSource) -> ResultType<()> {
    let app_name = crate::get_app_name();
    validate_install_app_name(&app_name)?;
    let is_installed_daemon = is_installed_daemon(false);
    let option_stop_service = "stop-service";
    let is_service_stopped = hbb_common::config::option2bool(
        option_stop_service,
        &crate::ui_interface::get_option(option_stop_service),
    );

    if is_installed_daemon && !is_service_stopped {
        let agent = format!("{}_server.plist", crate::get_full_name());
        let agent_plist_file = format!("/Library/LaunchAgents/{}", agent);
        update_daemon_agent(agent_plist_file, update_source)?;
    } else {
        let (update_source_path, expected_sha256) = update_source.into_script_args();
        let output = Command::new("osascript")
            .arg("-e")
            .arg(PRIVILEGED_UPDATE_BODY)
            .arg(app_name.to_string())
            .arg(std::process::id().to_string())
            .arg(update_source_path)
            .arg(get_active_username())
            .arg(if is_installed_daemon { "0" } else { "1" })
            .arg(expected_sha256)
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
            Ok(output) => log_update_cleanup_warning(&output),
        }
    }
    match Command::new("open")
        .arg("-n")
        .arg(&format!("/Applications/{}.app", app_name))
        .status()
    {
        Ok(status) if !status.success() => {
            log::warn!("Failed to relaunch updated app: {}", status);
        }
        Err(err) => {
            log::warn!("Failed to relaunch updated app: {}", err);
        }
        _ => {}
    }
    // leave open a little time
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

pub fn update_from_dmg(dmg_path: &str) -> ResultType<()> {
    let update_temp_dir = get_update_temp_dir_string();
    println!("Starting update from DMG: {}", dmg_path);
    let update_result = (|| {
        let copied_dmg = copy_dmg_to_update_temp_file(dmg_path)?;
        let guard = attach_dmg(&copied_dmg.to_string_lossy())?;
        update_me_from_app_dir(format!(
            "{}/{}.app",
            guard.mount_point(),
            crate::get_app_name()
        ))
    })();
    try_remove_temp_update_dir(Some(&update_temp_dir));
    update_result?;
    println!("Update process started");
    Ok(())
}

pub fn update_to_verified_dmg(
    file: &str,
    expected_sha256: &str,
    expected_size: Option<u64>,
) -> ResultType<()> {
    let verified_dmg = copy_and_verify_dmg_file(file, expected_sha256, expected_size)?;
    update_from_verified_dmg(&verified_dmg)?;
    // Remove the private DMG copy before this process exits.
    drop(verified_dmg);
    quit_gui();
    Ok(())
}

fn update_from_verified_dmg(verified_dmg: &VerifiedDmg) -> ResultType<()> {
    println!("Starting update from verified DMG");
    update_me_from_source(verified_dmg_update_source(verified_dmg)?)?;
    println!("Update process started");
    Ok(())
}

fn backup_update_plist(source: &str, backup: &str) -> ResultType<()> {
    match std::fs::symlink_metadata(source) {
        Ok(metadata) => {
            if !metadata.file_type().is_file() {
                bail!("[root-update] plist is not a regular file: {}", source);
            }
            std::fs::copy(source, backup)?;
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            bail!(
                "[root-update] required installed plist is missing: {}",
                source
            )
        }
        Err(err) => Err(err.into()),
    }
}

fn validate_update_tree(path: &Path, framework_root: Option<&Path>) -> ResultType<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        // Frameworks legitimately use internal symlinks (Resources,
        // Versions/Current), but never allow a link to leave its framework.
        let Some(framework_root) = framework_root else {
            bail!(
                "[root-update] symlink outside framework: {}",
                path.display()
            );
        };
        let target = std::fs::read_link(path)?;
        let target = if target.is_absolute() {
            target
        } else {
            path.parent().unwrap_or(Path::new("/")).join(target)
        };
        let target = std::fs::canonicalize(target)?;
        let framework_root = std::fs::canonicalize(framework_root)?;
        if target.starts_with(&framework_root) {
            return Ok(());
        }
        bail!("[root-update] symlink in update bundle: {}", path.display());
    }
    if metadata.file_type().is_dir() {
        for entry in std::fs::read_dir(path)? {
            let child = entry?.path();
            let child_framework_root = if child
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".framework"))
                && std::fs::symlink_metadata(&child)?.file_type().is_dir()
            {
                Some(child.as_path())
            } else {
                framework_root
            };
            validate_update_tree(&child, child_framework_root)?;
        }
    } else if !metadata.file_type().is_file() {
        bail!(
            "[root-update] unsupported file in update bundle: {}",
            path.display()
        );
    }
    Ok(())
}

fn read_bundle_plist_value(app: &Path, key: &str) -> ResultType<String> {
    let info_plist = app.join("Contents/Info.plist");
    let plist_command = format!("Print :{key}");
    let output = Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", plist_command.as_str()])
        .arg(info_plist)
        .output()?;
    if !output.status.success() {
        bail!(
            "[root-update] failed to read bundle plist value {key}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let value = String::from_utf8(output.stdout)
        .map_err(|err| anyhow!("[root-update] bundle plist value {key} is not UTF-8: {err}"))?;
    if value.trim().is_empty() {
        bail!("[root-update] bundle plist value {key} is empty");
    }
    Ok(value.trim().to_owned())
}

fn verify_update_app_identity(installed: &Path, candidate: &Path) -> ResultType<()> {
    let installed_id = read_bundle_plist_value(installed, "CFBundleIdentifier")?;
    let candidate_id = read_bundle_plist_value(candidate, "CFBundleIdentifier")?;
    if installed_id != candidate_id {
        bail!("[root-update] candidate bundle identifier mismatch");
    }
    Ok(())
}

/// Performs a silent update from a DMG file without any osascript dialog.
/// Must be called from a process running as root (e.g. the service binary).
pub fn update_from_dmg_as_root(dmg_path: &str, expected_version: &str) -> ResultType<()> {
    let app_name = crate::get_app_name();
    validate_install_app_name(&app_name)?;
    let app_bundle = format!("/Applications/{}.app", app_name);
    let tmp_dir_output = std::process::Command::new("/usr/bin/mktemp")
        .args(&["-d", "/tmp/.rustdeskupdate-root-XXXXXX"])
        .output()?;
    let tmp_dir = String::from_utf8(tmp_dir_output.stdout)
        .map_err(|e| anyhow!("[root-update] mktemp output error: {}", e))?
        .trim()
        .to_string();
    if tmp_dir.is_empty() {
        bail!("[root-update] Failed to create temp directory");
    }
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let agent_plist = format!(
        "/Library/LaunchAgents/com.carriez.{}_server.plist",
        app_name
    );
    let daemon_plist = format!(
        "/Library/LaunchDaemons/com.carriez.{}_service.plist",
        app_name
    );

    log::info!(
        "[root-update] Starting silent root update from {}",
        dmg_path
    );
    // Check sessions before extracting to avoid unnecessary work
    if !crate::updater::has_no_active_conns_ipc() {
        bail!("[root-update] Active session detected, deferring update.");
    }
    // Extract DMG to temp dir
    extract_dmg_into_existing_dir(dmg_path, &tmp_dir)?;
    let src_app = format!("{}/{}.app", tmp_dir, app_name);
    log::info!("[root-update] DMG extracted to {}", tmp_dir);
    validate_update_tree(Path::new(&src_app), None)?;
    verify_update_app_identity(Path::new(&app_bundle), Path::new(&src_app))?;

    // Bind the downloaded asset to the version returned by the update
    // service before changing plists or executing anything from the staged
    // bundle. A release asset with the right filename but the wrong bundle
    // must not be allowed to replace the installed application.
    let info_plist = format!("{}/Contents/Info.plist", src_app);
    let staged_version_result = (|| -> ResultType<String> {
        let output = Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", "Print :CFBundleShortVersionString", &info_plist])
            .output()?;
        if !output.status.success() {
            bail!(
                "[root-update] failed to read staged bundle version: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let version = String::from_utf8(output.stdout)
            .map_err(|err| anyhow!("[root-update] staged bundle version is not UTF-8: {}", err))?;
        if version.trim().is_empty() {
            bail!("[root-update] staged bundle version is empty");
        }
        Ok(version.trim().to_owned())
    })();
    let staged_version = match staged_version_result {
        Ok(version) => version,
        Err(err) => {
            if let Err(cleanup_err) = std::fs::remove_dir_all(&tmp_dir) {
                log::warn!(
                    "[root-update] Failed to remove temp dir {}: {}",
                    tmp_dir,
                    cleanup_err
                );
            }
            return Err(err);
        }
    };
    if staged_version != expected_version {
        if let Err(err) = std::fs::remove_dir_all(&tmp_dir) {
            log::warn!(
                "[root-update] Failed to remove temp dir {}: {}",
                tmp_dir,
                err
            );
        }
        bail!(
            "[root-update] staged bundle version mismatch: expected {:?}, found {:?}",
            expected_version,
            staged_version
        );
    }

    // A leftover backup makes `mv app app.bak` nest the live bundle inside
    // the old directory instead of creating a transaction backup. Never
    // overwrite or guess at recovery state left by an earlier interrupted
    // update; require an administrator to inspect it first.
    let app_backup = format!("{}.bak", app_bundle);
    let failed_bundle = format!("{}.failed-update", app_bundle);
    for recovery_path in [&app_backup, &failed_bundle] {
        match std::fs::symlink_metadata(recovery_path) {
            Ok(_) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                bail!(
                    "[root-update] stale application recovery path requires inspection: {}",
                    recovery_path
                );
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(err.into());
            }
        }
    }

    // Backup current plists before overwriting — needed for restore on reload failure
    let daemon_plist_bak = format!("{}/daemon_plist.bak", tmp_dir);
    let agent_plist_bak = format!("{}/agent_plist.bak", tmp_dir);
    // Backups are part of the update transaction. Do not allow the new
    // service binary to overwrite either live plist unless both installed
    // definitions have been captured successfully.
    backup_update_plist(&daemon_plist, &daemon_plist_bak)?;
    backup_update_plist(&agent_plist, &agent_plist_bak)?;

    // Ensure the staged release contains the service executable before we
    // proceed. Plist generation itself is done in this already-root process;
    // launching a freshly extracted service binary from /tmp is not required.
    let new_service = format!("{}/Contents/MacOS/service", src_app);
    if !std::path::Path::new(&new_service).is_file() {
        bail!(
            "[root-update] staged service binary is missing: {}",
            new_service
        );
    }
    // The new binary writes its own plist definitions after the bundle is
    // moved into its final root-owned location.  This avoids executing code
    // directly from /tmp while ensuring the plist matches the new release.

    // Final session check after extraction — minimize race window
    if !crate::updater::has_no_active_conns_ipc() {
        let _ = std::fs::remove_dir_all(&tmp_dir);
        bail!("[root-update] Active session detected after extraction, deferring update.");
    }

    // Let the detached-script launch settle before taking the affected-user
    // snapshot. The final IPC check then happens after the delay and as close
    // as possible to stopping those exact launchd domains.
    std::thread::sleep(std::time::Duration::from_secs(3));
    if !crate::updater::has_no_active_conns_ipc() {
        bail!("[root-update] active session started before update launch");
    }
    let logged_in_uids = get_logged_in_uids();
    // UIDs are parsed as integers before embedding in the root-run shell
    // script, so they cannot alter its command structure.
    let uid_list = logged_in_uids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    // Write a shell script that runs detached after this function returns.
    // We cannot directly replace /Applications/RustDesk.app while it is running,
    // so we spawn a script that waits, kills processes, copies, and restarts.
    let daemon_label = format!("com.carriez.{}_service", app_name);
    let agent_label = format!("com.carriez.{}_server", app_name);
    let script_path = format!("{}/rustdesk_update.sh", tmp_dir);
    let script = format!(
        r#"#!/bin/sh
rollback_done=0
bundle_swapped=0
bootstrap_agent() {{
    agent_uid="$1"
    if [ "$agent_uid" != "0" ]; then
        launchctl bootstrap gui/"$agent_uid" "{agent_plist}" 2>/dev/null || \
            launchctl bootstrap user/"$agent_uid" "{agent_plist}" 2>/dev/null || \
            launchctl load -w "{agent_plist}" 2>/dev/null
    else
        # At the login window there is no gui/0 domain.  launchctl load uses
        # the plist's LoginWindow/Aqua session policy instead.
        launchctl load -w -S LoginWindow "{agent_plist}" 2>/dev/null || \
            launchctl load -w "{agent_plist}" 2>/dev/null
    fi
}}
bootstrap_agents() {{
    for agent_uid in {uid_list}; do
        bootstrap_agent "$agent_uid" || return 1
    done
}}
loginwindow_asid() {{
    root_user_info=$(launchctl print user/0 2>/dev/null || true)
    root_login_asid=$(printf '%s\n' "$root_user_info" | \
        awk '/^[[:space:]]*asid = [0-9]+[[:space:]]*$/ {{print $3; exit}}')
    case "$root_login_asid" in
        ''|*[!0-9]*) return 1 ;;
    esac
    printf '%s\n' "$root_login_asid"
}}
bootout_agents() {{
    # Legacy launchctl commands can report success despite operational
    # failure. Treat these as requests; stop_agents verifies the result.
    stopping_loginwindow_asid=""
    for agent_uid in {uid_list}; do
        if [ "$agent_uid" != "0" ]; then
            launchctl bootout gui/"$agent_uid"/{agent_label} 2>/dev/null || true
            launchctl bootout user/"$agent_uid"/{agent_label} 2>/dev/null || true
        else
            # LoginWindow jobs run in a login/<asid> domain even though
            # legacy root `launchctl load` is issued from the system context.
            # Remove every applicable registration before killing the process
            # so KeepAlive cannot immediately respawn it.
            launchctl unload -w -S LoginWindow "{agent_plist}" 2>/dev/null || true
            stopping_loginwindow_asid=$(loginwindow_asid || true)
            if [ -n "$stopping_loginwindow_asid" ]; then
                launchctl bootout login/"$stopping_loginwindow_asid"/{agent_label} 2>/dev/null || true
            fi
            launchctl bootout user/0/{agent_label} 2>/dev/null || true
            launchctl bootout system/{agent_label} 2>/dev/null || true
            launchctl unload -w "{agent_plist}" 2>/dev/null || true
        fi
    done
}}
find_agent_pid() {{
    agent_uid="$1"
    for candidate_pid in $(pgrep -u "$agent_uid" -x {app_name} 2>/dev/null || true); do
        process_args=$(ps -p "$candidate_pid" -o args= 2>/dev/null || true)
        if printf '%s\n' "$process_args" | grep -F "/Applications/{app_name}.app/Contents/MacOS/{app_name}" >/dev/null && \
           printf '%s\n' "$process_args" | grep -E '(^|[[:space:]])--server([[:space:]]|$)' >/dev/null; then
            printf '%s\n' "$candidate_pid"
            return 0
        fi
    done
    return 1
}}
launchd_agent_pid() {{
    agent_uid="$1"
    agent_info=$(launchctl print gui/"$agent_uid"/{agent_label} 2>/dev/null || \
        launchctl print user/"$agent_uid"/{agent_label} 2>/dev/null || true)
    agent_job_pid=$(printf '%s\n' "$agent_info" | awk '/^[[:space:]]*pid = / {{print $3; exit}}')
    if [ -n "$agent_job_pid" ] && \
       printf '%s\n' "$agent_info" | grep -E '^[[:space:]]*state = running[[:space:]]*$' >/dev/null; then
        printf '%s\n' "$agent_job_pid"
        return 0
    fi
    return 1
}}
agent_pid_for_uid() {{
    agent_uid="$1"
    if [ "$agent_uid" = "0" ]; then
        # LoginWindow agents have no ordinary gui/0 bootstrap domain. Locate
        # the root-owned --server process and validate it below instead.
        find_agent_pid "$agent_uid"
    else
        launchd_agent_pid "$agent_uid"
    fi
}}
agent_process_matches() {{
    agent_uid="$1"
    agent_pid="$2"
    process_uid=$(ps -p "$agent_pid" -o uid= 2>/dev/null | tr -d '[:space:]')
    process_args=$(ps -p "$agent_pid" -o args= 2>/dev/null || true)
    [ "$process_uid" = "$agent_uid" ] && \
        printf '%s\n' "$process_args" | grep -F "/Applications/{app_name}.app/Contents/MacOS/{app_name}" >/dev/null && \
        printf '%s\n' "$process_args" | grep -E '(^|[[:space:]])--server([[:space:]]|$)' >/dev/null
}}
capture_stopping_agent_pids() {{
    stopping_agent_pids=""
    for agent_uid in {uid_list}; do
        for candidate_pid in $(pgrep -u "$agent_uid" -x {app_name} 2>/dev/null || true); do
            if agent_process_matches "$agent_uid" "$candidate_pid"; then
                stopping_agent_pids="$stopping_agent_pids $candidate_pid"
            fi
        done
    done
}}
terminate_agent_processes() {{
    for agent_uid in {uid_list}; do
        for candidate_pid in $(pgrep -u "$agent_uid" -x {app_name} 2>/dev/null || true); do
            if agent_process_matches "$agent_uid" "$candidate_pid"; then
                kill -KILL "$candidate_pid" 2>/dev/null || true
            fi
        done
    done
}}
terminate_user_bundle_processes() {{
    for agent_uid in {uid_list}; do
        for candidate_pid in $(pgrep -u "$agent_uid" -x {app_name} 2>/dev/null || true); do
            process_args=$(ps -p "$candidate_pid" -o args= 2>/dev/null || true)
            if printf '%s\n' "$process_args" | grep -F "/Applications/{app_name}.app/" >/dev/null; then
                kill -KILL "$candidate_pid" 2>/dev/null || true
            fi
        done
    done
}}
user_bundle_processes_absent() {{
    for agent_uid in {uid_list}; do
        for candidate_pid in $(pgrep -u "$agent_uid" -x {app_name} 2>/dev/null || true); do
            process_args=$(ps -p "$candidate_pid" -o args= 2>/dev/null || true)
            if printf '%s\n' "$process_args" | grep -F "/Applications/{app_name}.app/" >/dev/null; then
                return 1
            fi
        done
    done
    return 0
}}
stop_user_bundle_processes() {{
    terminate_user_bundle_processes
    for _ in $(/usr/bin/seq 1 30); do
        if user_bundle_processes_absent; then
            sleep 2
            user_bundle_processes_absent && return 0
        fi
        terminate_user_bundle_processes
        sleep 1
    done
    return 1
}}
agent_jobs_absent() {{
    for agent_uid in {uid_list}; do
        if [ "$agent_uid" != "0" ]; then
            if launchctl print gui/"$agent_uid"/{agent_label} >/dev/null 2>&1 || \
               launchctl print user/"$agent_uid"/{agent_label} >/dev/null 2>&1; then
                return 1
            fi
        else
            if launchctl print system/{agent_label} >/dev/null 2>&1 || \
               launchctl print user/0/{agent_label} >/dev/null 2>&1; then
                return 1
            fi
            if [ -n "$stopping_loginwindow_asid" ] && \
               launchctl print login/"$stopping_loginwindow_asid"/{agent_label} >/dev/null 2>&1; then
                return 1
            fi
        fi
        find_agent_pid "$agent_uid" >/dev/null 2>&1 && return 1
    done
    return 0
}}
captured_agent_pids_gone() {{
    for stopped_pid in $stopping_agent_pids; do
        kill -0 "$stopped_pid" 2>/dev/null && return 1
    done
    return 0
}}
agents_stopped() {{
    captured_agent_pids_gone && agent_jobs_absent
}}
stop_agents() {{
    bootout_agents
    terminate_agent_processes
    for _ in $(/usr/bin/seq 1 30); do
        if agents_stopped; then
            sleep 2
            agents_stopped && return 0
        fi
        terminate_agent_processes
        sleep 1
    done
    return 1
}}
capture_agent_snapshot() {{
    agent_pids=""
    for agent_uid in {uid_list}; do
        agent_pid=$(agent_pid_for_uid "$agent_uid" || true)
        [ -n "$agent_pid" ] || return 1
        [ -S "/tmp/{app_name}-$agent_uid/ipc" ] || return 1
        kill -0 "$agent_pid" 2>/dev/null || return 1
        agent_process_matches "$agent_uid" "$agent_pid" || return 1
        agent_pids="$agent_pids $agent_uid:$agent_pid"
    done
    return 0
}}
agent_snapshot_stable() {{
    for agent_entry in $agent_pids; do
        agent_uid=$(printf '%s\n' "$agent_entry" | cut -d: -f1)
        expected_pid=$(printf '%s\n' "$agent_entry" | cut -d: -f2)
        current_pid=$(agent_pid_for_uid "$agent_uid" || true)
        [ -n "$current_pid" ] && [ "$current_pid" = "$expected_pid" ] || return 1
        [ -S "/tmp/{app_name}-$agent_uid/ipc" ] || return 1
        kill -0 "$current_pid" 2>/dev/null || return 1
        agent_process_matches "$agent_uid" "$current_pid" || return 1
    done
    return 0
}}
agent_ready() {{
    for _ in $(/usr/bin/seq 1 30); do
        if capture_agent_snapshot; then
            sleep 2
            agent_snapshot_stable && return 0
        fi
        sleep 1
    done
    return 1
}}
daemon_snapshot_stable() {{
    stable_daemon_info=$(launchctl print system/{daemon_label} 2>/dev/null || true)
    stable_daemon_pid=$(printf '%s\n' "$stable_daemon_info" | awk '/^[[:space:]]*pid = / {{print $3; exit}}')
    [ -n "$daemon_pid" ] && \
        [ "$stable_daemon_pid" = "$daemon_pid" ] && \
        printf '%s\n' "$stable_daemon_info" | grep -E '^[[:space:]]*state = running[[:space:]]*$' >/dev/null && \
        [ -S "/tmp/{app_name}-service/ipc_service" ] && \
        kill -0 "$daemon_pid" 2>/dev/null
}}
daemon_ready() {{
    daemon_pid=""
    for _ in $(/usr/bin/seq 1 30); do
        daemon_info=$(launchctl print system/{daemon_label} 2>/dev/null || true)
        daemon_pid=$(printf '%s\n' "$daemon_info" | awk '/^[[:space:]]*pid = / {{print $3; exit}}')
        if [ -n "$daemon_pid" ] && \
           printf '%s\n' "$daemon_info" | grep -E '^[[:space:]]*state = running[[:space:]]*$' >/dev/null && \
           [ -S "/tmp/{app_name}-service/ipc_service" ] && \
           kill -0 "$daemon_pid" 2>/dev/null; then
            sleep 2
            daemon_snapshot_stable && return 0
        fi
        sleep 1
    done
    return 1
}}
capture_stopping_daemon_pid() {{
    stopping_daemon_info=$(launchctl print system/{daemon_label} 2>/dev/null || true)
    stopping_daemon_pid=$(printf '%s\n' "$stopping_daemon_info" | awk '/^[[:space:]]*pid = / {{print $3; exit}}')
}}
daemon_stopped() {{
    if [ -n "$stopping_daemon_pid" ] && kill -0 "$stopping_daemon_pid" 2>/dev/null; then
        return 1
    fi
    ! launchctl print system/{daemon_label} >/dev/null 2>&1
}}
stop_daemon() {{
    capture_stopping_daemon_pid
    # Command status is advisory. daemon_stopped verifies that both the
    # captured process generation and launchd registration are gone.
    launchctl bootout system/{daemon_label} 2>/dev/null || \
        launchctl unload -w "{daemon_plist}" 2>/dev/null || true
    for _ in $(/usr/bin/seq 1 30); do
        if daemon_stopped; then
            sleep 2
            daemon_stopped && return 0
        fi
        sleep 1
    done
    return 1
}}
write_new_plists() {{
    /Applications/{app_name}.app/Contents/MacOS/service --write-plists \
        >"{tmp_dir}/write-plists.log" 2>&1 &
    write_pid=$!
    for _ in $(/usr/bin/seq 1 60); do
        if ! kill -0 "$write_pid" 2>/dev/null; then
            wait "$write_pid"
            return $?
        fi
        sleep 1
    done
    kill -TERM "$write_pid" 2>/dev/null || true
    sleep 1
    kill -KILL "$write_pid" 2>/dev/null || true
    wait "$write_pid" 2>/dev/null || true
    return 124
}}
restore_old_bundle() {{
    [ "$bundle_swapped" -eq 1 ] || return 0
    if [ ! -d "{app_bundle}.bak" ] || [ -L "{app_bundle}.bak" ]; then
        echo "[root-update] CRITICAL: valid application backup is unavailable" >> {tmp_dir}/rustdesk_root_update.log
        return 1
    fi
    if [ -e "{app_bundle}" ] || [ -L "{app_bundle}" ]; then
        if [ -e "{app_bundle}.failed-update" ] || [ -L "{app_bundle}.failed-update" ] || \
           ! mv "{app_bundle}" "{app_bundle}.failed-update"; then
            echo "[root-update] CRITICAL: could not vacate failed bundle safely" >> {tmp_dir}/rustdesk_root_update.log
            return 1
        fi
    fi
    if ! mv "{app_bundle}.bak" "{app_bundle}"; then
        echo "[root-update] CRITICAL: failed to restore application bundle" >> {tmp_dir}/rustdesk_root_update.log
        if [ ! -e "{app_bundle}" ] && [ ! -L "{app_bundle}" ]; then
            mv "{app_bundle}.failed-update" "{app_bundle}" 2>/dev/null || true
        fi
        return 1
    fi
    rm -rf "{app_bundle}.failed-update" 2>/dev/null || true
    bundle_swapped=0
    return 0
}}
rollback_transaction() {{
    # Rollback restores and verifies unattended service state. It does not
    # guarantee relaunching GUI windows that were stopped by the transaction.
    [ "$rollback_done" -eq 0 ] || return 0
    rollback_done=1
    restore_failed=0
    stop_daemon || restore_failed=1
    capture_stopping_agent_pids
    stop_agents || restore_failed=1
    restore_old_bundle || restore_failed=1
    cp "{daemon_plist_bak}" "{daemon_plist}" || restore_failed=1
    cp "{agent_plist_bak}" "{agent_plist}" || restore_failed=1
    touch /var/root/.rustdeskupdate_failed || restore_failed=1
    if ! launchctl load -w "{daemon_plist}" 2>/dev/null && \
       ! launchctl bootstrap system "{daemon_plist}" 2>/dev/null; then
        restore_failed=1
    fi
    daemon_ready || restore_failed=1
    bootstrap_agents || restore_failed=1
    agent_ready || restore_failed=1
    if [ "$restore_failed" -eq 0 ] && \
       {{ ! daemon_snapshot_stable || ! agent_snapshot_stable; }}; then
        restore_failed=1
    fi
    if [ "$restore_failed" -ne 0 ]; then
        echo "[root-update] CRITICAL: rollback restoration failed" >> {tmp_dir}/rustdesk_root_update.log
    else
        echo "[root-update] Rollback daemon and agents verified healthy" >> {tmp_dir}/rustdesk_root_update.log
    fi
}}
trap rollback_transaction EXIT
gui_uids=""
for agent_uid in {uid_list}; do
    for pid in $(pgrep -u "$agent_uid" -x {app_name} || true); do
        process_args=$(ps -p "$pid" -o args= 2>/dev/null || true)
        if printf '%s\n' "$process_args" | grep -F "/Applications/{app_name}.app/" >/dev/null && \
           ! printf '%s\n' "$process_args" | grep -E "(^|[[:space:]])(--server|--service|--update)([[:space:]]|$)" >/dev/null; then
            gui_uids="$gui_uids $agent_uid"
            break
        fi
    done
done
if ! capture_agent_snapshot; then
    echo "[root-update] old LaunchAgent readiness check failed before shutdown" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
capture_stopping_agent_pids
if ! stop_daemon; then
    echo "[root-update] daemon did not stop before bundle swap" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
if ! stop_agents; then
    echo "[root-update] old LaunchAgent did not stop before bundle swap" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Agents have already been verified absent. Stop and verify any remaining GUI
# processes as well so no process keeps the old bundle mapped across the swap.
if ! stop_user_bundle_processes; then
    echo "[root-update] RustDesk GUI process did not stop before bundle swap" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
staged_bundle="{tmp_dir}/staged.app"
if [ -e "$staged_bundle" ] || [ -L "$staged_bundle" ]; then
    echo "[root-update] staged bundle path already exists, aborting" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
if ! ditto {src_app} "$staged_bundle" 2>/dev/null; then
    echo "[root-update] ditto failed, aborting update" >> {tmp_dir}/rustdesk_root_update.log
    rm -rf "$staged_bundle"
    exit 1
fi
# Validate staged bundle before atomic swap
if [ ! -d "$staged_bundle/Contents/MacOS" ] || \
   [ ! -f "$staged_bundle/Contents/MacOS/{app_name}" ] || \
   [ ! -f "$staged_bundle/Contents/MacOS/service" ] || \
   [ ! -f "$staged_bundle/Contents/Info.plist" ]; then
    echo "[root-update] staged bundle validation failed, aborting" >> {tmp_dir}/rustdesk_root_update.log
    rm -rf "$staged_bundle"
    exit 1
fi
if ! mv {app_bundle} {app_bundle}.bak; then
    echo "[root-update] backup mv failed, aborting" >> {tmp_dir}/rustdesk_root_update.log
    rm -rf "$staged_bundle"
    exit 1
fi
bundle_swapped=1
if ! mv "$staged_bundle" {app_bundle}; then
    echo "[root-update] replacement mv failed, restoring backup" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Install the entire bundle as root-owned.  The LaunchDaemon executes code
# from this bundle, so no nested framework, helper, or resource may remain
# user-writable.
if ! chown -R root:wheel {app_bundle} || ! chmod -R go-w {app_bundle}; then
    echo "[root-update] chown failed, restoring backup" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
xattr -r -d com.apple.quarantine {app_bundle} || true
# Keep root-executed files AND entire ancestor chain root-owned — prevent privilege escalation
if ! chown root:wheel {app_bundle} || \
   ! chmod 755 {app_bundle} || \
   ! chown root:wheel {app_bundle}/Contents || \
   ! chmod 755 {app_bundle}/Contents || \
   ! chown root:wheel {app_bundle}/Contents/MacOS || \
   ! chmod 755 {app_bundle}/Contents/MacOS || \
   ! chown root:wheel {app_bundle}/Contents/MacOS/service || \
   ! chmod 755 {app_bundle}/Contents/MacOS/service || \
   ! chown root:wheel {app_bundle}/Contents/MacOS/{app_name} || \
   ! chmod 755 {app_bundle}/Contents/MacOS/{app_name}; then
    echo "[root-update] hardening failed, restoring backup" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Generate launchd definitions from the new, final-location binary.  The
# subprocess is bounded and its output is retained for diagnosis; failure
# causes the existing bundle/plists to be restored by the EXIT trap.
if ! write_new_plists; then
    echo "[root-update] CRITICAL: new binary failed to write plists" >> {tmp_dir}/rustdesk_root_update.log
    cat "{tmp_dir}/write-plists.log" >> {tmp_dir}/rustdesk_root_update.log 2>/dev/null || true
    exit 1
fi
echo "[root-update] Plist definitions written by new binary" >> {tmp_dir}/rustdesk_root_update.log
# Check daemon registration and readiness BEFORE removing backup.  launchctl
# load/bootstrap only registers the job; the service can still exit immediately.
if ! launchctl load -w {daemon_plist} 2>/dev/null && \
   ! launchctl bootstrap system {daemon_plist} 2>/dev/null; then
    echo "[root-update] CRITICAL: daemon reload failed, restoring backup" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
if ! daemon_ready; then
    echo "[root-update] CRITICAL: daemon failed readiness check, restoring" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Bootstrap agent BEFORE removing backup — needed for rollback on failure.
# This also uses launchctl load for the login-window/no-console-user case.
if ! bootstrap_agents || ! agent_ready; then
    echo "[root-update] CRITICAL: agent bootstrap failed, rolling back" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Recheck daemon liveness after the agent is restored and immediately before
# deleting the only rollback bundle.
if ! daemon_snapshot_stable || ! agent_snapshot_stable; then
    echo "[root-update] CRITICAL: daemon or agent stopped before commit, restoring" >> {tmp_dir}/rustdesk_root_update.log
    exit 1
fi
# Only remove backup after BOTH daemon AND agent confirmed running
rollback_done=1
bundle_swapped=0
if ! rm -rf "{app_bundle}.bak"; then
    echo "[root-update] WARNING: committed update but could not remove backup" >> {tmp_dir}/rustdesk_root_update.log
fi
for gui_uid in $gui_uids; do
    launchctl asuser "$gui_uid" open -a "{app_bundle}" || true
done
echo "[root-update] Done!" >> {tmp_dir}/rustdesk_root_update.log
rm -rf {tmp_dir}
"#,
        app_name = app_name,
        app_bundle = app_bundle,
        src_app = src_app,
        uid_list = uid_list,
        daemon_plist = daemon_plist,
        agent_plist = agent_plist,
        tmp_dir = tmp_dir,
        daemon_label = daemon_label,
        agent_label = agent_label,
        daemon_plist_bak = daemon_plist_bak,
        agent_plist_bak = agent_plist_bak,
    );

    {
        use std::io::Write;
        if let Err(err) = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script_path)
            .and_then(|mut f| f.write_all(script.as_bytes()))
        {
            return Err(err.into());
        }
    }
    match Command::new("/bin/chmod")
        .args(&["+x", &script_path])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => {
            bail!(
                "[root-update] failed to make update script executable: {}",
                status
            );
        }
        Err(err) => {
            return Err(err.into());
        }
    }
    // Reject session changes observed before launch, but this snapshot is
    // best-effort: it is not atomic with shutdown in the detached script.
    if get_logged_in_uids() != logged_in_uids {
        bail!("[root-update] GUI session set changed before update launch");
    }
    if !crate::updater::has_no_active_conns_ipc() {
        bail!("[root-update] active session started before update launch");
    }
    if let Err(err) = Command::new("/bin/bash")
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
    {
        return Err(err.into());
    }

    log::info!("[root-update] Update script launched.");
    Ok(())
}

fn extract_dmg_into_existing_dir(dmg_path: &str, target_dir: &str) -> ResultType<()> {
    let target_path = Path::new(target_dir);
    if !target_path.exists() {
        bail!(
            "[root-update] Temp directory does not exist: {:?}",
            target_path
        );
    }
    extract_dmg_inner(dmg_path, target_dir)
}

fn verified_dmg_update_source(verified_dmg: &VerifiedDmg) -> ResultType<UpdateSource> {
    verify_stored_dmg(verified_dmg)?;
    Ok(UpdateSource::VerifiedDmg {
        path: verified_dmg_path(verified_dmg)?.to_owned(),
        expected_sha256: verified_dmg.expected_sha256.clone(),
    })
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

pub(crate) fn get_bundle_id() -> Option<String> {
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

#[cfg(test)]
mod update_tests {
    use super::*;

    fn privileged_update_scripts() -> [&'static str; 2] {
        let daemon_script = PRIVILEGES_SCRIPTS_DIR
            .get_file("update.scpt")
            .unwrap()
            .contents_utf8()
            .unwrap();
        [daemon_script, PRIVILEGED_UPDATE_BODY]
    }

    #[test]
    fn update_tree_rejects_framework_root_symlink() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-update-tree-framework-link-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let app_dir = test_dir.join("RustDesk.app");
        let frameworks_dir = app_dir.join("Contents/Frameworks");
        let external_framework = test_dir.join("External.framework");
        std::fs::create_dir_all(&frameworks_dir).unwrap();
        std::fs::create_dir_all(&external_framework).unwrap();
        std::os::unix::fs::symlink(
            &external_framework,
            frameworks_dir.join("External.framework"),
        )
        .unwrap();

        let result = validate_update_tree(&app_dir, None);

        std::fs::remove_dir_all(test_dir).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn update_tree_allows_internal_framework_symlink() {
        let test_dir = std::env::temp_dir().join(format!(
            "rustdesk-update-tree-internal-link-test-{}-{}",
            std::process::id(),
            hbb_common::rand::random::<u64>()
        ));
        let framework_dir = test_dir.join("RustDesk.app/Contents/Frameworks/Test.framework");
        let versions_dir = framework_dir.join("Versions");
        std::fs::create_dir_all(versions_dir.join("A")).unwrap();
        std::os::unix::fs::symlink("A", versions_dir.join("Current")).unwrap();

        let result = validate_update_tree(&test_dir.join("RustDesk.app"), None);

        std::fs::remove_dir_all(test_dir).unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn update_scripts_roll_back_uncommitted_bundle_swap() {
        let [daemon_script, manual_script] = privileged_update_scripts();

        for script in [daemon_script, manual_script] {
            assert!(script.contains("transaction_committed"));
            assert!(script.contains("rollback_bundle"));
            assert!(script.contains("bundle_backed_up"));
            assert!(script.contains(r#"if [ \"${rollback_status:-0}\" -eq 0 ]"#));
        }
        assert!(daemon_script
            .contains("rollback_bundle & rollback_plists & restore_service & restore_agent"));
        assert!(daemon_script.contains(
            "load_service & wait_for_service & load_agent & wait_for_agent & verify_readiness & commit_update"
        ));
        assert!(manual_script.contains("restore_installed_owner & commit_update"));
    }

    #[test]
    fn committed_update_cleanup_failure_is_non_fatal() {
        for script in privileged_update_scripts() {
            let cleanup = script
                .lines()
                .find(|line| line.contains("set cleanup_verified"))
                .unwrap();
            let rollback = script
                .lines()
                .find(|line| line.contains("set rollback_update"))
                .unwrap();

            assert!(cleanup.contains("cleanup_status=1"));
            assert!(!cleanup.contains("|| status=1"));
            assert!(rollback.contains("cleanup_status=0"));
            assert!(rollback.contains("UPDATE_CLEANUP_FAILED_AFTER_COMMIT"));
            assert!(
                rollback.contains(r#"[ \"${transaction_committed:-0}\" -ne 1 ]; then status=1"#)
            );
        }
    }

    #[test]
    fn verified_dmg_is_hashed_after_privileged_copy() {
        for script in privileged_update_scripts() {
            let copy = script.find("/bin/cp").unwrap();
            let hash = script.find("/usr/bin/shasum -a 256").unwrap();
            let attach = script.find("/usr/bin/hdiutil attach -readonly").unwrap();
            assert!(script.contains("expected_sha256"));
            assert!(copy < hash && hash < attach);
        }
    }

    #[test]
    fn daemon_update_requires_launch_agent_load() {
        let [daemon_script, _] = privileged_update_scripts();
        let bootstrap_agent = daemon_script
            .lines()
            .find(|line| line.contains("set bootstrap_agent"))
            .unwrap();

        assert!(!bootstrap_agent.contains("|| true"));
    }

    #[test]
    fn daemon_update_quotes_daemon_plist_path() {
        let [daemon_script, _] = privileged_update_scripts();

        assert!(daemon_script.contains("set daemon_plist_q to quoted form of daemon_plist"));
        assert!(!daemon_script.contains("& daemon_plist &"));
    }

    #[test]
    fn daemon_update_bounds_plist_generation() {
        let [daemon_script, _] = privileged_update_scripts();
        let write_new_plists = daemon_script
            .lines()
            .find(|line| line.contains("set write_new_plists"))
            .unwrap();

        assert!(daemon_script.contains("set write_plist_attempts to \"60\""));
        assert!(write_new_plists.contains("kill -TERM"));
        assert!(write_new_plists.contains("kill -KILL"));
        assert!(write_new_plists.contains("return 124"));
    }

    #[test]
    fn daemon_update_quotes_launchd_targets() {
        let [daemon_script, _] = privileged_update_scripts();
        let check_service = daemon_script
            .lines()
            .find(|line| line.contains("set check_service"))
            .unwrap();
        let check_agent = daemon_script
            .lines()
            .find(|line| line.contains("set check_agent"))
            .unwrap();
        let kickstart_agent = daemon_script
            .lines()
            .find(|line| line.contains("set kickstart_agent"))
            .unwrap();

        assert!(daemon_script
            .contains("set daemon_target_q to quoted form of (\"system/\" & daemon_label)"));
        assert!(check_service.contains("launchctl print \" & daemon_target_q & \""));
        for target in [
            r#"\"gui/$uid/$agent_label\""#,
            r#"\"user/$uid/$agent_label\""#,
            r#"\"system/$agent_label\""#,
        ] {
            assert!(check_agent.contains(target));
        }
        assert!(kickstart_agent.contains(r#"\"gui/$uid/$agent_label\""#));
        assert!(kickstart_agent.contains(r#"\"user/$uid/$agent_label\""#));
    }
}
