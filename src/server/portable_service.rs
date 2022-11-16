use core::slice;
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail,
    config::Config,
    log,
    message_proto::{KeyEvent, MouseEvent},
    protobuf::Message,
    tokio::{self, sync::mpsc},
    ResultType,
};
use scrap::{Capturer, Frame, TraitCapturer};
use shared_memory::*;
use std::{
    mem::size_of,
    ops::{Deref, DerefMut},
    sync::{Arc, Mutex},
    time::Duration,
};
use winapi::{
    shared::minwindef::{BOOL, FALSE, TRUE},
    um::winuser::{self, CURSORINFO, PCURSORINFO},
};

use crate::{
    ipc::{self, new_listener, Connection, Data, DataPortableService},
    video_service::get_current_display,
};

use super::video_qos;

const SIZE_COUNTER: usize = size_of::<i32>() * 2;
const FRAME_ALIGN: usize = 64;

const ADDR_CURSOR_PARA: usize = 0;
const ADDR_CURSOR_COUNTER: usize = ADDR_CURSOR_PARA + size_of::<CURSORINFO>();

const ADDR_CAPTURER_PARA: usize = ADDR_CURSOR_COUNTER + SIZE_COUNTER;
const ADDR_CAPTURE_FRAME_SIZE: usize = ADDR_CAPTURER_PARA + size_of::<CapturerPara>();
const ADDR_CAPTURE_WOULDBLOCK: usize = ADDR_CAPTURE_FRAME_SIZE + size_of::<i32>();
const ADDR_CAPTURE_FRAME_COUNTER: usize = ADDR_CAPTURE_WOULDBLOCK + size_of::<i32>();

const ADDR_CAPTURE_FRAME: usize =
    (ADDR_CAPTURE_FRAME_COUNTER + SIZE_COUNTER + FRAME_ALIGN - 1) / FRAME_ALIGN * FRAME_ALIGN;

const IPC_PROFIX: &str = "_portable_service";
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
        let flink = Self::flink(name.to_string());
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
        log::info!("Create shared memory, size:{}, flink:{}", size, flink);
        Self::set_all_perm(&flink);
        Ok(SharedMemory { inner: shmem })
    }

    pub fn open_existing(name: &str) -> ResultType<Self> {
        let flink = Self::flink(name.to_string());
        let shmem = match ShmemConf::new().flink(&flink).allow_raw(true).open() {
            Ok(m) => m,
            Err(e) => {
                bail!("Unable to open existing shmem flink {} : {}", flink, e);
            }
        };
        log::info!("open existing shared memory, flink:{:?}", flink);
        Ok(SharedMemory { inner: shmem })
    }

    pub fn write(&self, addr: usize, data: &[u8]) {
        unsafe {
            assert!(addr + data.len() <= self.inner.len());
            let ptr = self.inner.as_ptr().add(addr);
            let shared_mem_slice = slice::from_raw_parts_mut(ptr, data.len());
            shared_mem_slice.copy_from_slice(data);
        }
    }

    fn flink(name: String) -> String {
        let mut shmem_flink = format!("shared_memory{}", name);
        if cfg!(windows) {
            let df = "C:\\ProgramData";
            let df = if std::path::Path::new(df).exists() {
                df.to_owned()
            } else {
                std::env::var("TEMP").unwrap_or("C:\\Windows\\TEMP".to_owned())
            };
            let df = format!("{}\\{}", df, *hbb_common::config::APP_NAME.read().unwrap());
            std::fs::create_dir(&df).ok();
            shmem_flink = format!("{}\\{}", df, shmem_flink);
        } else {
            shmem_flink = Config::ipc_path("").replace("ipc", "") + &shmem_flink;
        }
        return shmem_flink;
    }

    fn set_all_perm(_p: &str) {
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(_p, std::fs::Permissions::from_mode(0o0777)).ok();
        }
    }
}

mod utils {
    use core::slice;
    use std::mem::size_of;

    pub fn i32_to_vec(i: i32) -> Vec<u8> {
        i.to_ne_bytes().to_vec()
    }

