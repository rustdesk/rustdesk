use core::slice;
use hbb_common::{
    allow_err,
    anyhow::anyhow,
    bail,
    config::Config,
    log,
    message_proto::{KeyEvent, MouseEvent},
    protobuf::Message,
    sleep,
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
const IPC_CONN_TIMEOUT: Duration = Duration::from_secs(3);

pub enum PortableServiceStatus {
    NonStart,
    Running,
}

impl Default for PortableServiceStatus {
    fn default() -> Self {
        Self::NonStart
    }
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

    pub fn increase_counter(ptr: *mut u8) {
        unsafe {
            let i = ptr_to_i32(ptr);
            let v = i32_to_vec(i + 1);
            std::ptr::copy_nonoverlapping(v.as_ptr(), ptr, size_of::<i32>());
        }
    }

    pub fn align(v: usize, align: usize) -> usize {
        (v + align - 1) / align * align
    }
}

// functions called in seperate SYSTEM user process.
pub mod server {
    use hbb_common::tokio::time::Instant;

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
            run_ipc_server();
        }));
        threads.push(std::thread::spawn(|| {
            run_exit_check();
        }));
        for th in threads.drain(..) {
            th.join().unwrap();
            log::info!("all thread joined");
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
        loop {
            if EXIT.lock().unwrap().clone() {
                break;
            }
            let start = std::time::Instant::now();
            unsafe {
                let para_ptr = shmem.as_ptr().add(ADDR_CAPTURER_PARA);
                let para = para_ptr as *const CapturerPara;
                let current_display = (*para).current_display;
                let use_yuv = (*para).use_yuv;
                let timeout_ms = (*para).timeout_ms;
                if c.is_none() {
                    let use_yuv = true;
                    *crate::video_service::CURRENT_DISPLAY.lock().unwrap() = current_display;
                    let (_, _current, display) = get_current_display().unwrap();
                    match Capturer::new(display, use_yuv) {
                        Ok(mut v) => {
                            c = {
                                last_current_display = current_display;
                                last_use_yuv = use_yuv;
                                // dxgi failed at loadFrame on my PC.
                                // to-do: try dxgi on another PC.
                                v.set_gdi();
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
                match c.as_mut().unwrap().frame(spf) {
                    Ok(f) => {
                        let len = f.0.len();
                        let len_slice = utils::i32_to_vec(len as _);
                        shmem.write(ADDR_CAPTURE_FRAME_SIZE, &len_slice);
                        shmem.write(ADDR_CAPTURE_FRAME, f.0);
                        shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                        utils::increase_counter(shmem.as_ptr().add(ADDR_CAPTURE_FRAME_COUNTER));
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            log::error!("capture frame failed:{:?}", e);
                            crate::platform::try_change_desktop();
                            c = None;
                            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(FALSE));
                            continue;
                        } else {
                            shmem.write(ADDR_CAPTURE_WOULDBLOCK, &utils::i32_to_vec(TRUE));
                        }
                    }
                }
            }
            let elapsed = start.elapsed();
            if elapsed < spf {
                std::thread::sleep(spf - elapsed);
            }
        }
    }

    #[tokio::main(flavor = "current_thread")]
    async fn run_ipc_server() {
        use DataPortableService::*;

        let postfix = IPC_PROFIX;
        let last_recv_time = Arc::new(Mutex::new(Instant::now()));
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        match new_listener(postfix).await {
            Ok(mut incoming) => loop {
                tokio::select! {
                    Some(result) = incoming.next() => {
                        match result {
                            Ok(stream) => {
                                log::info!("Got new connection");
                                let  last_recv_time_cloned = last_recv_time.clone();
                                tokio::spawn(async move {
                                    let mut stream = Connection::new(stream);
                                    let postfix = postfix.to_owned();
                                    let mut timer = tokio::time::interval(Duration::from_secs(1));
                                    let mut nack = 0;
                                    let mut old_conn_count = 0;
                                    loop {
                                        tokio::select! {
                                            res = stream.next() => {
                                                if res.is_ok() {
                                                    *last_recv_time_cloned.lock().unwrap() = Instant::now();
                                                }
                                                match res {
                                                    Err(err) => {
                                                        log::error!(
                                                            "ipc{} connection closed: {}",
                                                            postfix,
                                                            err
                                                        );
                                                        *EXIT.lock().unwrap() = true;
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
                                                            if old_conn_count != 0 && n == 0 {
                                                                log::info!("Connection count decrease to 0, exit");
                                                                stream.send(&Data::DataPortableService(WillClose)).await.ok();
                                                                *EXIT.lock().unwrap() = true;
                                                                break;
                                                            }
                                                            old_conn_count = n;
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
                                                    *EXIT.lock().unwrap() = true;
                                                    break;
                                                }
                                                stream.send(&Data::DataPortableService(Ping)).await.ok();
                                                stream.send(&Data::DataPortableService(ConnCount(None))).await.ok();
                                            }
                                        }
                                    }
                                });
                            }
                            Err(err) => {
                                log::error!("Couldn't get portable client: {:?}", err);
                                *EXIT.lock().unwrap() = true;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if last_recv_time.lock().unwrap().elapsed() > IPC_CONN_TIMEOUT {
                            log::error!("receive data timeout");
                            *EXIT.lock().unwrap() = true;
                        }
                        if EXIT.lock().unwrap().clone() {
                            break;
                        }
                    }
                }
            },
            Err(err) => {
                log::error!("Failed to start cm ipc server: {}", err);
                *EXIT.lock().unwrap() = true;
            }
        }
    }
}

// functions called in main process.
pub mod client {
    use hbb_common::anyhow::Context;

    use super::*;

    lazy_static::lazy_static! {
        pub static ref SHMEM: Arc<Mutex<Option<SharedMemory>>> = Default::default();
        pub static ref PORTABLE_SERVICE_STATUS: Arc<Mutex<PortableServiceStatus>> = Default::default();
        static ref SENDER : Mutex<mpsc::UnboundedSender<ipc::Data>> = Mutex::new(client::start_ipc_client());
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PortableServiceStatus {
        NotStarted,
        Starting,
        Running,
    }

    impl Default for PortableServiceStatus {
        fn default() -> Self {
            Self::NotStarted
        }
    }

    pub(crate) fn start_portable_service() -> ResultType<()> {
        if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::NotStarted {
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
            if crate::common::run_me(vec!["--portable-service"]).is_err() {
                *SHMEM.lock().unwrap() = None;
                bail!("Failed to run portable service process");
            }
            *PORTABLE_SERVICE_STATUS.lock().unwrap() = PortableServiceStatus::Starting;
            let _sender = SENDER.lock().unwrap();
        }
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
            Self::set_para(CapturerPara {
                current_display,
                use_yuv,
                timeout_ms: 33,
            });
            CapturerPortable {}
        }

        fn set_para(para: CapturerPara) {
            let mut option = SHMEM.lock().unwrap();
            let shmem = option.as_mut().unwrap();
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
                if use_yuv != (*para).use_yuv {
                    Self::set_para(CapturerPara {
                        current_display: (*para).current_display,
                        use_yuv,
                        timeout_ms: (*para).timeout_ms,
                    });
                }
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
                    Self::set_para(CapturerPara {
                        current_display: (*para).current_display,
                        use_yuv: (*para).use_yuv,
                        timeout_ms: timeout.as_millis() as _,
                    });
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

        fn is_gdi(&self) -> bool {
            true
        }

        fn set_gdi(&mut self) -> bool {
            true
        }
    }

    pub(super) fn start_ipc_client() -> mpsc::UnboundedSender<Data> {
        let (tx, rx) = mpsc::unbounded_channel::<Data>();
        std::thread::spawn(move || start_ipc_client_async(rx));
        tx
    }

    #[tokio::main(flavor = "current_thread")]
    async fn start_ipc_client_async(rx: mpsc::UnboundedReceiver<Data>) {
        use DataPortableService::*;
        let mut rx = rx;
        let mut connect_failed = 0;
        loop {
            if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::NotStarted
            {
                sleep(1.).await;
                continue;
            }
            if let Ok(mut c) = ipc::connect(1000, IPC_PROFIX).await {
                let mut nack = 0;
                let mut timer = tokio::time::interval(Duration::from_secs(1));
                loop {
                    tokio::select! {
                        res = c.next() => {
                            match res {
                                Err(err) => {
                                    log::error!("ipc connection closed: {}", err);
                                    break;
                                }
                                Ok(Some(Data::DataPortableService(data))) => {
                                    match data {
                                        Ping => {
                                            c.send(&Data::DataPortableService(Pong)).await.ok();
                                        }
                                        Pong => {
                                            nack = 0;
                                            *PORTABLE_SERVICE_STATUS.lock().unwrap() = PortableServiceStatus::Running;
                                        },
                                        ConnCount(None) => {
                                            let cnt = crate::server::CONN_COUNT.lock().unwrap().clone();
                                            c.send(&Data::DataPortableService(ConnCount(Some(cnt)))).await.ok();
                                        },
                                        WillClose => {
                                            log::info!("portable service will close, set status to not started");
                                            *PORTABLE_SERVICE_STATUS.lock().unwrap() = PortableServiceStatus::NotStarted;
                                            break;
                                        }
                                        _=>{}
                                    }
                                }
                                _ => {}
                            }
                        }
                        _ = timer.tick() => {
                            nack+=1;
                            if nack > MAX_NACK {
                                // In fact, this will not happen, ipc will be closed before max nack.
                                log::error!("max ipc nack, set status to not started");
                                *PORTABLE_SERVICE_STATUS.lock().unwrap() = PortableServiceStatus::NotStarted;
                                break;
                            }
                            c.send(&Data::DataPortableService(Ping)).await.ok();
                        }
                        Some(data) = rx.recv() => {
                            allow_err!(c.send(&data).await);
                        }

                    }
                }
            } else {
                connect_failed += 1;
                if connect_failed > IPC_CONN_TIMEOUT.as_secs() {
                    connect_failed = 0;
                    *PORTABLE_SERVICE_STATUS.lock().unwrap() = PortableServiceStatus::NotStarted;
                    log::info!(
                        "connect failed {} times, set status to not started",
                        connect_failed
                    );
                }
                log::info!(
                    "client ip connect failed, status:{:?}",
                    PORTABLE_SERVICE_STATUS.lock().unwrap().clone(),
                );
            }
            sleep(1.).await;
        }
    }

    fn client_ipc_send(data: Data) -> ResultType<()> {
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
        client_ipc_send(Data::DataPortableService(DataPortableService::Mouse(v)))
    }

    fn handle_key_(evt: &KeyEvent) -> ResultType<()> {
        let mut v = vec![];
        evt.write_to_vec(&mut v)?;
        client_ipc_send(Data::DataPortableService(DataPortableService::Key(v)))
    }

    pub fn create_capturer(
        current_display: usize,
        display: scrap::Display,
        use_yuv: bool,
    ) -> ResultType<Box<dyn TraitCapturer>> {
        if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::Running {
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
        if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::Running {
            get_cursor_info_(&mut SHMEM.lock().unwrap().as_mut().unwrap(), pci)
        } else {
            unsafe { winuser::GetCursorInfo(pci) }
        }
    }

    pub fn handle_mouse(evt: &MouseEvent) {
        if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::Running {
            handle_mouse_(evt).ok();
        } else {
            crate::input_service::handle_mouse_(evt);
        }
    }

    pub fn handle_key(evt: &KeyEvent) {
        if PORTABLE_SERVICE_STATUS.lock().unwrap().clone() == PortableServiceStatus::Running {
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
    timeout_ms: i32,
}
