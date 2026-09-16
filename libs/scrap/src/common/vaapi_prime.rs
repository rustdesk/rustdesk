use hbb_common::{anyhow::anyhow, bail, libc, log, ResultType};
use std::os::raw::{c_int, c_void};

#[allow(dead_code)]
pub const GPU_FRAME_NV12: i32 = 1;
pub const GPU_FRAME_PRIME: i32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GpuPrimeDesc {
    pub kind: i32,
    pub n_fds: i32,
    pub fds: [i32; 4],
    pub fourcc: u32,
    pub modifier: u64,
    pub width: i32,
    pub height: i32,
    pub n_planes: i32,
    pub pitches: [i32; 4],
    pub offsets: [i32; 4],
    pub obj_indices: [i32; 4],
}

impl Default for GpuPrimeDesc {
    fn default() -> Self {
        Self {
            kind: GPU_FRAME_PRIME,
            n_fds: 0,
            fds: [-1; 4],
            fourcc: 0,
            modifier: 0,
            width: 0,
            height: 0,
            n_planes: 0,
            pitches: [0; 4],
            offsets: [0; 4],
            obj_indices: [0; 4],
        }
    }
}

impl GpuPrimeDesc {
    pub fn close_fds(&mut self) {
        for i in 0..self.n_fds.max(0) as usize {
            let fd = self.fds[i];
            if fd >= 0 {
                unsafe {
                    libc::close(fd);
                }
                self.fds[i] = -1;
            }
        }
        self.n_fds = 0;
    }
}

type OpenFn = unsafe extern "C" fn(c_int) -> *mut c_void;
type CloseFn = unsafe extern "C" fn(*mut c_void);
type DecodeFn = unsafe extern "C" fn(*mut c_void, *const u8, c_int, *mut GpuPrimeDesc) -> c_int;

fn load_sym<T>(name: &[u8]) -> Option<T> {
    unsafe {
        let p = hbb_common::libc::dlsym(hbb_common::libc::RTLD_DEFAULT, name.as_ptr() as _);
        if p.is_null() {
            None
        } else {
            Some(std::mem::transmute_copy(&p))
        }
    }
}

pub fn available() -> bool {
    load_sym::<OpenFn>(b"RustDeskVaapiPrimeOpen\0").is_some()
        && load_sym::<DecodeFn>(b"RustDeskVaapiPrimeDecode\0").is_some()
}

/// Flutter Linux only has a current EGL display (required for dma-buf
/// EGLImage) under Wayland. X11 uses GLX; importing there SIGSEGVs.
pub fn egl_dmabuf_ok() -> bool {
    match std::env::var("GDK_BACKEND") {
        Ok(b) if b.eq_ignore_ascii_case("x11") => return false,
        Ok(b) if b.eq_ignore_ascii_case("wayland") => return true,
        _ => {}
    }
    if std::env::var("WAYLAND_DISPLAY")
        .map(|s| !s.is_empty())
        .unwrap_or(false)
    {
        return true;
    }
    std::env::var("XDG_SESSION_TYPE")
        .map(|s| s.eq_ignore_ascii_case("wayland"))
        .unwrap_or(false)
}

pub struct VaapiPrimeDecoder {
    ctx: *mut c_void,
    close: CloseFn,
    decode: DecodeFn,
    pub last: GpuPrimeDesc,
}

unsafe impl Send for VaapiPrimeDecoder {}

impl VaapiPrimeDecoder {
    pub fn open(hevc: bool) -> ResultType<Self> {
        let open = load_sym::<OpenFn>(b"RustDeskVaapiPrimeOpen\0")
            .ok_or_else(|| anyhow!("VAAPI PRIME decoder not in process"))?;
        let close = load_sym::<CloseFn>(b"RustDeskVaapiPrimeClose\0")
            .ok_or_else(|| anyhow!("RustDeskVaapiPrimeClose missing"))?;
        let decode = load_sym::<DecodeFn>(b"RustDeskVaapiPrimeDecode\0")
            .ok_or_else(|| anyhow!("RustDeskVaapiPrimeDecode missing"))?;
        let ctx = unsafe { open(if hevc { 1 } else { 0 }) };
        if ctx.is_null() {
            bail!("RustDeskVaapiPrimeOpen failed");
        }
        log::info!("VAAPI DRM-PRIME decoder opened (hevc={hevc})");
        Ok(Self {
            ctx,
            close,
            decode,
            last: GpuPrimeDesc::default(),
        })
    }

    pub fn decode(&mut self, data: &[u8]) -> ResultType<bool> {
        self.last.close_fds();
        let mut out = GpuPrimeDesc::default();
        let ok = unsafe {
            (self.decode)(
                self.ctx,
                data.as_ptr(),
                data.len() as c_int,
                &mut out as *mut _,
            )
        };
        if ok == 1 {
            self.last = out;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Drop for VaapiPrimeDecoder {
    fn drop(&mut self) {
        self.last.close_fds();
        if !self.ctx.is_null() {
            unsafe { (self.close)(self.ctx) };
            self.ctx = std::ptr::null_mut();
        }
    }
}