    pub fn ptr_to_i32(ptr: *const u8) -> i32 {
        unsafe {
            let v = slice::from_raw_parts(ptr, size_of::<i32>());
            i32::from_ne_bytes([v[0], v[1], v[2], v[3]])
        }
    }

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

    pub fn counter_equal(counter: *const u8) -> bool {
        unsafe {
            let wptr = counter;
            let rptr = counter.add(size_of::<i32>());
            let iw = ptr_to_i32(wptr);
            let ir = ptr_to_i32(rptr);
            iw == ir
        }
    }

    pub fn increase_counter(counter: *mut u8) {
        unsafe {
            let wptr = counter;
            let rptr = counter.add(size_of::<i32>());
            let iw = ptr_to_i32(counter);
            let ir = ptr_to_i32(counter);
            let v = i32_to_vec(iw + 1);
            std::ptr::copy_nonoverlapping(v.as_ptr(), wptr, size_of::<i32>());
            if ir == iw + 1 {
                let v = i32_to_vec(iw);
                std::ptr::copy_nonoverlapping(v.as_ptr(), rptr, size_of::<i32>());
            }
        }
    }

    pub fn align(v: usize, align: usize) -> usize {
        (v + align - 1) / align * align
    }
}

// functions called in seperate SYSTEM user process.
pub mod server {
    use super::*;

    lazy_static::lazy_static! {
        static ref EXIT: Arc<Mutex<bool>> = Default::default();
    }

    pub fn run_portable_service() {
        let shmem = Arc::new(SharedMemory::open_existing(SHMEM_NAME).unwrap());
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
        for th in threads.drain(..) {
            th.join().unwrap();
            log::info!("thread joined");
        }
    }

