// https://github.com/astraw/vpx-encode
// https://github.com/astraw/env-libvpx-sys
// https://github.com/rust-av/vpx-rs/blob/master/src/decoder.rs

use hbb_common::anyhow::{anyhow, Context};
use hbb_common::message_proto::{EncodedVideoFrame, EncodedVideoFrames, Message, VideoFrame};
use hbb_common::ResultType;

use crate::STRIDE_ALIGN;
use crate::{codec::EncoderApi, ImageFormat};

use super::vpx::{vp8e_enc_control_id::*, vpx_codec_err_t::*, *};
use hbb_common::bytes::Bytes;
use std::os::raw::{c_int, c_uint};
use std::{ptr, slice};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VpxVideoCodecId {
    VP8,
    VP9,
}

impl Default for VpxVideoCodecId {
    fn default() -> VpxVideoCodecId {
        VpxVideoCodecId::VP9
    }
}

pub struct VpxEncoder {
    ctx: vpx_codec_ctx_t,
    width: usize,
    height: usize,
}

pub struct VpxDecoder {
    ctx: vpx_codec_ctx_t,
}

#[derive(Debug)]
pub enum Error {
    FailedCall(String),
    BadPtr(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! call_vpx {
    ($x:expr) => {{
        let result = unsafe { $x }; // original expression
        let result_int = unsafe { std::mem::transmute::<_, i32>(result) };
        if result_int != 0 {
            return Err(Error::FailedCall(format!(
                "errcode={} {}:{}:{}:{}",
                result_int,
                module_path!(),
                file!(),
                line!(),
                column!()
            ))
            .into());
        }
        result
    }};
}

macro_rules! call_vpx_ptr {
    ($x:expr) => {{
        let result = unsafe { $x }; // original expression
        let result_int = unsafe { std::mem::transmute::<_, isize>(result) };
        if result_int == 0 {
            return Err(Error::BadPtr(format!(
                "errcode={} {}:{}:{}:{}",
                result_int,
                module_path!(),
                file!(),
                line!(),
                column!()
            ))
            .into());
        }
        result
    }};
}

impl EncoderApi for VpxEncoder {
    fn new(cfg: crate::codec::EncoderCfg) -> ResultType<Self>
    where
        Self: Sized,
    {
        match cfg {
            crate::codec::EncoderCfg::VPX(config) => {
                let i;
                if cfg!(feature = "VP8") {
                    i = match config.codec {
                        VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_cx()),
                        VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_cx()),
                    };
                } else {
                    i = call_vpx_ptr!(vpx_codec_vp9_cx());
                }
                let mut c = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
                call_vpx!(vpx_codec_enc_config_default(i, &mut c, 0));

                // https://www.webmproject.org/docs/encoder-parameters/
                // default: c.rc_min_quantizer = 0, c.rc_max_quantizer = 63
                // try rc_resize_allowed later

                c.g_w = config.width;
                c.g_h = config.height;
                c.g_timebase.num = config.timebase[0];
                c.g_timebase.den = config.timebase[1];
                c.rc_target_bitrate = config.bitrate;
                c.rc_undershoot_pct = 95;
                c.rc_dropframe_thresh = 25;
                c.g_threads = if config.num_threads == 0 {
                    num_cpus::get() as _
                } else {
                    config.num_threads
                };
                c.g_error_resilient = VPX_ERROR_RESILIENT_DEFAULT;
                // https://developers.google.com/media/vp9/bitrate-modes/
                // Constant Bitrate mode (CBR) is recommended for live streaming with VP9.
                c.rc_end_usage = vpx_rc_mode::VPX_CBR;
                // c.kf_min_dist = 0;
                // c.kf_max_dist = 999999;
                c.kf_mode = vpx_kf_mode::VPX_KF_DISABLED; // reduce bandwidth a lot

                /*
                VPX encoder支持two-pass encode，这是为了rate control的。
                对于两遍编码，就是需要整个编码过程做两次，第一次会得到一些新的控制参数来进行第二遍的编码，
                这样可以在相同的bitrate下得到最好的PSNR
                */

                let mut ctx = Default::default();
                call_vpx!(vpx_codec_enc_init_ver(
                    &mut ctx,
                    i,
                    &c,
                    0,
                    VPX_ENCODER_ABI_VERSION as _
                ));

                if config.codec == VpxVideoCodecId::VP9 {
                    // set encoder internal speed settings
                    // in ffmpeg, it is --speed option
                    /*
                    set to 0 or a positive value 1-16, the codec will try to adapt its
                    complexity depending on the time it spends encoding. Increasing this
                    number will make the speed go up and the quality go down.
                    Negative values mean strict enforcement of this
                    while positive values are adaptive
                    */
                    /* https://developers.google.com/media/vp9/live-encoding
                    Speed 5 to 8 should be used for live / real-time encoding.
                    Lower numbers (5 or 6) are higher quality but require more CPU power.
                    Higher numbers (7 or 8) will be lower quality but more manageable for lower latency
                    use cases and also for lower CPU power devices such as mobile.
                    */
                    call_vpx!(vpx_codec_control_(&mut ctx, VP8E_SET_CPUUSED as _, 7,));
                    // set row level multi-threading
                    /*
                    as some people in comments and below have already commented,
                    more recent versions of libvpx support -row-mt 1 to enable tile row
                    multi-threading. This can increase the number of tiles by up to 4x in VP9
                    (since the max number of tile rows is 4, regardless of video height).
                    To enable this, use -tile-rows N where N is the number of tile rows in
                    log2 units (so -tile-rows 1 means 2 tile rows and -tile-rows 2 means 4 tile
                    rows). The total number of active threads will then be equal to
                    $tile_rows * $tile_columns
                    */
                    call_vpx!(vpx_codec_control_(
                        &mut ctx,
                        VP9E_SET_ROW_MT as _,
                        1 as c_int
                    ));

                    call_vpx!(vpx_codec_control_(
                        &mut ctx,
                        VP9E_SET_TILE_COLUMNS as _,
                        4 as c_int
                    ));
                }

                Ok(Self {
                    ctx,
                    width: config.width as _,
                    height: config.height as _,
                })
            }
            _ => Err(anyhow!("encoder type mismatch")),
        }
    }

