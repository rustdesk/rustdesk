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
use hbb_common::{bail, log};
use include_dir::{include_dir, Dir};
use objc::{class, msg_send, sel, sel_impl};
use scrap::{libc::c_void, quartz::ffi::*};
use std::path::PathBuf;

static PRIVILEGES_SCRIPTS_DIR: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/src/platform/privileges_scripts");
static mut LATEST_SEED: i32 = 0;

extern "C" {
    fn CGSCurrentCursorSeed() -> i32;
    fn CGEventCreate(r: *const c_void) -> *const c_void;
    fn CGEventGetLocation(e: *const c_void) -> CGPoint;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> BOOL;
    fn InputMonitoringAuthStatus(_: BOOL) -> BOOL;
}

pub fn is_process_trusted(prompt: bool) -> bool {
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

// macOS >= 10.15
// https://stackoverflow.com/questions/56597221/detecting-screen-recording-settings-on-macos-catalina/
// remove just one app from all the permissions: tccutil reset All com.carriez.rustdesk
pub fn is_can_screen_recording(prompt: bool) -> bool {
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
            Capturer::new(d, true).ok();
        }
    }
    can_record_screen
}

pub fn is_installed_daemon(prompt: bool) -> bool {
    let daemon = format!("{}_service.plist", crate::get_full_name());
    let agent = format!("{}_server.plist", crate::get_full_name());
    let agent_plist_file = format!("/Library/LaunchAgents/{}", agent);
    if !prompt {
        if !std::path::Path::new(&format!("/Library/LaunchDaemons/{}", daemon)).exists() {
            return false;
        }
        if !std::path::Path::new(&agent_plist_file).exists() {
            return false;
        }
        return true;
    }

    let install_script = PRIVILEGES_SCRIPTS_DIR.get_file("install.scpt").unwrap();
    let install_script_body = install_script.contents_utf8().unwrap();

    let daemon_plist = PRIVILEGES_SCRIPTS_DIR.get_file(&daemon).unwrap();
    let daemon_plist_body = daemon_plist.contents_utf8().unwrap();

    let agent_plist = PRIVILEGES_SCRIPTS_DIR.get_file(&agent).unwrap();
    let agent_plist_body = agent_plist.contents_utf8().unwrap();

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
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&format!(
                            "sleep 0.5; open -n /Applications/{}.app",
                            crate::get_app_name(),
                        ))
                        .spawn()
                        .ok();
                    quit_gui();
                }
            }
        }
    });
    false
}

pub fn uninstall() -> bool {
    // to-do: do together with win/linux about refactory start/stop service
    if !is_installed_daemon(false) {
        return false;
    }

    let script_file = PRIVILEGES_SCRIPTS_DIR.get_file("uninstall.scpt").unwrap();
    let script_body = script_file.contents_utf8().unwrap();

    std::thread::spawn(move || {
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
                    crate::ipc::set_option("stop-service", "Y");
                    // leave ipc a little time
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    std::process::Command::new("launchctl")
                        .args(&["remove", &format!("{}_server", crate::get_full_name())])
                        .status()
                        .ok();
                    std::process::Command::new("sh")
                        .arg("-c")
                        .arg(&format!(
                            "sleep 0.5; open /Applications/{}.app",
                            crate::get_app_name(),
                        ))
                        .spawn()
                        .ok();
                    quit_gui();
                }
            }
        }
    });
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

pub fn get_cursor() -> ResultType<Option<u64>> {
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

// https://github.com/stweil/OSXvnc/blob/master/OSXvnc-server/mousecursor.c
pub fn get_cursor_data(hcursor: u64) -> ResultType<CursorData> {
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
        // TIFF is rgb colrspace, no need to convert
        // let cs: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
        for y in 0..(size.height as _) {
            for x in 0..(size.width as _) {
                let color: id = msg_send![rep, colorAtX:x y:y];
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
    let exe = std::env::current_exe().unwrap_or_default();
    let tm0 = hbb_common::get_modified_time(&exe);
    log::info!("{}", crate::username());

    std::thread::spawn(move || loop {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(300));
            let now = hbb_common::get_modified_time(&exe);
            if now != tm0 && now != std::time::UNIX_EPOCH {
                // sleep a while to wait for resources file ready
                std::thread::sleep(std::time::Duration::from_millis(300));
                println!("{:?} updated, will restart", exe);
                // this won't kill myself
                std::process::Command::new("pkill")
                    .args(&["-f", &crate::get_app_name()])
                    .status()
                    .ok();
                println!("The others killed");
                // launchctl load/unload/start agent not work in daemon, show not priviledged.
                // sudo launchctl asuser 501 open -n also not allowed.
                std::process::Command::new("launchctl")
                    .args(&[
                        "asuser",
                        &get_active_userid(),
                        "open",
                        "-a",
                        &exe.to_str().unwrap_or(""),
                        "--args",
                        "--server",
                    ])
                    .status()
                    .ok();
                std::process::exit(0);
            }
        }
    });

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

pub fn block_input(_v: bool) -> bool {
    true
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
    use cocoa::appkit::NSApp;
    unsafe {
        let () = msg_send!(NSApp(), terminate: nil);
    };
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
