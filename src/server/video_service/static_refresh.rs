use super::{GenericService, VideoFrameController, VideoSource};
use base::message_proto::Message;
use hbb_common::{bail, log, ResultType};
use scrap::{
    codec::{Encoder, EncoderCfg, BR_BALANCED, BR_BEST, BR_SPEED},
    record::Recorder,
    CodecFormat, EncodeInput,
};
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

const INITIAL_REPEAT_INTERVAL: Duration = Duration::from_millis(100);
const MAX_REPEAT_FAILURES: usize = 3;
const MAX_REPEAT_NO_OUTPUTS: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HardwareBackend {
    Nvenc,
    Qsv,
    Amf,
    Other,
}

fn max_repeat_frames(codec: CodecFormat, quality: f32, backend: HardwareBackend) -> usize {
    use HardwareBackend::*;
    let (low, low_to_balanced, balanced_to_best, best) = match (codec, backend) {
        (CodecFormat::VP8, _) => (20, 20, 20, 30),
        (CodecFormat::VP9, _) => (20, 20, 20, 20),
        (CodecFormat::H264, Nvenc) => (50, 50, 60, 30),
        (CodecFormat::H264, Qsv) => (70, 70, 130, 110),
        (CodecFormat::H264, Amf) => (150, 150, 170, 110),
        (CodecFormat::H265, Nvenc) => (90, 90, 60, 30),
        (CodecFormat::H265, Qsv) => (30, 30, 30, 30),
        (CodecFormat::H265, Amf) => (200, 200, 200, 120),
        (CodecFormat::H264, Other) => (90, 60, 60, 30),
        (CodecFormat::H265, Other) => (150, 120, 90, 30),
        (CodecFormat::AV1 | CodecFormat::Unknown, _) => return 0,
    };
    if quality >= BR_BEST {
        best
    } else if quality >= BR_BALANCED {
        balanced_to_best
    } else if quality > BR_SPEED {
        low_to_balanced
    } else {
        low
    }
}

#[derive(Debug)]
pub(super) struct FrameEncodeResult {
    pub(super) send_conn_ids: HashSet<i32>,
    pub(super) vpx_no_output: bool,
}

pub(super) struct StaticRefresh<'a> {
    source: VideoSource,
    codec_format: CodecFormat,
    hardware_backend: HardwareBackend,
    sp: &'a GenericService,
    recorder: &'a Arc<Mutex<Option<Recorder>>>,
    display_idx: usize,
    width: usize,
    height: usize,
    last_encode: Instant,
    #[cfg(test)]
    elapsed_since_encode: Duration,
    repeat_output_counter: usize,
    repeat_no_outputs: usize,
    repeat_failures: usize,
    source_ready: bool,
    #[cfg(all(windows, feature = "vram"))]
    has_texture: bool,
}

impl<'a> StaticRefresh<'a> {
    pub(super) fn new(
        source: VideoSource,
        codec_format: CodecFormat,
        encoder_config: &EncoderCfg,
        sp: &'a GenericService,
        recorder: &'a Arc<Mutex<Option<Recorder>>>,
        display_idx: usize,
        width: usize,
        height: usize,
    ) -> Self {
        let hardware_backend = match encoder_config {
            #[cfg(feature = "hwcodec")]
            EncoderCfg::HWRAM(config) => match config.name.as_str() {
                "h264_nvenc" | "hevc_nvenc" => HardwareBackend::Nvenc,
                "h264_qsv" | "hevc_qsv" => HardwareBackend::Qsv,
                "h264_amf" | "hevc_amf" => HardwareBackend::Amf,
                _ => HardwareBackend::Other,
            },
            #[cfg(feature = "vram")]
            // The FFMPEG VRAM driver selects the encoder from the adapter vendor.
            EncoderCfg::VRAM(config) => match config.feature.vendor {
                scrap::vram::Driver::NV => HardwareBackend::Nvenc,
                scrap::vram::Driver::MFX => HardwareBackend::Qsv,
                scrap::vram::Driver::AMF => HardwareBackend::Amf,
                _ => HardwareBackend::Other,
            },
            _ => HardwareBackend::Other,
        };
        Self {
            source,
            codec_format,
            hardware_backend,
            sp,
            recorder,
            display_idx,
            width,
            height,
            last_encode: Instant::now(),
            #[cfg(test)]
            elapsed_since_encode: Duration::ZERO,
            repeat_output_counter: 0,
            repeat_no_outputs: 0,
            repeat_failures: 0,
            source_ready: false,
            #[cfg(all(windows, feature = "vram"))]
            has_texture: false,
        }
    }

