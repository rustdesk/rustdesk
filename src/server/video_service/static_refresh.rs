use super::{GenericService, VideoFrameController, VideoSource};
use base::message_proto::Message;
use hbb_common::{bail, log, ResultType};
use scrap::{codec::Encoder, record::Recorder, CodecFormat, EncodeInput};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) struct StaticRefresh<'a> {
    source: VideoSource,
    codec_format: CodecFormat,
    sp: &'a GenericService,
    recorder: &'a Arc<Mutex<Option<Recorder>>>,
    display_idx: usize,
    width: usize,
    height: usize,
    last_encode: Instant,
    repeat_counter: usize,
    repeat_failures: usize,
    source_ready: bool,
    #[cfg(all(windows, feature = "vram"))]
    has_texture: bool,
}

impl<'a> StaticRefresh<'a> {
    pub(super) fn new(
        source: VideoSource,
        codec_format: CodecFormat,
        sp: &'a GenericService,
        recorder: &'a Arc<Mutex<Option<Recorder>>>,
        display_idx: usize,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            source,
            codec_format,
            sp,
            recorder,
            display_idx,
            width,
            height,
            last_encode: Instant::now(),
            repeat_counter: 0,
            repeat_failures: 0,
            source_ready: false,
            #[cfg(all(windows, feature = "vram"))]
            has_texture: false,
        }
    }

    pub(super) fn on_frame(&mut self, _frame: &EncodeInput) {
        self.repeat_counter = 0;
        self.source_ready = false;
        #[cfg(all(windows, feature = "vram"))]
        {
            self.has_texture =
                matches!(_frame, EncodeInput::Texture((texture, 0)) if !texture.is_null());
        }
    }

    pub(super) fn on_encoded(&mut self, success: bool) {
        self.last_encode = Instant::now();
        if success {
            self.source_ready = true;
            self.repeat_failures = 0;
        }
    }

    pub(super) fn try_encode(
        &mut self,
        yuv: &[u8],
        spf: Duration,
        now: Instant,
        ms: i64,
        encoder: &mut Encoder,
        sent_counter: &mut usize,
        frame_controller: &mut VideoFrameController,
    ) -> ResultType<()> {
        if !self.source.is_monitor()
            || self.codec_format == CodecFormat::AV1
            || !self.source_ready
            || self.repeat_failures >= 3
            || self.repeat_counter >= 100
            || self.last_encode.elapsed() < Duration::from_millis(100).max(spf)
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

            self.repeat_counter += 1;
            let result = encoder.encode_to_message(frame, ms);
            self.last_encode = Instant::now();
            let mut vf = match result {
                Ok(vf) if vf.union.is_none() => return Ok(()),
                Ok(vf) => vf,
                Err(error) => {
                    self.repeat_failures += 1;
                    log::debug!(
                        "static refresh failed ({}/3): {error:?}",
                        self.repeat_failures
                    );
                    return Ok(());
                }
            };
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
            if !send_conn_ids.is_empty() {
                *sent_counter += 1;
            }
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
        codec::{EncoderApi, EncoderCfg},
        EncodeYuvFormat,
    };
    use std::{cell::Cell, rc::Rc};

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
            Ok(VideoFrame::new())
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
                Instant::now(),
                0,
                &mut encoder,
                &mut 0,
                &mut VideoFrameController::new(0),
            )
            .unwrap();
    }

    #[test]
    fn long_network_waits_preserve_budget_and_new_frames_restart_it() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = StaticRefresh::new(
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
        for _ in 0..100 {
            refresh.last_encode = Instant::now() - Duration::from_secs(60);
            attempt(&mut refresh, &calls, &[1], spf);
        }
        assert_eq!(calls.get(), 100);

        refresh.last_encode = Instant::now() - Duration::from_secs(600);
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), 100);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        refresh.last_encode = Instant::now() - Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), 101);
    }

    #[test]
    fn recent_encodes_and_qos_limit_refresh_rate() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = StaticRefresh::new(
            VideoSource::Monitor,
            CodecFormat::VP9,
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

        refresh.last_encode = Instant::now() - Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 0);

        refresh.last_encode = Instant::now() - Duration::from_secs(3);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 1);
        attempt(&mut refresh, &calls, &[1], Duration::from_secs(2));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    #[cfg(all(windows, feature = "vram"))]
    fn texture_frames_request_repeats_without_accessing_capture_resources() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = StaticRefresh::new(
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
        refresh.last_encode = Instant::now() - Duration::from_secs(1);
        attempt(&mut refresh, &calls, &[], Duration::from_millis(16));
        assert_eq!(calls.get(), 1);

        for frame in [
            EncodeInput::YUV(&[1]),
            EncodeInput::Texture((texture, 90)),
            EncodeInput::Texture((std::ptr::null_mut(), 0)),
        ] {
            refresh.on_frame(&frame);
            refresh.on_encoded(true);
            refresh.last_encode = Instant::now() - Duration::from_secs(1);
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
            let mut refresh = StaticRefresh::new(source, codec_format, &sp, &recorder, 0, 1, 1);
            refresh.on_frame(&EncodeInput::YUV(yuv));
            refresh.on_encoded(true);
            refresh.last_encode = Instant::now() - Duration::from_secs(60);
            attempt(&mut refresh, &calls, yuv, Duration::from_millis(100));
        }
        assert_eq!(calls.get(), 0);
    }
}

#[cfg(test)]
mod error_tests;