    fn encode_to_message(&mut self, frame: &[u8], ms: i64) -> ResultType<Message> {
        let mut frames = Vec::new();
        for ref frame in self
            .encode(ms, frame, STRIDE_ALIGN)
            .with_context(|| "Failed to encode")?
        {
            frames.push(VpxEncoder::create_frame(frame));
        }
        for ref frame in self.flush().with_context(|| "Failed to flush")? {
            frames.push(VpxEncoder::create_frame(frame));
        }

        // to-do: flush periodically, e.g. 1 second
        if frames.len() > 0 {
            Ok(VpxEncoder::create_msg(frames))
        } else {
            Err(anyhow!("no valid frame"))
        }
    }

    fn use_yuv(&self) -> bool {
        true
    }

    fn set_bitrate(&mut self, bitrate: u32) -> ResultType<()> {
        let mut new_enc_cfg = unsafe { *self.ctx.config.enc.to_owned() };
        new_enc_cfg.rc_target_bitrate = bitrate;
        call_vpx!(vpx_codec_enc_config_set(&mut self.ctx, &new_enc_cfg));
        return Ok(());
    }
}

impl VpxEncoder {
    pub fn encode(&mut self, pts: i64, data: &[u8], stride_align: usize) -> Result<EncodeFrames> {
        if 2 * data.len() < 3 * self.width * self.height {
            return Err(Error::FailedCall("len not enough".to_string()));
        }

        let mut image = Default::default();
        call_vpx_ptr!(vpx_img_wrap(
            &mut image,
            vpx_img_fmt::VPX_IMG_FMT_I420,
            self.width as _,
            self.height as _,
            stride_align as _,
            data.as_ptr() as _,
        ));

        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            &image,
            pts as _,
            1, // Duration
            0, // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the encoder to return any pending packets
    pub fn flush(&mut self) -> Result<EncodeFrames> {
        call_vpx!(vpx_codec_encode(
            &mut self.ctx,
            ptr::null(),
            -1, // PTS
            1,  // Duration
            0,  // Flags
            VPX_DL_REALTIME as _,
        ));

        Ok(EncodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    #[inline]
    fn create_msg(vp9s: Vec<EncodedVideoFrame>) -> Message {
        let mut msg_out = Message::new();
        let mut vf = VideoFrame::new();
        vf.set_vp9s(EncodedVideoFrames {
            frames: vp9s.into(),
            ..Default::default()
        });
        msg_out.set_video_frame(vf);
        msg_out
    }

    #[inline]
    fn create_frame(frame: &EncodeFrame) -> EncodedVideoFrame {
        EncodedVideoFrame {
            data: Bytes::from(frame.data.to_vec()),
            key: frame.key,
            pts: frame.pts,
            ..Default::default()
        }
    }
}

impl Drop for VpxEncoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeFrame<'a> {
    /// Compressed data.
    pub data: &'a [u8],
    /// Whether the frame is a keyframe.
    pub key: bool,
    /// Presentation timestamp (in timebase units).
    pub pts: i64,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxEncoderConfig {
    /// The width (in pixels).
    pub width: c_uint,
    /// The height (in pixels).
    pub height: c_uint,
    /// The timebase numerator and denominator (in seconds).
    pub timebase: [c_int; 2],
    /// The target bitrate (in kilobits per second).
    pub bitrate: c_uint,
    /// The codec
    pub codec: VpxVideoCodecId,
    pub num_threads: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct VpxDecoderConfig {
    pub codec: VpxVideoCodecId,
    pub num_threads: u32,
}

pub struct EncodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for EncodeFrames<'a> {
    type Item = EncodeFrame<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            unsafe {
                let pkt = vpx_codec_get_cx_data(self.ctx, &mut self.iter);
                if pkt.is_null() {
                    return None;
                } else if (*pkt).kind == vpx_codec_cx_pkt_kind::VPX_CODEC_CX_FRAME_PKT {
                    let f = &(*pkt).data.frame;
                    return Some(Self::Item {
                        data: slice::from_raw_parts(f.buf as _, f.sz as _),
                        key: (f.flags & VPX_FRAME_IS_KEY) != 0,
                        pts: f.pts,
                    });
                } else {
                    // Ignore the packet.
                }
            }
        }
    }
}

impl VpxDecoder {
    /// Create a new decoder
    ///
    /// # Errors
    ///
    /// The function may fail if the underlying libvpx does not provide
    /// the VP9 decoder.
    pub fn new(config: VpxDecoderConfig) -> Result<Self> {
        // This is sound because `vpx_codec_ctx` is a repr(C) struct without any field that can
        // cause UB if uninitialized.
        let i;
        if cfg!(feature = "VP8") {
            i = match config.codec {
                VpxVideoCodecId::VP8 => call_vpx_ptr!(vpx_codec_vp8_dx()),
                VpxVideoCodecId::VP9 => call_vpx_ptr!(vpx_codec_vp9_dx()),
            };
        } else {
            i = call_vpx_ptr!(vpx_codec_vp9_dx());
        }
        let mut ctx = Default::default();
        let cfg = vpx_codec_dec_cfg_t {
            threads: if config.num_threads == 0 {
                num_cpus::get() as _
            } else {
                config.num_threads
            },
            w: 0,
            h: 0,
        };
        /*
        unsafe {
            println!("{}", vpx_codec_get_caps(i));
        }
        */
        call_vpx!(vpx_codec_dec_init_ver(
            &mut ctx,
            i,
            &cfg,
            0,
            VPX_DECODER_ABI_VERSION as _,
        ));
        Ok(Self { ctx })
    }