    fn run_exit_check() {
        loop {
            if EXIT.lock().unwrap().clone() {
                std::thread::sleep(Duration::from_secs(1));
                log::info!("exit from seperate check thread");
                std::process::exit(0);
            }
            std::thread::sleep(Duration::from_secs(1));
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
        let mut last_use_yuv = false;
        let mut last_timeout_ms: i32 = 33;
        let mut spf = Duration::from_millis(last_timeout_ms as _);
        let mut first_frame_captured = false;
        let mut dxgi_failed_times = 0;
        loop {
            if EXIT.lock().unwrap().clone() {
                break;
            }
            unsafe {
                let para_ptr = shmem.as_ptr().add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                let current_display = (*para).current_display;
                let use_yuv = (*para).use_yuv;
                let use_yuv_set = (*para).use_yuv_set;
                let timeout_ms = (*para).timeout_ms;
                if !use_yuv_set {
                    c = None;
                    std::thread::sleep(spf);
                    continue;
                }
                if c.is_none() {
                    *crate::video_service::CURRENT_DISPLAY.lock().unwrap() = current_display;
                    let (_, _current, display) = get_current_display().unwrap();
                    match Capturer::new(display, use_yuv) {
                        Ok(mut v) => {
                            c = {
                                last_current_display = current_display;
                                last_use_yuv = use_yuv;
                                first_frame_captured = false;
                                if dxgi_failed_times > MAX_DXGI_FAIL_TIME {
                                    dxgi_failed_times = 0;
                                    v.set_gdi();
                                }
                                Some(v)
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to create gdi capturer:{:?}", e);
                            std::thread::sleep(std::time::Duration::from_secs(1));
                            continue;
                        }
                    }
                } else {
                    if current_display != last_current_display || use_yuv != last_use_yuv {
                        log::info!(
                            "display:{}->{}, use_yuv:{}->{}",
                            last_current_display,
                            current_display,
                            last_use_yuv,
                            use_yuv
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
                        std::thread::sleep(spf);
                        continue;
                    }
                }
                match c.as_mut().unwrap().frame(spf) {
                    Ok(f) => {
                        let len = f.0.len();
                        let len_slice = utils::i32_to_vec(len as _);
                        shmem.write(ADDR_CAPTURE_FRAME_SIZE, &len_slice);
                        shmem.write(ADDR_CAPTURE_FRAME, f.0);
                        shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                        utils::increase_counter(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER));
                        first_frame_captured = true;
                        dxgi_failed_times = 0;
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            // DXGI_ERROR_INVALID_CALL after each success on Microsoft GPU driver
                            // log::error!("capture frame failed:{:?}", e);
                            if crate::platform::windows::desktop_changed() {
                                crate::platform::try_change_desktop();
                                c = None;
                                std::thread::sleep(spf);
                                continue;
                            }
                            if !c.as_ref().unwrap().is_gdi() {
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
                }
            }
        }
    }

    #[tokio::main(flavor = "current_thread")]
    async fn run_ipc_client() {
        use DataPortableService::*;

        let postfix = IPC_PROFIX;

        match ipc::connect(1000, postfix).await {
            Ok(mut stream) => {
                let mut timer = tokio::time::interval(Duration::from_secs(1));
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
                                    ConnCount(Some(_n)) => {
                                        #[cfg(not(feature = "quick_start"))]
                                        if _n == 0 {
                                            log::info!("Connnection count equals 0, exit");
                                            stream.send(&Data::DataPortableService(WillClose)).await.ok();
                                            break;
                                        }
                                    }
                                    Mouse(v) => {
                                        if let Ok(evt) = MouseEvent::parse_from_bytes(&v) {
                                            crate::input_service::handle_mouse_(&evt);
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
                            #[cfg(not(feature = "quick_start"))]
                            stream.send(&Data::DataPortableService(ConnCount(None))).await.ok();
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to connect portable service ipc:{:?}", e);
            }
        }

        *EXIT.lock().unwrap() = true;
    }
}

// functions called in main process.
pub mod client {
    use hbb_common::anyhow::Context;

    use super::*;

    lazy_static::lazy_static! {
        pub static ref PORTABLE_SERVICE_RUNNING: Arc<Mutex<bool>> = Default::default();
        static ref SHMEM: Arc<Mutex<Option<SharedMemory>>> = Default::default();
        static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(client::start_ipc_server());
    }

    pub(crate) fn start_portable_service() -> ResultType<()> {
        log::info!("start portable service");
        if PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
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
                let pixel = utils::align(d.width(), align) * utils::align(d.height(), align);
                if max_pixel < pixel {
                    max_pixel = pixel;
                }
            }
            let shmem_size = utils::align(ADDR_CAPTURE_FRAME + max_pixel * 4, align);
            // os error 112, no enough space
            *SHMEM.lock().unwrap() = Some(crate::portable_service::SharedMemory::create(
                crate::portable_service::SHMEM_NAME,
                shmem_size,
            )?);
            shutdown_hooks::add_shutdown_hook(drop_shmem);
        }
        let mut option = SHMEM.lock().unwrap();
        let shmem = option.as_mut().unwrap();
        unsafe {
            libc::memset(shmem.as_ptr() as _, 0, shmem.len() as _);
        }
        if crate::platform::run_background(
            &std::env::current_exe()?.to_string_lossy().to_string(),
            "--portable-service",
        )
        .is_err()
        {
            *SHMEM.lock().unwrap() = None;
            bail!("Failed to run portable service process");
        }
        let _sender = SENDER.lock().unwrap();
        Ok(())
    }

    extern "C" fn drop_shmem() {
        log::info!("drop shared memory");
        *SHMEM.lock().unwrap() = None;
    }

    pub struct CapturerPortable;

    impl CapturerPortable {
        pub fn new(current_display: usize, use_yuv: bool) -> Self
        where
            Self: Sized,
        {
            let mut option = SHMEM.lock().unwrap();
            let shmem = option.as_mut().unwrap();
            Self::set_para(
                shmem,
                CapturerPara {
                    current_display,
                    use_yuv,
                    use_yuv_set: false,
                    timeout_ms: 33,
                },
            );
            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
            CapturerPortable {}
        }

        fn set_para(shmem: &mut SharedMemory, para: CapturerPara) {
            let para_ptr = &para as *const CapturerPara as *const u8;
            let para_data;
            unsafe {
                para_data = slice::from_raw_parts(para_ptr, size_of::<CapturerPara>());
            }
            shmem.write(ADDR_CAPTURER_PARA, para_data);
        }
    }

    impl TraitCapturer for CapturerPortable {
        fn set_use_yuv(&mut self, use_yuv: bool) {
            let mut option = SHMEM.lock().unwrap();
            let shmem = option.as_mut().unwrap();
            unsafe {
                let para_ptr = shmem.as_ptr().add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                Self::set_para(
                    shmem,
                    CapturerPara {
                        current_display: (*para).current_display,
                        use_yuv,
                        use_yuv_set: true,
                        timeout_ms: (*para).timeout_ms,
                    },
                );
            }
        }

        fn frame<'a>(&'a mut self, timeout: Duration) -> std::io::Result<Frame<'a>> {
            let mut option = SHMEM.lock().unwrap();
            let shmem = option.as_mut().unwrap();
            unsafe {
                let base = shmem.as_ptr();
                let para_ptr = base.add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                if timeout.as_millis() != (*para).timeout_ms as _ {
                    Self::set_para(
                        shmem,
                        CapturerPara {
                            current_display: (*para).current_display,
                            use_yuv: (*para).use_yuv,
                            use_yuv_set: (*para).use_yuv_set,
                            timeout_ms: timeout.as_millis() as _,
                        },
                    );
                }
                if utils::counter_ready(base.add(ADDR_CAPTURE_FRAME_COUNTER)) {
                    let frame_len_ptr = base.add(ADDR_CAPTURE_FRAME_SIZE);
                    let frame_len = utils::ptr_to_i32(frame_len_ptr);
                    let frame_ptr = base.add(ADDR_CAPTURE_FRAME);
                    let data = slice::from_raw_parts(frame_ptr, frame_len as usize);
                    Ok(Frame(data))
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
        let postfix = IPC_PROFIX;

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
                                        let mut timer = tokio::time::interval(Duration::from_secs(1));
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
                                                                *PORTABLE_SERVICE_RUNNING.lock().unwrap() = true;
                                                            },
                                                            ConnCount(None) => {
                                                                let cnt = crate::server::CONN_COUNT.lock().unwrap().clone();
                                                                stream.send(&Data::DataPortableService(ConnCount(Some(cnt)))).await.ok();
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
                                        *PORTABLE_SERVICE_RUNNING.lock().unwrap() = false;
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

    fn handle_mouse_(evt: &MouseEvent) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Mouse(v)))
    }

    fn handle_key_(evt: &KeyEvent) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        ipc_send(Data::DataPortableService(DataPortableService::Key(v)))
    }

    pub fn create_capturer(
        current_display: usize,
        display: scrap::Display,
        use_yuv: bool,
        portable_service_running: bool,
    ) -> ResultType<Box<dyn TraitCapturer>> {
        if portable_service_running != PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
            log::info!("portable service status mismatch");
        }
        if portable_service_running {
            log::info!("Create shared memeory capturer");
            return Ok(Box::new(CapturerPortable::new(current_display, use_yuv)));
        } else {
            log::debug!("Create capturer dxgi|gdi");
            return Ok(Box::new(
                Capturer::new(display, use_yuv).with_context(|| "Failed to create capturer")?,
            ));
        }
    }

    pub fn get_cursor_info(pci: PCURSORINFO) -> BOOL {
        if PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
            get_cursor_info_(&mut SHMEM.lock().unwrap().as_mut().unwrap(), pci)
        } else {
            unsafe { winuser::GetCursorInfo(pci) }
        }
    }

    pub fn handle_mouse(evt: &MouseEvent) {
        if PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
            handle_mouse_(evt).ok();
        } else {
            crate::input_service::handle_mouse_(evt);
        }
    }

    pub fn handle_key(evt: &KeyEvent) {
        if PORTABLE_SERVICE_RUNNING.lock().unwrap().clone() {
            handle_key_(evt).ok();
        } else {
            crate::input_service::handle_key_(evt);
        }
    }
}

#[repr(C)]
struct CapturerPara {
    current_display: usize,
    use_yuv: bool,
    use_yuv_set: bool,
    timeout_ms: i32,
}
