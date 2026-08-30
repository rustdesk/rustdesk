use std::{ffi::c_void, ptr};

use hbb_common::log;

const AUDIOSTREAM_SUCCESS: i32 = 0;
const AUDIOSTREAM_TYPE_CAPTURER: i32 = 2;
const AUDIOSTREAM_SAMPLE_S16LE: i32 = 1;
const AUDIOSTREAM_ENCODING_TYPE_RAW: i32 = 0;
const AUDIOSTREAM_SOURCE_TYPE_MIC: i32 = 0;
const AUDIOSTREAM_SAMPLE_RATE: i32 = 48_000;
const AUDIOSTREAM_CHANNEL_COUNT: i32 = 2;
const AUDIOSTREAM_CALLBACK_FRAME_SIZE: i32 = 960;

#[repr(C)]
struct OH_AudioStreamBuilder {
    _private: [u8; 0],
}

#[repr(C)]
struct OH_AudioCapturer {
    _private: [u8; 0],
}

type CapturerReadCallback =
    Option<unsafe extern "C" fn(*mut OH_AudioCapturer, *mut c_void, *mut c_void, i32)>;

#[link(name = "ohaudio")]
unsafe extern "C" {
    fn OH_AudioStreamBuilder_Create(builder: *mut *mut OH_AudioStreamBuilder, kind: i32) -> i32;
    fn OH_AudioStreamBuilder_Destroy(builder: *mut OH_AudioStreamBuilder) -> i32;
    fn OH_AudioStreamBuilder_SetSamplingRate(builder: *mut OH_AudioStreamBuilder, rate: i32)
        -> i32;
    fn OH_AudioStreamBuilder_SetChannelCount(
        builder: *mut OH_AudioStreamBuilder,
        channels: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetSampleFormat(
        builder: *mut OH_AudioStreamBuilder,
        format: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetEncodingType(
        builder: *mut OH_AudioStreamBuilder,
        encoding: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetCapturerInfo(
        builder: *mut OH_AudioStreamBuilder,
        source_type: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetFrameSizeInCallback(
        builder: *mut OH_AudioStreamBuilder,
        frame_size: i32,
    ) -> i32;
    fn OH_AudioStreamBuilder_SetCapturerReadDataCallback(
        builder: *mut OH_AudioStreamBuilder,
        callback: CapturerReadCallback,
        user_data: *mut c_void,
    ) -> i32;
    fn OH_AudioStreamBuilder_GenerateCapturer(
        builder: *mut OH_AudioStreamBuilder,
        capturer: *mut *mut OH_AudioCapturer,
    ) -> i32;
    fn OH_AudioCapturer_Start(capturer: *mut OH_AudioCapturer) -> i32;
    fn OH_AudioCapturer_Stop(capturer: *mut OH_AudioCapturer) -> i32;
    fn OH_AudioCapturer_Release(capturer: *mut OH_AudioCapturer) -> i32;
}

unsafe extern "C" fn read_pcm(
    _capturer: *mut OH_AudioCapturer,
    _user_data: *mut c_void,
    audio_data: *mut c_void,
    audio_data_size: i32,
) {
    if audio_data.is_null() || audio_data_size <= 0 {
        return;
    }
    let byte_len = audio_data_size as usize;
    if byte_len % std::mem::size_of::<i16>() != 0 {
        return;
    }
    let samples = unsafe {
        std::slice::from_raw_parts(
            audio_data as *const i16,
            byte_len / std::mem::size_of::<i16>(),
        )
    };
    let mut pcm = Vec::with_capacity(samples.len() * std::mem::size_of::<f32>());
    for sample in samples {
        pcm.extend_from_slice(&((*sample as f32 / i16::MAX as f32).clamp(-1.0, 1.0)).to_ne_bytes());
    }
    crate::platform::ohos::push_host_audio_f32_stereo(&pcm);
}

pub(crate) struct OhosAudioInput {
    capturer: *mut OH_AudioCapturer,
}

impl OhosAudioInput {
    pub(crate) fn start() -> Result<Self, String> {
        let mut builder: *mut OH_AudioStreamBuilder = ptr::null_mut();
        let create_result =
            unsafe { OH_AudioStreamBuilder_Create(&mut builder, AUDIOSTREAM_TYPE_CAPTURER) };
        if create_result != AUDIOSTREAM_SUCCESS || builder.is_null() {
            return Err(format!(
                "OH_AudioStreamBuilder_Create failed: {create_result}"
            ));
        }

        let setup_steps = [
            unsafe { OH_AudioStreamBuilder_SetSamplingRate(builder, AUDIOSTREAM_SAMPLE_RATE) },
            unsafe { OH_AudioStreamBuilder_SetChannelCount(builder, AUDIOSTREAM_CHANNEL_COUNT) },
            unsafe { OH_AudioStreamBuilder_SetSampleFormat(builder, AUDIOSTREAM_SAMPLE_S16LE) },
            unsafe {
                OH_AudioStreamBuilder_SetEncodingType(builder, AUDIOSTREAM_ENCODING_TYPE_RAW)
            },
            unsafe { OH_AudioStreamBuilder_SetCapturerInfo(builder, AUDIOSTREAM_SOURCE_TYPE_MIC) },
            unsafe {
                OH_AudioStreamBuilder_SetFrameSizeInCallback(
                    builder,
                    AUDIOSTREAM_CALLBACK_FRAME_SIZE,
                )
            },
            unsafe {
                OH_AudioStreamBuilder_SetCapturerReadDataCallback(
                    builder,
                    Some(read_pcm),
                    ptr::null_mut(),
                )
            },
        ];
        if let Some((index, result)) = setup_steps
            .into_iter()
            .enumerate()
            .find(|(_, result)| *result != AUDIOSTREAM_SUCCESS)
        {
            unsafe {
                OH_AudioStreamBuilder_Destroy(builder);
            }
            return Err(format!(
                "OHAudio capturer setup step {index} failed: {result}"
            ));
        }

        let mut capturer: *mut OH_AudioCapturer = ptr::null_mut();
        let generate_result =
            unsafe { OH_AudioStreamBuilder_GenerateCapturer(builder, &mut capturer) };
        unsafe {
            OH_AudioStreamBuilder_Destroy(builder);
        }
        if generate_result != AUDIOSTREAM_SUCCESS || capturer.is_null() {
            return Err(format!(
                "OH_AudioStreamBuilder_GenerateCapturer failed: {generate_result}"
            ));
        }

        let start_result = unsafe { OH_AudioCapturer_Start(capturer) };
        if start_result != AUDIOSTREAM_SUCCESS {
            let release_result = unsafe { OH_AudioCapturer_Release(capturer) };
            if release_result != AUDIOSTREAM_SUCCESS {
                log::error!(
                    "OH_AudioCapturer_Release after start failure returned {release_result}"
                );
            }
            return Err(format!("OH_AudioCapturer_Start failed: {start_result}"));
        }
        log::info!("OHOS host audio capture started");
        Ok(Self { capturer })
    }
}

impl Drop for OhosAudioInput {
    fn drop(&mut self) {
        if self.capturer.is_null() {
            return;
        }
        let stop_result = unsafe { OH_AudioCapturer_Stop(self.capturer) };
        if stop_result != AUDIOSTREAM_SUCCESS {
            log::warn!("OH_AudioCapturer_Stop failed: {stop_result}");
        }
        let release_result = unsafe { OH_AudioCapturer_Release(self.capturer) };
        if release_result != AUDIOSTREAM_SUCCESS {
            log::error!("OH_AudioCapturer_Release failed: {release_result}");
        }
        self.capturer = ptr::null_mut();
        log::info!("OHOS host audio capture stopped");
    }
}