    pub fn decode2rgb(&mut self, data: &[u8], fmt: ImageFormat) -> Result<Vec<u8>> {
        let mut img = Image::new();
        for frame in self.decode(data)? {
            drop(img);
            img = frame;
        }
        for frame in self.flush()? {
            drop(img);
            img = frame;
        }
        if img.is_null() {
            Ok(Vec::new())
        } else {
            let mut out = Default::default();
            img.to(fmt, 1, &mut out);
            Ok(out)
        }
    }

    /// Feed some compressed data to the encoder
    ///
    /// The `data` slice is sent to the decoder
    ///
    /// It matches a call to `vpx_codec_decode`.
    pub fn decode(&mut self, data: &[u8]) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            data.as_ptr(),
            data.len() as _,
            ptr::null_mut(),
            0,
        ));

        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }

    /// Notify the decoder to return any pending frame
    pub fn flush(&mut self) -> Result<DecodeFrames> {
        call_vpx!(vpx_codec_decode(
            &mut self.ctx,
            ptr::null(),
            0,
            ptr::null_mut(),
            0
        ));
        Ok(DecodeFrames {
            ctx: &mut self.ctx,
            iter: ptr::null(),
        })
    }
}

impl Drop for VpxDecoder {
    fn drop(&mut self) {
        unsafe {
            let result = vpx_codec_destroy(&mut self.ctx);
            if result != VPX_CODEC_OK {
                panic!("failed to destroy vpx codec");
            }
        }
    }
}

