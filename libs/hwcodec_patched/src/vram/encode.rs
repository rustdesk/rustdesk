use crate::{
    common::Driver::*,
    ffmpeg::init_av_log,
    vram::{
        amf, ffmpeg, inner::EncodeCalls, mfx, nv, DynamicContext, EncodeContext, FeatureContext,
    },
};
use log::trace;
use std::{
    fmt::Display, os::raw::{c_int, c_void}, slice::from_raw_parts
};

pub struct Encoder {
    calls: EncodeCalls,
    codec: *mut c_void,
    frames: *mut Vec<EncodeFrame>,
    pub ctx: EncodeContext,
}

unsafe impl Send for Encoder {}
unsafe impl Sync for Encoder {}

impl Encoder {
    pub fn new(ctx: EncodeContext) -> Result<Self, ()> {
        init_av_log();
        if ctx.d.width % 2 == 1 || ctx.d.height % 2 == 1 {
            return Err(());
        }
        let calls = match ctx.f.driver {
            NV => nv::encode_calls(),
            AMF => amf::encode_calls(),
            MFX => mfx::encode_calls(),
            FFMPEG => ffmpeg::encode_calls(),
        };
        unsafe {
            let codec = (calls.new)(
                ctx.d.device.unwrap_or(std::ptr::null_mut()),
                ctx.f.luid,
                ctx.f.data_format as i32,
                ctx.d.width,
                ctx.d.height,
                ctx.d.kbitrate,
                ctx.d.framerate,
                ctx.d.gop,
            );
            if codec.is_null() {
                return Err(());
            }
            Ok(Self {
                calls,
                codec,
                frames: Box::into_raw(Box::new(Vec::<EncodeFrame>::new())),
                ctx,
            })
        }
    }

    pub fn encode(&mut self, tex: *mut c_void, ms: i64) -> Result<&mut Vec<EncodeFrame>, i32> {
        unsafe {
            (&mut *self.frames).clear();
            let result = (self.calls.encode)(
                self.codec,
                tex,
                Some(Self::callback),
                self.frames as *mut _ as *mut c_void,
                ms,
            );
            if result != 0 {
                Err(result)
            } else {
                Ok(&mut *self.frames)
            }
        }
    }

    extern "C" fn callback(data: *const u8, size: c_int, key: i32, obj: *const c_void, pts: i64) {
        unsafe {
            let frames = &mut *(obj as *mut Vec<EncodeFrame>);
            frames.push(EncodeFrame {
                data: from_raw_parts(data, size as usize).to_vec(),
                pts,
                key,
            });
        }
    }

    pub fn set_bitrate(&mut self, kbs: i32) -> Result<(), i32> {
        unsafe {
            match (self.calls.set_bitrate)(self.codec, kbs) {
                0 => Ok(()),
                err => Err(err),
            }
        }
    }

    pub fn set_framerate(&mut self, framerate: i32) -> Result<(), i32> {
        unsafe {
            match (self.calls.set_framerate)(self.codec, framerate) {
                0 => Ok(()),
                err => Err(err),
            }
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            (self.calls.destroy)(self.codec);
            self.codec = std::ptr::null_mut();
            let _ = Box::from_raw(self.frames);
            trace!("Encoder dropped");
        }
    }
}

pub struct EncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

impl Display for EncodeFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "encode len:{}, key:{}", self.data.len(), self.key)
    }
}

pub fn available(d: DynamicContext) -> Vec<FeatureContext> {
    use log::debug;

    let mut natives: Vec<_> = vec![];
    natives.append(
        &mut ffmpeg::possible_support_encoders()
            .drain(..)
            .map(|n| (FFMPEG, n))
            .collect(),
    );
    natives.append(
        &mut nv::possible_support_encoders()
            .drain(..)
            .map(|n| (NV, n))
            .collect(),
    );
    natives.append(
        &mut amf::possible_support_encoders()
            .drain(..)
            .map(|n| (AMF, n))
            .collect(),
    );
    natives.append(
        &mut mfx::possible_support_encoders()
            .drain(..)
            .map(|n| (MFX, n))
            .collect(),
    );
    let inputs: Vec<EncodeContext> = natives
        .drain(..)
        .map(|(driver, n)| EncodeContext {
            f: FeatureContext {
                driver: driver.clone(),
                vendor: driver, // Initially set vendor same as driver, will be updated by test results
                data_format: n.format,
                luid: 0,
            },
            d,
        })
        .collect();

    let mut outputs = Vec::<EncodeContext>::new();
    let mut exclude_luid_formats = Vec::<(i64, i32)>::new();

    for input in inputs {
        debug!(
            "Testing vram encoder: driver={:?}, format={:?}",
            input.f.driver, input.f.data_format
        );

        let test = match input.f.driver {
            NV => nv::encode_calls().test,
            AMF => amf::encode_calls().test,
            MFX => mfx::encode_calls().test,
            FFMPEG => ffmpeg::encode_calls().test,
        };

        let mut luids: Vec<i64> = vec![0; crate::vram::MAX_ADATERS];
        let mut vendors: Vec<i32> = vec![0; crate::vram::MAX_ADATERS];
        let mut desc_count: i32 = 0;

        let (excluded_luids, exclude_formats): (Vec<i64>, Vec<i32>) = exclude_luid_formats
            .iter()
            .map(|(luid, format)| (*luid, *format))
            .unzip();

        let result = unsafe {
            test(
                luids.as_mut_ptr(),
                vendors.as_mut_ptr(),
                luids.len() as _,
                &mut desc_count,
                input.f.data_format as i32,
                input.d.width,
                input.d.height,
                input.d.kbitrate,
                input.d.framerate,
                input.d.gop,
                excluded_luids.as_ptr(),
                exclude_formats.as_ptr(),
                exclude_luid_formats.len() as i32,
            )
        };

        if result == 0 {
            if desc_count as usize <= luids.len() {
                debug!(
                    "vram encoder test passed: driver={:?}, adapters={}",
                    input.f.driver, desc_count
                );
                for i in 0..desc_count as usize {
                    let mut input = input.clone();
                    input.f.luid = luids[i];
                    input.f.vendor = match vendors[i] {
                        0 => NV,
                        1 => AMF,
                        2 => MFX,
                        _ => {
                            log::error!(
                                "Unexpected vendor value encountered: {}. Skipping.",
                                vendors[i]
                            );
                            continue;
                        },
                    };
                    exclude_luid_formats.push((luids[i], input.f.data_format as i32));
                    outputs.push(input);
                }
            }
        } else {
            debug!(
                "vram encoder test failed: driver={:?}, error={}",
                input.f.driver, result
            );
        }
    }

    let result: Vec<_> = outputs.drain(..).map(|e| e.f).collect();
    result
}