    pub(super) fn on_frame(&mut self, _frame: &EncodeInput) {
        self.repeat_output_counter = 0;
        self.repeat_no_outputs = 0;
        self.source_ready = false;
        #[cfg(all(windows, feature = "vram"))]
        {
            self.has_texture =
                matches!(_frame, EncodeInput::Texture((texture, 0)) if !texture.is_null());
        }
    }

    pub(super) fn on_frame_encoded(&mut self, result: &FrameEncodeResult) {
        // VP8/VP9 rate control may drop a frame without an encoding error.
        // The latest input is still cached, so allow repeats even without output;
        // otherwise the final update may never be sent once capture becomes idle.
        self.on_encoded(
            !result.send_conn_ids.is_empty()
                || (matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9)
                    && result.vpx_no_output),
        );
    }

    pub(super) fn on_encoded(&mut self, success: bool) {
        self.last_encode = Instant::now();
        #[cfg(test)]
        {
            self.elapsed_since_encode = Duration::ZERO;
        }
        if success {
            self.source_ready = true;
            self.repeat_failures = 0;
        }
    }

    pub(super) fn try_encode(
        &mut self,
        yuv: &[u8],
        spf: Duration,
        quality: f32,
        now: Instant,
        ms: i64,
        encoder: &mut Encoder,
        frame_controller: &mut VideoFrameController,
    ) -> ResultType<()> {
        if !self.source.is_monitor()
            || self.codec_format == CodecFormat::AV1
            || !self.source_ready
            || (self.repeat_failures >= MAX_REPEAT_FAILURES
                && !matches!(
                    self.codec_format,
                    CodecFormat::VP8 | CodecFormat::VP9 | CodecFormat::AV1
                ))
            || self.repeat_no_outputs >= MAX_REPEAT_NO_OUTPUTS
            || self.repeat_output_counter
                >= max_repeat_frames(self.codec_format, quality, self.hardware_backend)
            || {
                #[cfg(not(test))]
                let elapsed = self.last_encode.elapsed();
                #[cfg(test)]
                let elapsed = self.elapsed_since_encode;
                let interval = if matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9)
                    || (self.repeat_output_counter == 0 && self.repeat_no_outputs == 0)
                {
                    INITIAL_REPEAT_INTERVAL.max(spf)
                } else {
                    spf
                };
                elapsed < interval
            }
        {
            return Ok(());
        }
        let frame = if yuv.is_empty() {
            None
        } else {
            Some(EncodeInput::YUV(yuv))
        };
        #[cfg(all(windows, feature = "vram"))]
        let frame = frame.or_else(|| self.has_texture.then_some(EncodeInput::Repeat));
        if let Some(frame) = frame {
            self.sp.snapshot(|sps| {
                if sps.has_subscribes() {
                    bail!("SWITCH");
                }
                Ok(())
            })?;

            let result = encoder.encode_to_message(frame, ms);
            self.last_encode = if matches!(self.codec_format, CodecFormat::VP8 | CodecFormat::VP9) {
                Instant::now()
            } else {
                // Align with the capture loop so encoding time does not skip the next tick.
                now
            };
            #[cfg(test)]
            {
                self.elapsed_since_encode = Duration::ZERO;
            }
            let mut vf = match result {
                Ok(vf) if vf.union.is_none() => {
                    self.repeat_no_outputs += 1;
                    return Ok(());
                }
                Ok(vf) => vf,
                Err(error) => {
                    self.repeat_no_outputs += 1;
                    self.repeat_failures += 1;
                    log::debug!(
                        "static refresh failed ({}): {error:?}",
                        self.repeat_failures
                    );
                    return Ok(());
                }
            };
            self.repeat_output_counter += 1;
            self.repeat_failures = 0;
            vf.display = self.display_idx as _;
            let mut msg = Message::new();
            msg.set_video_frame(vf);
            self.recorder
                .lock()
                .unwrap()
                .as_mut()
                .map(|r| r.write_message(&msg, self.width, self.height));
            let send_conn_ids = self.sp.send_video_frame(msg);
            frame_controller.set_send(now, send_conn_ids);
            // Static refinement must not count as motion for adaptive bitrate.
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base::message_proto::VideoFrame;
    use scrap::{
        codec::{EncoderApi, EncoderCfg, BR_BALANCED},
        vpxcodec::{VpxEncoderConfig, VpxVideoCodecId},
        EncodeYuvFormat,
    };
    use std::{cell::Cell, rc::Rc};

    pub(super) fn new_refresh<'a>(
        source: VideoSource,
        codec_format: CodecFormat,
        sp: &'a GenericService,
        recorder: &'a Arc<Mutex<Option<Recorder>>>,
        display_idx: usize,
        width: usize,
        height: usize,
    ) -> StaticRefresh<'a> {
        // The mock encoders simulate each format without a hardware backend.
        let config = EncoderCfg::VPX(VpxEncoderConfig {
            width: width as _,
            height: height as _,
            quality: BR_BALANCED,
            codec: VpxVideoCodecId::VP9,
            keyframe_interval: None,
        });
        StaticRefresh::new(
            source,
            codec_format,
            &config,
            sp,
            recorder,
            display_idx,
            width,
            height,
        )
    }

    struct CountingEncoder(Rc<Cell<usize>>);

    impl EncoderApi for CountingEncoder {
        fn new(_: EncoderCfg, _: bool) -> ResultType<Self> {
            unreachable!()
        }

        fn encode_to_message(&mut self, frame: EncodeInput, _: i64) -> ResultType<VideoFrame> {
            match frame {
                EncodeInput::YUV(yuv) => assert_eq!(yuv, &[1]),
                #[cfg(all(windows, feature = "vram"))]
                EncodeInput::Repeat => {}
                _ => panic!("unexpected encoder input"),
            }
            self.0.set(self.0.get() + 1);
            let mut vf = VideoFrame::new();
            vf.set_h264s(Default::default());
            Ok(vf)
        }

        fn yuvfmt(&self) -> EncodeYuvFormat {
            unreachable!()
        }

        #[cfg(feature = "vram")]
        fn input_texture(&self) -> bool {
            false
        }

        fn set_quality(&mut self, _: f32) -> ResultType<()> {
            unreachable!()
        }

        fn bitrate(&self) -> u32 {
            0
        }

        fn support_changing_quality(&self) -> bool {
            false
        }

        fn latency_free(&self) -> bool {
            true
        }

        fn is_hardware(&self) -> bool {
            false
        }

        fn disable(&self) {
            unreachable!()
        }
    }

    fn attempt(
        refresh: &mut StaticRefresh<'_>,
        calls: &Rc<Cell<usize>>,
        yuv: &[u8],
        spf: Duration,
    ) {
        let mut encoder = Encoder {
            codec: Box::new(CountingEncoder(Rc::clone(calls))),
        };
        refresh
            .try_encode(
                yuv,
                spf,
                BR_BALANCED,
                Instant::now(),
                0,
                &mut encoder,
                &mut VideoFrameController::new(0),
            )
            .unwrap();
    }

    #[test]
    fn long_network_waits_preserve_budget_and_new_frames_restart_it() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = new_refresh(
            VideoSource::Monitor,
            CodecFormat::VP9,
            &sp,
            &recorder,
            0,
            1,
            1,
        );
        let calls = Rc::new(Cell::new(0));
        let spf = Duration::from_millis(100);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        let limit = max_repeat_frames(CodecFormat::VP9, BR_BALANCED, HardwareBackend::Other);
        for _ in 0..limit {
            refresh.elapsed_since_encode = Duration::from_secs(60);
            attempt(&mut refresh, &calls, &[1], spf);
        }
        assert_eq!(calls.get(), limit);

        refresh.elapsed_since_encode = Duration::from_secs(600);
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), limit);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        refresh.elapsed_since_encode = Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), limit + 1);
    }

    #[test]
    fn initial_delay_repeat_interval_and_qos_limit_refresh_rate() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = new_refresh(
            VideoSource::Monitor,
            CodecFormat::H264,
            &sp,
            &recorder,
            0,
            1,
            1,
        );
        let calls = Rc::new(Cell::new(0));
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 0);

        refresh.elapsed_since_encode = Duration::from_millis(50);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 0);

        refresh.elapsed_since_encode = Duration::from_millis(100);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 1);

        refresh.elapsed_since_encode = Duration::from_millis(9);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 1);

        refresh.elapsed_since_encode = Duration::from_millis(10);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 2);

        refresh.elapsed_since_encode = Duration::from_millis(49);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(50));
        assert_eq!(calls.get(), 2);
        refresh.elapsed_since_encode = Duration::from_millis(50);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(50));
        assert_eq!(calls.get(), 3);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        refresh.elapsed_since_encode = Duration::from_millis(50);
        attempt(&mut refresh, &calls, &[1], Duration::from_millis(10));
        assert_eq!(calls.get(), 3);

        refresh.elapsed_since_encode = Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 3);

        refresh.elapsed_since_encode = Duration::from_secs(3);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 4);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 4);
        refresh.elapsed_since_encode = Duration::from_millis(1999);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 4);
        refresh.elapsed_since_encode = Duration::from_millis(2000);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 5);
    }

    #[test]
    #[cfg(all(windows, feature = "vram"))]
    fn texture_frames_request_repeats_without_accessing_capture_resources() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = new_refresh(
            VideoSource::Monitor,
            CodecFormat::H264,
            &sp,
            &recorder,
            0,
            1,
            1,
        );
        let calls = Rc::new(Cell::new(0));
        // Deliberately not a D3D resource: neither hook may dereference it.
        let texture = std::ptr::NonNull::<u8>::dangling().as_ptr() as _;
        for _ in 0..60 {
            refresh.on_frame(&EncodeInput::Texture((texture, 0)));
        }
        refresh.on_encoded(true);
        refresh.elapsed_since_encode = Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[], Duration::from_millis(16));
        assert_eq!(calls.get(), 1);

        for frame in [
            EncodeInput::YUV(&[1]),
            EncodeInput::Texture((texture, 90)),
            EncodeInput::Texture((std::ptr::null_mut(), 0)),
        ] {
            refresh.on_frame(&frame);
            refresh.on_encoded(true);
            refresh.elapsed_since_encode = Duration::from_secs(1);
            attempt(&mut refresh, &calls, &[], Duration::from_millis(16));
            assert_eq!(calls.get(), 1);
        }
    }

    #[test]
    fn cameras_av1_and_missing_frames_are_not_refreshed() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let calls = Rc::new(Cell::new(0));
        for (source, codec_format, yuv) in [
            (VideoSource::Camera, CodecFormat::VP9, &[1][..]),
            (VideoSource::Monitor, CodecFormat::AV1, &[1][..]),
            (VideoSource::Monitor, CodecFormat::VP9, &[][..]),
        ] {
            let mut refresh = new_refresh(source, codec_format, &sp, &recorder, 0, 1, 1);
            refresh.on_frame(&EncodeInput::YUV(yuv));
            refresh.on_encoded(true);
            refresh.elapsed_since_encode = Duration::from_secs(60);
            attempt(&mut refresh, &calls, yuv, Duration::from_millis(100));
        }
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn quality_bands_stop_each_backend_at_its_output_budget() {
        let sp = GenericService::new("repeat-quality-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        for codec in [
            CodecFormat::VP8,
            CodecFormat::VP9,
            CodecFormat::H264,
            CodecFormat::H265,
        ] {
            for backend in [
                HardwareBackend::Nvenc,
                HardwareBackend::Qsv,
                HardwareBackend::Amf,
                HardwareBackend::Other,
            ] {
                let mut refresh = new_refresh(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
                refresh.hardware_backend = backend;
                let calls = Rc::new(Cell::new(0));
                let mut encoder = Encoder {
                    codec: Box::new(CountingEncoder(Rc::clone(&calls))),
                };
                refresh.on_frame(&EncodeInput::YUV(&[1]));
                refresh.on_encoded(true);
                for quality in [
                    BR_BEST,
                    BR_SPEED,
                    BR_BALANCED,
                    BR_SPEED + 0.01,
                    BR_BEST + 0.1,
                ] {
                    let limit = max_repeat_frames(codec, quality, backend);
                    refresh.repeat_output_counter = limit - 1;
                    let before = calls.get();
                    for _ in 0..2 {
                        refresh.elapsed_since_encode = Duration::from_secs(1);
                        refresh
                            .try_encode(
                                &[1],
                                Duration::from_millis(33),
                                quality,
                                Instant::now(),
                                0,
                                &mut encoder,
                                &mut VideoFrameController::new(0),
                            )
                            .unwrap();
                    }
                    assert_eq!(calls.get(), before + 1, "{codec:?} {backend:?} {quality}");
                    assert_eq!(refresh.repeat_output_counter, limit);
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "hwcodec")]
    fn ram_config_selects_the_actual_backend_and_preserves_other_encoders() {
        let sp = GenericService::new("repeat-backend-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        for (name, expected) in [
            ("h264_nvenc", HardwareBackend::Nvenc),
            ("hevc_nvenc", HardwareBackend::Nvenc),
            ("h264_qsv", HardwareBackend::Qsv),
            ("hevc_qsv", HardwareBackend::Qsv),
            ("h264_amf", HardwareBackend::Amf),
            ("hevc_amf", HardwareBackend::Amf),
            ("h264_videotoolbox", HardwareBackend::Other),
        ] {
            let config = EncoderCfg::HWRAM(scrap::hwcodec::HwRamEncoderConfig {
                name: name.to_owned(),
                mc_name: None,
                width: 1920,
                height: 1080,
                quality: BR_BALANCED,
                keyframe_interval: None,
            });
            let codec_format = if name.starts_with("hevc_") {
                CodecFormat::H265
            } else {
                CodecFormat::H264
            };
            let refresh = StaticRefresh::new(
                VideoSource::Monitor,
                codec_format,
                &config,
                &sp,
                &recorder,
                0,
                1920,
                1080,
            );
            assert_eq!(refresh.hardware_backend, expected);
        }
    }

    #[test]
    #[cfg(feature = "vram")]
    fn vram_config_resolves_ffmpeg_to_the_actual_hardware_backend() {
        let sp = GenericService::new("repeat-vram-backend-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        for (vendor, expected) in [
            ("NV", HardwareBackend::Nvenc),
            ("MFX", HardwareBackend::Qsv),
            ("AMF", HardwareBackend::Amf),
            ("FFMPEG", HardwareBackend::Other),
        ] {
            let config = EncoderCfg::VRAM(scrap::vram::VRamEncoderConfig {
                device: Default::default(),
                width: 1920,
                height: 1080,
                quality: BR_BALANCED,
                keyframe_interval: None,
                feature: serde_json::from_value(serde_json::json!({
                    "driver": "FFMPEG",
                    "vendor": vendor,
                    "luid": 0,
                    "data_format": "H264",
                }))
                .unwrap(),
            });
            let refresh = StaticRefresh::new(
                VideoSource::Monitor,
                CodecFormat::H264,
                &config,
                &sp,
                &recorder,
                0,
                1920,
                1080,
            );
            assert_eq!(refresh.hardware_backend, expected);
        }
    }
}

#[cfg(test)]
mod error_tests;