pub struct DecodeFrames<'a> {
    ctx: &'a mut vpx_codec_ctx_t,
    iter: vpx_codec_iter_t,
}

impl<'a> Iterator for DecodeFrames<'a> {
    type Item = Image;
    fn next(&mut self) -> Option<Self::Item> {
        let img = unsafe { vpx_codec_get_frame(self.ctx, &mut self.iter) };
        if img.is_null() {
            return None;
        } else {
            return Some(Image(img));
        }
    }
}

// https://chromium.googlesource.com/webm/libvpx/+/bali/vpx/src/vpx_image.c
pub struct Image(*mut vpx_image_t);
impl Image {
    #[inline]
    pub fn new() -> Self {
        Self(std::ptr::null_mut())
    }

    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    pub fn width(&self) -> usize {
        self.inner().d_w as _
    }

    #[inline]
    pub fn height(&self) -> usize {
        self.inner().d_h as _
    }

    #[inline]
    pub fn format(&self) -> vpx_img_fmt_t {
        // VPX_IMG_FMT_I420
        self.inner().fmt
    }

    #[inline]
    pub fn inner(&self) -> &vpx_image_t {
        unsafe { &*self.0 }
    }

    #[inline]
    pub fn stride(&self, iplane: usize) -> i32 {
        self.inner().stride[iplane]
    }

    pub fn to(&self, fmt: ImageFormat, stride: usize, dst: &mut Vec<u8>) {
        let h = self.height();
        let w = self.width();
        let bytes_per_pixel = match fmt {
            ImageFormat::Raw => 3,
            ImageFormat::ARGB | ImageFormat::ABGR => 4,
        };
        // https://github.com/lemenkov/libyuv/blob/6900494d90ae095d44405cd4cc3f346971fa69c9/source/convert_argb.cc#L128
        // https://github.com/lemenkov/libyuv/blob/6900494d90ae095d44405cd4cc3f346971fa69c9/source/convert_argb.cc#L129
        let bytes_per_row = (w * bytes_per_pixel + stride - 1) & !(stride - 1);
        dst.resize(h * bytes_per_row, 0);
        let img = self.inner();
        unsafe {
            match fmt {
                ImageFormat::Raw => {
                    super::I420ToRAW(
                        img.planes[0],
                        img.stride[0],
                        img.planes[1],
                        img.stride[1],
                        img.planes[2],
                        img.stride[2],
                        dst.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                ImageFormat::ARGB => {
                    super::I420ToARGB(
                        img.planes[0],
                        img.stride[0],
                        img.planes[1],
                        img.stride[1],
                        img.planes[2],
                        img.stride[2],
                        dst.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
                ImageFormat::ABGR => {
                    super::I420ToABGR(
                        img.planes[0],
                        img.stride[0],
                        img.planes[1],
                        img.stride[1],
                        img.planes[2],
                        img.stride[2],
                        dst.as_mut_ptr(),
                        bytes_per_row as _,
                        self.width() as _,
                        self.height() as _,
                    );
                }
            }
        }
    }

    #[inline]
    pub fn data(&self) -> (&[u8], &[u8], &[u8]) {
        unsafe {
            let img = self.inner();
            let h = (img.d_h as usize + 1) & !1;
            let n = img.stride[0] as usize * h;
            let y = slice::from_raw_parts(img.planes[0], n);
            let n = img.stride[1] as usize * (h >> 1);
            let u = slice::from_raw_parts(img.planes[1], n);
            let v = slice::from_raw_parts(img.planes[2], n);
            (y, u, v)
        }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { vpx_img_free(self.0) };
        }
    }
}

unsafe impl Send for vpx_codec_ctx_t {}
