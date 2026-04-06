use crate::{
    ipc::{self, new_listener, Connection, Data, DataPortableService},
    platform::{
        set_path_permission, set_path_permission_for_portable_service_shmem_dir,
        set_path_permission_for_portable_service_shmem_file,
    },
};
use core::slice;
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail, libc, log,
    message_proto::{KeyEvent, MouseEvent},
    protobuf::Message,
    tokio::{self, sync::mpsc},
    ResultType,
};
#[cfg(feature = "vram")]
use scrap::AdapterDevice;
use scrap::{Capturer, Frame, TraitCapturer, TraitPixelBuffer};
use shared_memory::*;
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use winapi::{
    shared::minwindef::{BOOL, FALSE, TRUE},
    um::winuser::{self, CURSORINFO, PCURSORINFO},
};
use windows::Win32::Storage::FileSystem::{FILE_GENERIC_EXECUTE, FILE_GENERIC_READ};

use super::video_qos;

const SIZE_COUNTER: usize = size_of::<i32>() * 2;
const FRAME_ALIGN: usize = 64;
const IPC_TOKEN_LEN: usize = 64;

const ADDR_IPC_TOKEN: usize = 0;
const ADDR_CURSOR_PARA: usize = ADDR_IPC_TOKEN + IPC_TOKEN_LEN;
const ADDR_CURSOR_COUNTER: usize = ADDR_CURSOR_PARA + size_of::<CURSORINFO>();

const ADDR_CAPTURER_PARA: usize = ADDR_CURSOR_COUNTER + SIZE_COUNTER;
const ADDR_CAPTURE_FRAME_INFO: usize = ADDR_CAPTURER_PARA + size_of::<CapturerPara>();
const ADDR_CAPTURE_WOULDBLOCK: usize = ADDR_CAPTURE_FRAME_INFO + size_of::<FrameInfo>();
const ADDR_CAPTURE_FRAME_COUNTER: usize = ADDR_CAPTURE_WOULDBLOCK + size_of::<i32>();

const ADDR_CAPTURE_FRAME: usize =
    (ADDR_CAPTURE_FRAME_COUNTER + SIZE_COUNTER + FRAME_ALIGN - 1) / FRAME_ALIGN * FRAME_ALIGN;
const MIN_RUNTIME_SHMEM_LEN: usize = ADDR_CAPTURE_FRAME + FRAME_ALIGN;

const IPC_SUFFIX: &str = "_portable_service";
pub const SHMEM_NAME: &str = "_portable_service";
pub const SHMEM_ARG_PREFIX: &str = "--portable-service-shmem-name=";
const SHMEM_PARENT_DIR: &str = "portable_service_shmem";
const SHMEM_NAME_MAX_LEN: usize = 64;
const MAX_NACK: usize = 3;
const PORTABLE_SERVICE_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_DXGI_FAIL_TIME: usize = 5;

#[inline]
fn is_valid_portable_service_shmem_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= SHMEM_NAME_MAX_LEN
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}

#[inline]
pub fn portable_service_shmem_arg(name: &str) -> String {
    format!("{SHMEM_ARG_PREFIX}{name}")
}

#[inline]
fn is_valid_portable_service_ipc_token(token: &str) -> bool {
    token.len() == IPC_TOKEN_LEN
        && token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[inline]
fn read_ipc_token_from_shmem(shmem: &SharedMemory) -> Option<String> {
    if shmem.len() < ADDR_IPC_TOKEN + IPC_TOKEN_LEN {
        log::error!(
            "Portable service shared memory too small: len={}, need>={}",
            shmem.len(),
            ADDR_IPC_TOKEN + IPC_TOKEN_LEN
        );
        return None;
    }
    unsafe {
        let ptr = shmem.as_ptr().add(ADDR_IPC_TOKEN);
        let bytes = slice::from_raw_parts(ptr, IPC_TOKEN_LEN);
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(IPC_TOKEN_LEN);
        if end == 0 {
            return None;
        }
        let token = std::str::from_utf8(&bytes[..end]).ok()?.to_owned();
        if is_valid_portable_service_ipc_token(&token) {
            Some(token)
        } else {
            None
        }
    }
}

#[inline]
fn validate_runtime_shmem_layout(shmem: &SharedMemory) -> ResultType<()> {
    if shmem.len() < MIN_RUNTIME_SHMEM_LEN {
        bail!(
            "Portable service shared memory too small for runtime layout: len={}, need>={}",
            shmem.len(),
            MIN_RUNTIME_SHMEM_LEN
        );
    }
    Ok(())
}

#[inline]
fn is_valid_capture_frame_length(shmem_len: usize, frame_len: usize) -> bool {
    let frame_capacity = shmem_len.saturating_sub(ADDR_CAPTURE_FRAME);
    frame_len > 0 && frame_len <= frame_capacity
}

#[inline]
fn shared_memory_flink_path_by_name(name: &str) -> ResultType<PathBuf> {
    let mut dir = crate::platform::user_accessible_folder()?;
    dir = dir.join(hbb_common::config::APP_NAME.read().unwrap().clone());
    dir = dir.join(SHMEM_PARENT_DIR);
    Ok(dir.join(format!("shared_memory{}", name)))
}

#[inline]
fn remove_shared_memory_flink_once(name: &str, log_on_error: bool, log_context: &str) -> bool {
    let flink = match shared_memory_flink_path_by_name(name) {
        Ok(path) => path,
        Err(err) => {
            if log_on_error {
                log::warn!(
                    "{} failed to resolve portable service shared-memory flink path for '{}': {}",
                    log_context,
                    name,
                    err
                );
            }
            return false;
        }
    };
    match std::fs::remove_file(&flink) {
        Ok(()) => {
            log::info!(
                "{} removed portable service shared-memory flink artifact: {:?}",
                log_context,
                flink
            );
            true
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => {
            if log_on_error {
                log::warn!(
                    "{} failed to remove portable service shared-memory flink artifact {:?}: {}",
                    log_context,
                    flink,
                    err
                );
            }
            false
        }
    }
}

#[inline]
fn write_ipc_token_to_shmem(shmem: &SharedMemory, token: &str) -> ResultType<()> {
    if !is_valid_portable_service_ipc_token(token) {
        bail!("Invalid portable service ipc token");
    }
    shmem.write(ADDR_IPC_TOKEN, token.as_bytes());
    Ok(())
}

#[inline]
fn clear_ipc_token_in_shmem(shmem: &SharedMemory) {
    shmem.write(ADDR_IPC_TOKEN, &[0u8; IPC_TOKEN_LEN]);
}

#[inline]
fn portable_service_arg_value_candidate_from_arg<'a>(
    arg: &'a str,
    prefix: &str,
) -> Option<&'a str> {
    let mut value = arg.strip_prefix(prefix)?;
    value = value.trim_start();
    value = value
        .strip_prefix('"')
        .or_else(|| value.strip_prefix('\''))
        .unwrap_or(value);
    value = value.split_whitespace().next().unwrap_or_default();
    value = value.trim_matches(|c| c == '"' || c == '\'');
    Some(value)
}

