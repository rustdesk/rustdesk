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
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use winapi::{
    shared::minwindef::{BOOL, FALSE, TRUE},
    um::winuser::{self, CURSORINFO, PCURSORINFO},
};

use crate::{
    ipc::{self, new_listener, Connection, Data, DataPortableService},
    platform::set_path_permission,
};

use super::video_qos;

const SIZE_COUNTER: usize = size_of::<i32>() * 2;
const FRAME_ALIGN: usize = 64;

const ADDR_CURSOR_PARA: usize = 0;
const ADDR_CURSOR_COUNTER: usize = ADDR_CURSOR_PARA + size_of::<CURSORINFO>();

const ADDR_CAPTURER_PARA: usize = ADDR_CURSOR_COUNTER + SIZE_COUNTER;
const ADDR_CAPTURE_FRAME_INFO: usize = ADDR_CAPTURER_PARA + size_of::<CapturerPara>();
const ADDR_CAPTURE_WOULDBLOCK: usize = ADDR_CAPTURE_FRAME_INFO + size_of::<FrameInfo>();
const ADDR_CAPTURE_FRAME_COUNTER: usize = ADDR_CAPTURE_WOULDBLOCK + size_of::<i32>();

const ADDR_CAPTURE_FRAME: usize =
    (ADDR_CAPTURE_FRAME_COUNTER + SIZE_COUNTER + FRAME_ALIGN - 1) / FRAME_ALIGN * FRAME_ALIGN;

const IPC_SUFFIX: &str = "_portable_service";
pub const SHMEM_NAME: &str = "_portable_service";
const MAX_NACK: usize = 3;
const MAX_DXGI_FAIL_TIME: usize = 5;

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
        set_path_permission(Path::new(&flink), "F").ok();
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
        if !dir.exists() {
            std::fs::create_dir(&dir)?;
            set_path_permission(&dir, "F").ok();
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
    }

    pub fn run_portable_service() {
        let shmem = match SharedMemory::open_existing(SHMEM_NAME) {
            Ok(shmem) => Arc::new(shmem),
            Err(e) => {
                log::error!("Failed to open existing shared memory: {:?}", e);
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
        threads.push(std::thread::spawn(|| {
            run_ipc_client();
        }));
        threads.push(std::thread::spawn(|| {
            run_exit_check();
        }));
        let record_pos_handle = crate::input_service::try_start_record_cursor_pos();
        for th in threads.drain(..) {
            th.join().ok();
            log::info!("thread joined");
        }

        crate::input_service::try_stop_record_cursor_pos();
        if let Some(handle) = record_pos_handle {
            match handle.join() {
                Ok(_) => log::info!("record_pos_handle joined"),
                Err(e) => log::error!("record_pos_handle join error {:?}", &e),
            }
        }
    }

    fn run_exit_check() {
        loop {
            if EXIT.lock().unwrap().clone() {
                std::thread::sleep(Duration::from_millis(50));
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
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
    async fn run_ipc_client() {
        use DataPortableService::*;

        let postfix = IPC_SUFFIX;

        match ipc::connect(1000, postfix).await {
            Ok(mut stream) => {
                let mut timer =
                    crate::rustdesk_interval(tokio::time::interval(Duration::from_secs(1)));
                let mut nack = 0;
                loop {
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
        static ref SHMEM: Arc<Mutex<Option<SharedMemory>>> = Default::default();
        static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(client::start_ipc_server());
        static ref QUICK_SUPPORT: Arc<Mutex<bool>> = Default::default();
    }

    pub enum StartPara {
        Direct,
        Logon(String, String),
    }

    pub(crate) fn start_portable_service(para: StartPara) -> ResultType<()> {
        log::info!("start portable service");
        if RUNNING.lock().unwrap().clone() {
            bail!("already running");
        }
        if SHMEM.lock().unwrap().is_none() {
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
            let shmem_size = utils::align(ADDR_CAPTURE_FRAME + max_pixel * 4, align);
            // os error 112, no enough space
            *SHMEM.lock().unwrap() = Some(crate::portable_service::SharedMemory::create(
                crate::portable_service::SHMEM_NAME,
                shmem_size,
            )?);
            shutdown_hooks::add_shutdown_hook(drop_portable_service_shared_memory);
        }
        if let Some(shmem) = SHMEM.lock().unwrap().as_mut() {
            unsafe {
                libc::memset(shmem.as_ptr() as _, 0, shmem.len() as _);
            }
        }
        match para {
            StartPara::Direct => {
                if let Err(e) = crate::platform::run_background(
                    &std::env::current_exe()?.to_string_lossy().to_string(),
                    "--portable-service",
                ) {
                    *SHMEM.lock().unwrap() = None;
                    bail!("Failed to run portable service process: {}", e);
                }
            }
            StartPara::Logon(username, password) => {
                #[allow(unused_mut)]
                let mut exe = std::env::current_exe()?.to_string_lossy().to_string();
                #[cfg(feature = "flutter")]
                {
                    if let Some(dir) = Path::new(&exe).parent() {
                        if set_path_permission(Path::new(dir), "RX").is_err() {
                            *SHMEM.lock().unwrap() = None;
                            bail!("Failed to set permission of {:?}", dir);
                        }
                    }
                }
                #[cfg(not(feature = "flutter"))]
                match hbb_common::directories_next::UserDirs::new() {
                    Some(user_dir) => {
                        let dir = user_dir
                            .home_dir()
                            .join("AppData")
                            .join("Local")
                            .join("rustdesk-sciter");
                        if std::fs::create_dir_all(&dir).is_ok() {
                            let dst = dir.join("rustdesk.exe");
                            if std::fs::copy(&exe, &dst).is_ok() {
                                if dst.exists() {
                                    if set_path_permission(&dir, "RX").is_ok() {
                                        exe = dst.to_string_lossy().to_string();
                                    }
                                }
                            }
                        }
                    }
                    None => {}
                }
                if let Err(e) = crate::platform::windows::create_process_with_logon(
                    username.as_str(),
                    password.as_str(),
                    &exe,
                    "--portable-service",
                ) {
                    *SHMEM.lock().unwrap() = None;
                    bail!("Failed to run portable service process: {}", e);
                }
            }
        }
        let _sender = SENDER.lock().unwrap();
        Ok(())
    }

    pub extern "C" fn drop_portable_service_shared_memory() {
        // https://stackoverflow.com/questions/35980148/why-does-an-atexit-handler-panic-when-it-accesses-stdout
        // Please make sure there is no print in the call stack
        let mut lock = SHMEM.lock().unwrap();
        if lock.is_some() {
            *lock = None;
        }
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
                    libc::memset(shmem.as_ptr() as _, 0, shmem.len() as _);
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
                    let data = slice::from_raw_parts(frame_ptr, (*frame_info).length);
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
                                    log::info!("Got portable service ipc connection");
                                    let rx_clone = rx.clone();
                                    tokio::spawn(async move {
                                        let mut stream = Connection::new(stream);
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