#[inline]
pub fn portable_service_shmem_name_from_args() -> Option<String> {
    for arg in std::env::args() {
        if let Some(value) = portable_service_arg_value_candidate_from_arg(&arg, SHMEM_ARG_PREFIX) {
            if is_valid_portable_service_shmem_name(value) {
                return Some(value.to_owned());
            }
            log::error!(
                "Invalid portable service shared memory name argument: '{}'",
                value
            );
            return None;
        }
    }
    None
}

#[inline]
pub fn has_portable_service_shmem_arg() -> bool {
    std::env::args().any(|arg| arg.starts_with(SHMEM_ARG_PREFIX))
}

pub struct SharedMemory {
    inner: Shmem,
}

unsafe impl Send for SharedMemory {}
unsafe impl Sync for SharedMemory {}

impl Deref for SharedMemory {
    type Target = Shmem;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for SharedMemory {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl SharedMemory {
    pub fn create(name: &str, size: usize) -> ResultType<Self> {
        let flink = Self::flink(name.to_string())?;
        let shmem = match ShmemConf::new()
            .size(size)
            .flink(&flink)
            .force_create_flink()
            .create()
        {
            Ok(m) => m,
            Err(ShmemError::LinkExists) => {
                bail!(
                    "Unable to force create shmem flink {}, which should not happen.",
                    flink
                )
            }
            Err(e) => {
                bail!("Unable to create shmem flink {} : {}", flink, e);
            }
        };
        log::info!("Create shared memory, size: {}, flink: {}", size, flink);
        if let Err(err) = set_path_permission_for_portable_service_shmem_file(Path::new(&flink)) {
            // Release shmem handle first so best-effort flink cleanup has a chance to succeed.
            drop(shmem);
            match std::fs::remove_file(&flink) {
                Ok(()) => {
                    log::info!(
                        "Create cleanup removed portable service shared-memory flink artifact: {}",
                        flink
                    );
                }
                Err(remove_err) if remove_err.kind() == std::io::ErrorKind::NotFound => {}
                Err(remove_err) => {
                    log::warn!(
                        "Create cleanup failed to remove portable service shared-memory flink artifact {}: {}",
                        flink,
                        remove_err
                    );
                }
            }
            return Err(err);
        }
        Ok(SharedMemory { inner: shmem })
    }

    pub fn open_existing(name: &str) -> ResultType<Self> {
        let flink = Self::flink(name.to_string())?;
        let shmem = match ShmemConf::new().flink(&flink).allow_raw(true).open() {
            Ok(m) => m,
            Err(e) => {
                bail!("Unable to open existing shmem flink {} : {}", flink, e);
            }
        };
        log::info!("open existing shared memory, flink: {:?}", flink);
        Ok(SharedMemory { inner: shmem })
    }

    pub fn write(&self, addr: usize, data: &[u8]) {
        unsafe {
            debug_assert!(addr + data.len() <= self.inner.len());
            let ptr = self.inner.as_ptr().add(addr);
            let shared_mem_slice = slice::from_raw_parts_mut(ptr, data.len());
            shared_mem_slice.copy_from_slice(data);
        }
    }

    fn flink(name: String) -> ResultType<String> {
        let mut dir = crate::platform::user_accessible_folder()?;
        dir = dir.join(hbb_common::config::APP_NAME.read().unwrap().clone());
        dir = dir.join(SHMEM_PARENT_DIR);
        let parent_created = !dir.exists();
        if parent_created {
            std::fs::create_dir_all(&dir)?;
        }
        if parent_created || crate::platform::is_root() {
            // Harden parent ACL on first provisioning and periodically on SYSTEM path.
            set_path_permission_for_portable_service_shmem_dir(&dir)?;
        }
        Ok(dir
            .join(format!("shared_memory{}", name))
            .to_string_lossy()
            .to_string())
    }
}

mod utils {
    use core::slice;
    use std::mem::size_of;

    use super::{
        CapturerPara, FrameInfo, SharedMemory, ADDR_CAPTURER_PARA, ADDR_CAPTURE_FRAME_INFO,
    };

    #[inline]
    pub fn i32_to_vec(i: i32) -> Vec<u8> {
        i.to_ne_bytes().to_vec()
    }

    #[inline]
    pub fn ptr_to_i32(ptr: *const u8) -> i32 {
        unsafe {
            let v = slice::from_raw_parts(ptr, size_of::<i32>());
            i32::from_ne_bytes([v[0], v[1], v[2], v[3]])
        }
    }

    #[inline]
    pub fn counter_ready(counter: *const u8) -> bool {
        unsafe {
            let wptr = counter;
            let rptr = counter.add(size_of::<i32>());
            let iw = ptr_to_i32(wptr);
            let ir = ptr_to_i32(rptr);
            if ir != iw {
                std::ptr::copy_nonoverlapping(wptr, rptr as *mut _, size_of::<i32>());
                true
            } else {
                false
            }
        }
    }

    #[inline]
    pub fn counter_equal(counter: *const u8) -> bool {
        unsafe {
            let wptr = counter;
            let rptr = counter.add(size_of::<i32>());
            let iw = ptr_to_i32(wptr);
            let ir = ptr_to_i32(rptr);
            iw == ir
        }
    }

    #[inline]
    pub fn increase_counter(counter: *mut u8) {
        unsafe {
            let wptr = counter;
            let rptr = counter.add(size_of::<i32>());
            let iw = ptr_to_i32(counter);
            let ir = ptr_to_i32(counter);
            let iw_plus1 = if iw == i32::MAX { 0 } else { iw + 1 };
            let v = i32_to_vec(iw_plus1);
            std::ptr::copy_nonoverlapping(v.as_ptr(), wptr, size_of::<i32>());
            if ir == iw_plus1 {
                let v = i32_to_vec(iw);
                std::ptr::copy_nonoverlapping(v.as_ptr(), rptr, size_of::<i32>());
            }
        }
    }

    #[inline]
    pub fn align(v: usize, align: usize) -> usize {
        (v + align - 1) / align * align
    }

    #[inline]
    pub fn set_para(shmem: &SharedMemory, para: CapturerPara) {
        let para_ptr = &para as *const CapturerPara as *const u8;
        let para_data;
        unsafe {
            para_data = slice::from_raw_parts(para_ptr, size_of::<CapturerPara>());
        }
        shmem.write(ADDR_CAPTURER_PARA, para_data);
    }

    #[inline]
    pub fn set_frame_info(shmem: &SharedMemory, info: FrameInfo) {
        let ptr = &info as *const FrameInfo as *const u8;
        let data;
        unsafe {
            data = slice::from_raw_parts(ptr, size_of::<FrameInfo>());
        }
        shmem.write(ADDR_CAPTURE_FRAME_INFO, data);
    }
}

// functions called in separate SYSTEM user process.
pub mod server {
    use hbb_common::message_proto::PointerDeviceEvent;

    use crate::display_service;

    use super::*;

    lazy_static::lazy_static! {
        static ref EXIT: Arc<Mutex<bool>> = Default::default();
        static ref FORCE_EXIT_ARMED: AtomicBool = AtomicBool::new(false);
    }

    pub fn run_portable_service() {
        let shmem_name = match portable_service_shmem_name_from_args() {
            Some(name) => name,
            None => {
                if has_portable_service_shmem_arg() {
                    log::error!(
                        "Invalid portable service shared memory argument, aborting startup"
                    );
                } else {
                    log::error!(
                        "Missing portable service shared memory argument, aborting startup"
                    );
                }
                return;
            }
        };
        let shmem = match SharedMemory::open_existing(&shmem_name) {
            Ok(shmem) => Arc::new(shmem),
            Err(e) => {
                log::error!("Failed to open existing shared memory: {:?}", e);
                return;
            }
        };
        if let Err(e) = validate_runtime_shmem_layout(shmem.as_ref()) {
            log::error!("{}", e);
            return;
        }
        let ipc_token = match read_ipc_token_from_shmem(shmem.as_ref()) {
            Some(token) => token,
            None => {
                log::error!(
                    "Missing portable service ipc token in shared memory, aborting startup"
                );
                return;
            }
        };
        let shmem1 = shmem.clone();
        let shmem2 = shmem.clone();
        let mut threads = vec![];
        threads.push(std::thread::spawn(|| {
            run_get_cursor_info(shmem1);
        }));
        threads.push(std::thread::spawn(|| {
            run_capture(shmem2);
        }));
        threads.push(std::thread::spawn(move || {
            run_ipc_client(ipc_token);
        }));
        // Detached shutdown watchdog:
        // - gives graceful shutdown/cleanup a short window
        // - force-exits the process if workers are still stuck
        std::thread::spawn(|| {
            run_exit_check();
        });
        let record_pos_handle = crate::input_service::try_start_record_cursor_pos();
        // Arm forced-exit watchdog only for worker join phase.
        // Once join phase completes, cleanup should not be interrupted by forced exit.
        FORCE_EXIT_ARMED.store(true, Ordering::SeqCst);
        for th in threads.drain(..) {
            th.join().ok();
            log::info!("thread joined");
        }
        FORCE_EXIT_ARMED.store(false, Ordering::SeqCst);

        crate::input_service::try_stop_record_cursor_pos();
        if let Some(handle) = record_pos_handle {
            match handle.join() {
                Ok(_) => log::info!("record_pos_handle joined"),
                Err(e) => log::error!("record_pos_handle join error {:?}", &e),
            }
        }
        drop(shmem);
        remove_shared_memory_flink_with_retry(&shmem_name);
    }

    fn run_exit_check() {
        const FORCED_EXIT_DELAY: Duration = Duration::from_secs(3);
        loop {
            if EXIT.lock().unwrap().clone() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        // Fallback only: normal shutdown path should complete and process should exit naturally.
        // This forced exit is a last resort when worker threads are stuck and graceful teardown
        // does not finish in time.
        std::thread::sleep(FORCED_EXIT_DELAY);
        if FORCE_EXIT_ARMED.load(Ordering::SeqCst) {
            log::warn!(
                "Portable service shutdown watchdog fallback triggered: forcing process exit after {:?}",
                FORCED_EXIT_DELAY
            );
            std::process::exit(0);
        }
    }

    fn remove_shared_memory_flink_with_retry(name: &str) {
        const MAX_RETRY: usize = 20;
        const RETRY_INTERVAL: Duration = Duration::from_millis(200);
        for attempt in 0..MAX_RETRY {
            let is_last_attempt = attempt + 1 == MAX_RETRY;
            if remove_shared_memory_flink_once(name, is_last_attempt, "SYSTEM cleanup") {
                return;
            }
            if !is_last_attempt {
                std::thread::sleep(RETRY_INTERVAL);
            }
        }
        log::warn!(
            "SYSTEM cleanup failed to remove portable service shared-memory flink artifact '{}' after retry",
            name
        );
    }

    fn run_get_cursor_info(shmem: Arc<SharedMemory>) {
        loop {
            if EXIT.lock().unwrap().clone() {
                break;
            }
            unsafe {
                let para = shmem.as_ptr().add(ADDR_CURSOR_PARA) as *mut CURSORINFO;
                (*para).cbSize = size_of::<CURSORINFO>() as _;
                let result = winuser::GetCursorInfo(para);
                if result == TRUE {
                    utils::increase_counter(shmem.as_ptr().add(ADDR_CURSOR_COUNTER));
                }
            }
            // more frequent in case of `Error of mouse_cursor service`
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    fn run_capture(shmem: Arc<SharedMemory>) {
        let mut c = None;
        let mut last_current_display = usize::MAX;
        let mut last_timeout_ms: i32 = 33;
        let mut spf = Duration::from_millis(last_timeout_ms as _);
        let mut first_frame_captured = false;
        let mut dxgi_failed_times = 0;
        let mut display_width = 0;
        let mut display_height = 0;
        loop {
            if EXIT.lock().unwrap().clone() {
                break;
            }
            unsafe {
                let para_ptr = shmem.as_ptr().add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                let recreate = (*para).recreate;
                let current_display = (*para).current_display;
                let timeout_ms = (*para).timeout_ms;
                if c.is_none() {
                    let Ok(mut displays) = display_service::try_get_displays() else {
                        log::error!("Failed to get displays");
                        *EXIT.lock().unwrap() = true;
                        return;
                    };
                    if displays.len() <= current_display {
                        log::error!("Invalid display index:{}", current_display);
                        *EXIT.lock().unwrap() = true;
                        return;
                    }
                    let display = displays.remove(current_display);
                    display_width = display.width();
                    display_height = display.height();
                    match Capturer::new(display) {
                        Ok(mut v) => {
                            c = {
                                last_current_display = current_display;
                                first_frame_captured = false;
                                if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                                    dxgi_failed_times = 0;
                                    v.set_gdi();
                                }
                                utils::set_para(
                                    &shmem,
                                    CapturerPara {
                                        recreate: false,
                                        current_display: (*para).current_display,
                                        timeout_ms: (*para).timeout_ms,
                                    },
                                );
                                Some(v)
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create gdi capturer: {:?}", e);
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                    }
                } else {
                    if recreate || current_display != last_current_display {
                        log::info!(
                            "create capturer, display: {} -> {}",
                            last_current_display,
                            current_display,
                        );
                        c = None;
                        continue;
                    }
                    if timeout_ms != last_timeout_ms
                        && timeout_ms >= 1000 / video_qos::MAX_FPS as i32
                        && timeout_ms <= 1000 / video_qos::MIN_FPS as i32
                    {
                        last_timeout_ms = timeout_ms;
                        spf = Duration::from_millis(timeout_ms as _);
                    }
                }
                if first_frame_captured {
                    if !utils::counter_equal(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER)) {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                        continue;
                    }
                }
                match c.as_mut().map(|f| f.frame(spf)) {
                    Some(Ok(f)) => match f {
                        Frame::PixelBuffer(f) => {
                            let frame_capacity = shmem.len().saturating_sub(ADDR_CAPTURE_FRAME);
                            if f.data().len() > frame_capacity {
                                log::error!(
                                    "Portable service capture frame exceeds shared memory capacity: frame_len={}, capacity={}, shmem_len={}",
                                    f.data().len(),
                                    frame_capacity,
                                    shmem.len()
                                );
                                *EXIT.lock().unwrap() = true;
                                return;
                            }
                            utils::set_frame_info(
                                &shmem,
                                FrameInfo {
                                    length: f.data().len(),
                                    width: display_width,
                                    height: display_height,
                                },
                            );
                            shmem.write(ADDR_CAPTURE_FRAME, f.data());
                            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                            utils::increase_counter(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER));
                            first_frame_captured = true;
                            dxgi_failed_times = 0;
                        }
                        Frame::Texture(_) => {
                            // should not happen
                        }
                    },
                    Some(Err(e)) => {
                        if crate::platform::windows::desktop_changed() {
                            crate::platform::try_change_desktop();
                            c = None;
                            std::thread::sleep(spf);
                            continue;
                        }
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            // DXGI_ERROR_INVALID_CALL after each success on Microsoft GPU driver
                            // log::error!("capture frame failed: {:?}", e);
                            if c.as_ref().map(|c| c.is_gdi()) == Some(false) {
                                // nog gdi
                                dxgi_failed_times += 1;
                            }
                            if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                                c = None;
                                shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(FALSE));
                                std::thread::sleep(spf);
                            }
                        } else {
                            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                        }
                    }
                    _ => {
                        println!("unreachable!");
                    }
                }
            }
        }
    }

    #[tokio::main(flavor = "current_thread")]
    async fn run_ipc_client(ipc_token: String) {
        use DataPortableService::*;

        let postfix = IPC_SUFFIX;

        match ipc::connect(1000, postfix).await {
            Ok(mut stream) => {
                if let Err(err) =
                    ipc::portable_service_ipc_handshake_as_client(&mut stream, &ipc_token).await
                {
                    log::error!("portable service ipc handshake failed: {}", err);
                    *EXIT.lock().unwrap() = true;
                    return;
                }
                let mut timer =
                    crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
                let mut nack = 0;
                loop {
                    if *EXIT.lock().unwrap() {
                        log::info!("Portable service EXIT signaled, closing ipc client loop");
                        stream
                            .send(&Data::DataPortableService(WillClose))
                            .await
                            .ok();
                        break;
                    }

                    tokio::select! {
                        res = stream.next() => {
                            match res {
                                Err(err) => {
                                    log::error!(
                                        "ipc{} connection closed: {}",
                                        postfix,
                                        err
                                    );
                                    break;
                                }
                                Ok(Some(Data::DataPortableService(data))) => match data {
                                    Ping => {
                                        allow_err!(
                                            stream
                                                .send(&Data::DataPortableService(Pong))
                                                .await
                                        );
                                    }
                                    Pong => {
                                        nack = 0;
                                    }
                                    ConnCount(Some(n)) => {
                                        if n == 0 {
                                            log::info!("Connection count equals 0, exit");
                                            stream.send(&Data::DataPortableService(WillClose)).await.ok();
                                            break;
                                        }
                                    }
                                    Mouse((v, conn, username, argb, simulate, show_cursor)) => {
                                        if let Ok(evt) = MouseEvent::parse_from_bytes(&v) {
                                            crate::input_service::handle_mouse_(&evt, conn, username, argb, simulate, show_cursor);
                                        }
                                    }
                                    Pointer((v, conn)) => {
                                        if let Ok(evt) = PointerDeviceEvent::parse_from_bytes(&v) {
                                            crate::input_service::handle_pointer_(&evt, conn);
                                        }
                                    }
                                    Key(v) => {
                                        if let Ok(evt) = KeyEvent::parse_from_bytes(&v) {
                                            crate::input_service::handle_key_(&evt);
                                        }
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                        _ = timer.tick() => {
                            nack+=1;
                            if nack > MAX_NACK {
                                log::info!("max ping nack, exit");
                                break;
                            }
                            stream.send(&Data::DataPortableService(Ping)).await.ok();
                            stream.send(&Data::DataPortableService(ConnCount(None))).await.ok();
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect portable service ipc: {:?}", e);
            }
        }

        *EXIT.lock().unwrap() = true;
    }
}

// functions called in main process.
pub mod client {
    use super::*;
    use crate::display_service;
    use hbb_common::{anyhow::Context, message_proto::PointerDeviceEvent};
    use scrap::PixelBuffer;

    lazy_static::lazy_static! {
        static ref RUNNING: Arc<Mutex<bool>> = Default::default();
        static ref STARTING: Arc<Mutex<bool>> = Default::default();
        static ref STARTING_TOKEN: AtomicU64 = AtomicU64::new(0);
        static ref SHMEM: Arc<Mutex<Option<SharedMemory>>> = Default::default();
        static ref SHMEM_RUNTIME_NAME: Arc<Mutex<Option<String>>> = Default::default();
        static ref IPC_RUNTIME_TOKEN: Arc<Mutex<Option<String>>> = Default::default();
        static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(client::start_ipc_server());
        static ref QUICK_SUPPORT: Arc<Mutex<bool>> = Default::default();
    }

    pub enum StartPara {
        Direct,
        Logon(String, String),
    }

    fn has_running_portable_service_process() -> bool {
        let app_exe = format!("{}.exe", crate::get_app_name().to_lowercase());
        !crate::platform::get_pids_of_process_with_first_arg(&app_exe, "--portable-service")
            .is_empty()
    }

    #[inline]
    fn next_portable_service_shmem_name() -> String {
        format!(
            "{}_{}_{:08x}",
            crate::portable_service::SHMEM_NAME,
            std::process::id(),
            hbb_common::rand::random::<u32>()
        )
    }

    #[inline]
    fn set_runtime_ipc_token(token: String) {
        *IPC_RUNTIME_TOKEN.lock().unwrap() = Some(token);
    }

    #[inline]
    fn schedule_remove_runtime_shmem_flink_retry(name: String) {
        std::thread::spawn(move || {
            const MAX_RETRY: usize = 20;
            const RETRY_INTERVAL: Duration = Duration::from_millis(200);
            for _ in 0..MAX_RETRY {
                std::thread::sleep(RETRY_INTERVAL);
                if remove_shared_memory_flink_once(&name, false, "Client cleanup") {
                    return;
                }
            }
            log::warn!(
                "Failed to remove portable service shared-memory flink artifact '{}' after retry",
                name
            );
        });
    }

    #[inline]
    fn clear_runtime_shmem_state() {
        let mut runtime_token = IPC_RUNTIME_TOKEN.lock().unwrap();
        let mut shmem_lock = SHMEM.lock().unwrap();
        if let Some(shmem) = shmem_lock.as_mut() {
            clear_ipc_token_in_shmem(shmem);
        }
        *shmem_lock = None;
        let runtime_name = SHMEM_RUNTIME_NAME.lock().unwrap().take();
        *runtime_token = None;
        drop(runtime_token);
        drop(shmem_lock);
        if let Some(name) = runtime_name.as_deref() {
            if !remove_shared_memory_flink_once(name, true, "Client cleanup") {
                schedule_remove_runtime_shmem_flink_retry(name.to_owned());
            }
        }
    }

    #[inline]
    fn consume_runtime_ipc_token_if_match(candidate: &str) -> (bool, Option<String>) {
        let mut token = IPC_RUNTIME_TOKEN.lock().unwrap();
        if token.as_deref() != Some(candidate) {
            return (false, None);
        }
        let mut shmem_lock = SHMEM.lock().unwrap();
        let matched_shmem_name = SHMEM_RUNTIME_NAME.lock().unwrap().clone();
        *token = None;
        if let Some(shmem) = shmem_lock.as_mut() {
            clear_ipc_token_in_shmem(shmem);
        }
        (true, matched_shmem_name)
    }

    #[inline]
    fn restore_runtime_ipc_token_after_failed_handshake(
        token: &str,
        expected_shmem_name: Option<&str>,
    ) {
        let mut runtime_token = IPC_RUNTIME_TOKEN.lock().unwrap();
        if let Some(current) = runtime_token.as_deref() {
            if current != token {
                log::debug!(
                    "Skip restoring portable service ipc token after handshake failure: runtime token has changed to a newer value"
                );
                return;
            }
        }
        let mut shmem_lock = SHMEM.lock().unwrap();
        let current_shmem_name = SHMEM_RUNTIME_NAME.lock().unwrap().clone();
        if current_shmem_name.as_deref() != expected_shmem_name {
            if runtime_token.as_deref() == Some(token) {
                *runtime_token = None;
            }
            log::debug!(
                "Skip restoring portable service ipc token after handshake failure: shared-memory instance has changed"
            );
            return;
        }
        let shmem_write_error = if let Some(shmem) = shmem_lock.as_mut() {
            write_ipc_token_to_shmem(shmem, token)
                .err()
                .map(|err| err.to_string())
        } else {
            Some("shared memory unavailable".to_owned())
        };
        if let Some(err) = shmem_write_error {
            if runtime_token.as_deref() == Some(token) {
                *runtime_token = None;
            }
            log::warn!(
                "Failed to restore portable service ipc token after handshake failure: {}",
                err
            );
            return;
        }
        *runtime_token = Some(token.to_owned());
    }

    #[inline]
    fn schedule_starting_timeout_reset(launch_token: u64) {
        std::thread::spawn(move || {
            std::thread::sleep(PORTABLE_SERVICE_STARTUP_TIMEOUT);
            let should_reset = {
                // Guard against stale watchdogs from previous launches:
                // only the watchdog that matches the latest STARTING_TOKEN may reset STARTING.
                let current_token = STARTING_TOKEN.load(Ordering::SeqCst);
                // Keep lock guards in explicit short scopes to make it obvious
                // there is no nested lock ordering (and to avoid Copilot false positives).
                let starting = { *STARTING.lock().unwrap() };
                let running = { *RUNNING.lock().unwrap() };
                current_token == launch_token && starting && !running
            };
            if should_reset {
                log::warn!(
                    "Portable service startup timeout before IPC ready, reset STARTING state"
                );
                *STARTING.lock().unwrap() = false;
            }
        });
    }

    // Launch flow summary:
    // 1) Prepare/reset runtime shared memory + IPC token.
    // 2) Start helper process (direct or logon) with shmem argument.
    // 3) Keep STARTING=true until IPC ping/pong marks RUNNING, or timeout watchdog resets it.
    pub(crate) fn start_portable_service(para: StartPara) -> ResultType<()> {
        log::info!("start portable service");
        let launch_token = {
            // Keep lock guards in explicit short scopes to make it obvious
            // there is no nested lock ordering (and to avoid Copilot false positives).
            let running = { *RUNNING.lock().unwrap() };
            let mut starting = STARTING.lock().unwrap();
            if *starting && !running && !has_running_portable_service_process() {
                log::warn!(
                    "Detected stale portable service STARTING state without running process, reset it"
                );
                *starting = false;
            }
            if *starting || running {
                bail!("already running");
            }
            *starting = true;
            STARTING_TOKEN.fetch_add(1, Ordering::SeqCst) + 1
        };
        let start_result = (|| -> ResultType<()> {
            clear_runtime_shmem_state();
            let mut shmem_lock = SHMEM.lock().unwrap();
            let displays = scrap::Display::all()?;
            if displays.is_empty() {
                bail!("no display available!");
            }
            let mut max_pixel = 0;
            let align = 64;
            for d in displays {
                let resolutions = crate::platform::resolutions(&d.name());
                for r in resolutions {
                    let pixel =
                        utils::align(r.width as _, align) * utils::align(r.height as _, align);
                    if max_pixel < pixel {
                        max_pixel = pixel;
                    }
                }
            }
            let shmem_size =
                utils::align(ADDR_CAPTURE_FRAME + max_pixel * 4, align).max(MIN_RUNTIME_SHMEM_LEN);
            let shmem_name = next_portable_service_shmem_name();
            if !is_valid_portable_service_shmem_name(&shmem_name) {
                bail!("Generated invalid portable service shared memory name");
            }
            // os error 112, no enough space
            *shmem_lock = Some(crate::portable_service::SharedMemory::create(
                &shmem_name,
                shmem_size,
            )?);
            *SHMEM_RUNTIME_NAME.lock().unwrap() = Some(shmem_name);
            shutdown_hooks::add_shutdown_hook(drop_portable_service_shared_memory);
            let shmem_name = SHMEM_RUNTIME_NAME
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow!("portable service shared memory name is unavailable"))?;
            let ipc_token = ipc::generate_one_time_ipc_token();
            let init_token_result = if let Some(shmem) = shmem_lock.as_mut() {
                unsafe {
                    libc::memset(shmem.as_ptr() as _, 0, shmem.len() as _);
                }
                write_ipc_token_to_shmem(shmem, &ipc_token)
            } else {
                Ok(())
            };
            if let Err(e) = init_token_result {
                drop(shmem_lock);
                clear_runtime_shmem_state();
                bail!(
                    "Failed to initialize portable service ipc token in shared memory: {}",
                    e
                );
            };
            drop(shmem_lock);
            set_runtime_ipc_token(ipc_token.clone());
            let portable_service_arg = format!(
                "--portable-service {}",
                crate::portable_service::portable_service_shmem_arg(&shmem_name)
            );
            {
                let _sender = SENDER.lock().unwrap();
            }
            match para {
                StartPara::Direct => {
                    match crate::platform::run_background(
                        &std::env::current_exe()?.to_string_lossy().to_string(),
                        &portable_service_arg,
                    ) {
                        Ok(true) => {}
                        Ok(false) => {
                            clear_runtime_shmem_state();
                            bail!("Failed to run portable service process");
                        }
                        Err(e) => {
                            clear_runtime_shmem_state();
                            bail!("Failed to run portable service process: {}", e);
                        }
                    }
                }
                StartPara::Logon(username, password) => {
                    #[allow(unused_mut)]
                    let mut exe = std::env::current_exe()?.to_string_lossy().to_string();
                    #[cfg(feature = "flutter")]
                    {
                        if let Some(dir) = Path::new(&exe).parent() {
                            if let Err(err) = set_path_permission(
                                Path::new(dir),
                                FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0,
                            ) {
                                clear_runtime_shmem_state();
                                bail!("Failed to set permission of {:?}: {}", dir, err);
                            }
                        }
                    }
                    #[cfg(not(feature = "flutter"))]
                    if let Some((dir, dst)) =
                        crate::platform::windows::portable_service_logon_helper_paths()
                    {
                        let cleanup_helper_artifacts = || {
                            if Path::new(&exe) != dst {
                                std::fs::remove_file(&dst).ok();
                            }
                            std::fs::remove_dir(&dir).ok();
                        };
                        let mut use_logon_helper_exe = false;
                        if let Err(err) = std::fs::create_dir_all(&dir) {
                            log::warn!(
                                "Failed to create portable service logon helper dir {:?}: {}",
                                dir,
                                err
                            );
                        } else if let Err(err) = std::fs::copy(&exe, &dst) {
                            log::warn!(
                                "Failed to copy portable service logon helper binary from '{}' to {:?}: {}",
                                exe,
                                dst,
                                err
                            );
                            cleanup_helper_artifacts();
                        } else if !dst.exists() {
                            log::warn!(
                                "Portable service logon helper binary missing after copy: {:?}",
                                dst
                            );
                            cleanup_helper_artifacts();
                        } else if let Err(err) =
                            set_path_permission(&dir, FILE_GENERIC_READ.0 | FILE_GENERIC_EXECUTE.0)
                        {
                            log::warn!(
                                "Failed to set portable service logon helper path permission for {:?}: {}",
                                dir,
                                err
                            );
                            cleanup_helper_artifacts();
                        } else {
                            use_logon_helper_exe = true;
                        }
                        if use_logon_helper_exe {
                            exe = dst.to_string_lossy().to_string();
                        }
                    }
                    if let Err(e) = crate::platform::windows::create_process_with_logon(
                        username.as_str(),
                        password.as_str(),
                        &exe,
                        &portable_service_arg,
                    ) {
                        clear_runtime_shmem_state();
                        bail!("Failed to run portable service process: {}", e);
                    }
                }
            }
            schedule_starting_timeout_reset(launch_token);
            Ok(())
        })();
        if start_result.is_err() {
            *STARTING.lock().unwrap() = false;
        }
        start_result
    }

    pub extern "C" fn drop_portable_service_shared_memory() {
        // https://stackoverflow.com/questions/35980148/why-does-an-atexit-handler-panic-when-it-accesses-stdout
        // Please make sure there is no print in the call stack
        clear_runtime_shmem_state();
    }

    pub fn set_quick_support(v: bool) {
        *QUICK_SUPPORT.lock().unwrap() = v;
    }

    pub struct CapturerPortable {
        width: usize,
        height: usize,
    }

    impl CapturerPortable {
        pub fn new(current_display: usize) -> Self
        where
            Self: Sized,
        {
            let mut option = SHMEM.lock().unwrap();
            if let Some(shmem) = option.as_mut() {
                unsafe {
                    libc::memset(
                        shmem.as_ptr().add(ADDR_CURSOR_PARA) as _,
                        0,
                        shmem.len().saturating_sub(ADDR_CURSOR_PARA) as _,
                    );
                }
                utils::set_para(
                    shmem,
                    CapturerPara {
                        recreate: true,
                        current_display,
                        timeout_ms: 33,
                    },
                );
                shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
            }
            let (mut width, mut height) = (0, 0);
            if let Ok(displays) = display_service::try_get_displays() {
                if let Some(display) = displays.get(current_display) {
                    width = display.width();
                    height = display.height();
                }
            }
            CapturerPortable { width, height }
        }
    }

    impl TraitCapturer for CapturerPortable {
        fn frame<'a>(&'a mut self, timeout: Duration) -> std::io::Result<Frame<'a>> {
            let mut lock = SHMEM.lock().unwrap();
            let shmem = lock.as_mut().ok_or(std::io::Error::new(
                std::io::ErrorKind::Other,
                "shmem dropped".to_string(),
            ))?;
            unsafe {
                let base = shmem.as_ptr();
                let para_ptr = base.add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                if timeout.as_millis() != (*para).timeout_ms as _ {
                    utils::set_para(
                        shmem,
                        CapturerPara {
                            recreate: (*para).recreate,
                            current_display: (*para).current_display,
                            timeout_ms: timeout.as_millis() as _,
                        },
                    );
                }
                if utils::counter_ready(base.add(ADDR_CAPTURE_FRAME_COUNTER)) {
                    let frame_info_ptr = shmem.as_ptr().add(ADDR_CAPTURE_FRAME_INFO);
                    let frame_info = frame_info_ptr as *const FrameInfo;
                    let frame_len = (*frame_info).length;
                    if !is_valid_capture_frame_length(shmem.len(), frame_len) {
                        log::error!(
                            "Portable service frame length exceeds shared memory capacity: frame_len={}, shmem_len={}, frame_addr={}",
                            frame_len,
                            shmem.len(),
                            ADDR_CAPTURE_FRAME
                        );
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "invalid portable service frame length".to_string(),
                        ));
                    }
                    if (*frame_info).width != self.width || (*frame_info).height != self.height {
                        log::info!(
                            "skip frame, ({},{}) != ({},{})",
                            (*frame_info).width,
                            (*frame_info).height,
                            self.width,
                            self.height,
                        );
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "wouldblock error".to_string(),
                        ));
                    }
                    let frame_ptr = base.add(ADDR_CAPTURE_FRAME);
                    let data = slice::from_raw_parts(frame_ptr, frame_len);
                    Ok(Frame::PixelBuffer(PixelBuffer::with_BGRA(
                        data,
                        self.width,
                        self.height,
                    )))
                } else {
                    let ptr = base.add(ADDR_CAPTURE_WOULDBLOCK);
                    let wouldblock = utils::ptr_to_i32(ptr);
                    if wouldblock == TRUE {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::WouldBlock,
                            "wouldblock error".to_string(),
                        ))
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "other error".to_string(),
                        ))
                    }
                }
            }
        }

        // control by itself
        fn is_gdi(&self) -> bool {
            true
        }

        fn set_gdi(&mut self) -> bool {
            true
        }

        #[cfg(feature = "vram")]
        fn device(&self) -> AdapterDevice {
            AdapterDevice::default()
        }

        #[cfg(feature = "vram")]
        fn set_output_texture(&mut self, _texture: bool) {}
    }

    pub(super) fn start_ipc_server() -> mpsc::UnboundedSender<Data> {
        let (tx, rx) = mpsc::unbounded_channel::<Data>();
        std::thread::spawn(move || start_ipc_server_async(rx));
        tx
    }

    #[tokio::main(flavor = "current_thread")]
    async fn start_ipc_server_async(rx: mpsc::UnboundedReceiver<Data>) {
        use DataPortableService::*;
        let rx = Arc::new(tokio::sync::Mutex::new(rx));
        let postfix = IPC_SUFFIX;
        let quick_support = QUICK_SUPPORT.lock().unwrap().clone();

        match new_listener(postfix).await {
            Ok(mut incoming) => loop {
                {
                    tokio::select! {
                        Some(result) = incoming.next() => {
                            match result {
                                Ok(stream) => {
                                    let mut stream = Connection::new(stream);
                                    if !ipc::authorize_windows_portable_service_ipc_connection(
                                        &stream, postfix,
                                    ) {
                                        continue;
                                    }
                                    let mut consumed_token: Option<String> = None;
                                    let mut consumed_token_shmem_name: Option<String> = None;
                                    let handshake_result =
                                        ipc::portable_service_ipc_handshake_as_server(
                                            &mut stream,
                                            |token| {
                                                let (matched, matched_shmem_name) =
                                                    consume_runtime_ipc_token_if_match(token);
                                                if matched {
                                                    consumed_token = Some(token.to_owned());
                                                    consumed_token_shmem_name = matched_shmem_name;
                                                    true
                                                } else {
                                                    false
                                                }
                                            },
                                        )
                                        .await;
                                    if let Err(err) = handshake_result {
                                        if let Some(token) = consumed_token.as_deref() {
                                            restore_runtime_ipc_token_after_failed_handshake(
                                                token,
                                                consumed_token_shmem_name.as_deref(),
                                            );
                                            *STARTING.lock().unwrap() = false;
                                        }
                                        log::warn!(
                                            "Rejected portable service ipc connection due to token handshake failure: postfix={}, err={}",
                                            postfix,
                                            err
                                        );
                                        continue;
                                    }
                                    log::info!("Got portable service ipc connection");
                                    let rx_clone = rx.clone();
                                    tokio::spawn(async move {
                                        let mut stream = stream;
                                        let postfix = postfix.to_owned();
                                        let mut timer = crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
                                        let mut nack = 0;
                                        let mut rx = rx_clone.lock().await;
                                        loop {
                                            tokio::select! {
                                                res = stream.next() => {
                                                    match res {
                                                        Err(err) => {
                                                            log::info!(
                                                                "ipc{} connection closed: {}",
                                                                postfix,
                                                                err
                                                            );
                                                            break;
                                                        }
                                                        Ok(Some(Data::DataPortableService(data))) => match data {
                                                            Ping => {
                                                                stream.send(&Data::DataPortableService(Pong)).await.ok();
                                                            }
                                                            Pong => {
                                                                nack = 0;
                                                                *RUNNING.lock().unwrap() = true;
                                                                *STARTING.lock().unwrap() = false;
                                                            },
                                                            ConnCount(None) => {
                                                                if !quick_support {
                                                                    let remote_count = crate::server::AUTHED_CONNS
                                                                        .lock()
                                                                        .unwrap()
                                                                        .iter()
                                                                        .filter(|c| c.conn_type == crate::server::AuthConnType::Remote)
                                                                        .count();
                                                                    stream.send(&Data::DataPortableService(ConnCount(Some(remote_count)))).await.ok();
                                                                }
                                                            },
                                                            WillClose => {
                                                                log::info!("portable service will close");
                                                                break;
                                                            }
                                                            _=>{}
                                                        }
                                                        _=>{}
                                                    }
                                                }
                                                _ = timer.tick() => {
                                                    nack+=1;
                                                    if nack > MAX_NACK {
                                                        // In fact, this will not happen, ipc will be closed before max nack.
                                                        log::error!("max ipc nack");
                                                        break;
                                                    }
                                                    stream.send(&Data::DataPortableService(Ping)).await.ok();
                                                }
                                                Some(data) = rx.recv() => {
                                                    allow_err!(stream.send(&data).await);
                                                }
                                            }
                                        }
                                        *RUNNING.lock().unwrap() = false;
                                        *STARTING.lock().unwrap() = false;
                                    });
                                }
                                Err(err) => {
                                    log::error!("Couldn't get portable client: {:?}", err);
                                }
                            }
                        }
                    }
                }
            },
            Err(err) => {
                log::error!("Failed to start portable service ipc server: {}", err);
            }
        }
    }

    fn ipc_send(data: Data) -> ResultType<()> {
        let sender = SENDER.lock().unwrap();
        sender
            .send(data)
            .map_err(|e| anyhow!("ipc send error:{:?}", e))
    }

    fn get_cursor_info_(shmem: &mut SharedMemory, pci: PCURSORINFO) -> BOOL {
        unsafe {
            let shmem_addr_para = shmem.as_ptr().add(ADDR_CURSOR_PARA);
            if utils::counter_ready(shmem.as_ptr().add(ADDR_CURSOR_COUNTER)) {
                std::ptr::copy_nonoverlapping(shmem_addr_para, pci as _, size_of::<CURSORINFO>());
                return TRUE;
            }
            FALSE
        }
    }

    fn handle_mouse_(
        evt: &MouseEvent,
        conn: i32,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    ) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Mouse((
            v,
            conn,
            username,
            argb,
            simulate,
            show_cursor,
        ))))
    }

    fn handle_pointer_(evt: &PointerDeviceEvent, conn: i32) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Pointer((
            v, conn,
        ))))
    }

    fn handle_key_(evt: &KeyEvent) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Key(v)))
    }

    pub fn create_capturer(
        current_display: usize,
        display: scrap::Display,
        portable_service_running: bool,
    ) -> ResultType<Box<dyn TraitCapturer>> {
        if portable_service_running != RUNNING.lock().unwrap().clone() {
            log::info!("portable service status mismatch");
        }
        if portable_service_running && display.is_primary() {
            log::info!("Create shared memory capturer");
            return Ok(Box::new(CapturerPortable::new(current_display)));
        } else {
            log::debug!("Create capturer dxgi|gdi");
            return Ok(Box::new(
                Capturer::new(display).with_context(|| "Failed to create capturer")?,
            ));
        }
    }

    pub fn get_cursor_info(pci: PCURSORINFO) -> BOOL {
        if RUNNING.lock().unwrap().clone() {
            let mut option = SHMEM.lock().unwrap();
            option
                .as_mut()
                .map_or(FALSE, |sheme| get_cursor_info_(sheme, pci))
        } else {
            unsafe { winuser::GetCursorInfo(pci) }
        }
    }

    pub fn handle_mouse(
        evt: &MouseEvent,
        conn: i32,
        username: String,
        argb: u32,
        simulate: bool,
        show_cursor: bool,
    ) {
        if RUNNING.lock().unwrap().clone() {
            crate::input_service::update_latest_input_cursor_time(conn);
            handle_mouse_(evt, conn, username, argb, simulate, show_cursor).ok();
        } else {
            crate::input_service::handle_mouse_(evt, conn, username, argb, simulate, show_cursor);
        }
    }

    pub fn handle_pointer(evt: &PointerDeviceEvent, conn: i32) {
        if RUNNING.lock().unwrap().clone() {
            crate::input_service::update_latest_input_cursor_time(conn);
            handle_pointer_(evt, conn).ok();
        } else {
            crate::input_service::handle_pointer_(evt, conn);
        }
    }

    pub fn handle_key(evt: &KeyEvent) {
        if RUNNING.lock().unwrap().clone() {
            handle_key_(evt).ok();
        } else {
            crate::input_service::handle_key_(evt);
        }
    }

    pub fn running() -> bool {
        RUNNING.lock().unwrap().clone()
    }
}

#[repr(C)]
pub struct CapturerPara {
    recreate: bool,
    current_display: usize,
    timeout_ms: i32,
}

#[repr(C)]
pub struct FrameInfo {
    length: usize,
    width: usize,
    height: usize,
}

#[cfg(test)]
mod tests {
    use super::{is_valid_capture_frame_length, ADDR_CAPTURE_FRAME};

    #[test]
    fn test_is_valid_capture_frame_length_rejects_zero_length() {
        assert!(!is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 1024, 0));
    }

    #[test]
    fn test_is_valid_capture_frame_length_rejects_out_of_bounds_length() {
        assert!(!is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 16, 17));
    }

    #[test]
    fn test_is_valid_capture_frame_length_accepts_in_bounds_length() {
        assert!(is_valid_capture_frame_length(ADDR_CAPTURE_FRAME + 16, 16));
    }
}
