use super::{handle_one_frame, GenericService, VideoFrameController, VideoSource};
use hbb_common::ResultType;
use scrap::{codec::Encoder, record::Recorder, EncodeInput};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) struct StaticRefresh<'a> {
    source: VideoSource,
    sp: &'a GenericService,
    recorder: &'a Arc<Mutex<Option<Recorder>>>,
    display_idx: usize,
    width: usize,
    height: usize,
    last_encode: Instant,
    repeat_counter: usize,
    #[cfg(all(windows, feature = "vram"))]
    repeat_texture: Option<scrap::dxgi::repeat::RepeatTexture>,
}

impl<'a> StaticRefresh<'a> {
    pub(super) fn new(
        source: VideoSource,
        sp: &'a GenericService,
        recorder: &'a Arc<Mutex<Option<Recorder>>>,
        display_idx: usize,
        width: usize,
        height: usize,
    ) -> Self {
        Self {
            source,
            sp,
            recorder,
            display_idx,
            width,
            height,
            last_encode: Instant::now(),
            repeat_counter: 0,
            #[cfg(all(windows, feature = "vram"))]
            repeat_texture: Some(scrap::dxgi::repeat::RepeatTexture::default()),
        }
    }

    pub(super) fn on_frame(&mut self, _frame: &EncodeInput) {
        self.repeat_counter = 0;
        #[cfg(all(windows, feature = "vram"))]
        if self.source.is_monitor() {
            if let Some(texture) = self.repeat_texture.as_mut() {
                if let Err(err) = texture.update(_frame) {
                    hbb_common::log::warn!("Disable static texture refresh: {err}");
                    self.repeat_texture = None;
                }
            }
        }
    }

    pub(super) fn on_encoded(&mut self) {
        self.last_encode = Instant::now();
    }

    pub(super) fn try_encode(
        &mut self,
        yuv: &[u8],
        spf: Duration,
        now: Instant,
        ms: i64,
        encoder: &mut Encoder,
        encode_fail_counter: &mut usize,
        first_frame: &mut bool,
        sent_counter: &mut usize,
        frame_controller: &mut VideoFrameController,
    ) -> ResultType<()> {
        if !self.source.is_monitor()
            || self.repeat_counter >= 300
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
        let frame = frame.or_else(|| self.repeat_texture.as_ref().and_then(|t| t.frame()));
        if let Some(frame) = frame {
            let send_conn_ids = handle_one_frame(
                self.display_idx,
                self.sp,
                frame,
                ms,
                encoder,
                self.recorder.clone(),
                encode_fail_counter,
                first_frame,
                self.width,
                self.height,
            )?;
            self.repeat_counter += 1;
            if !send_conn_ids.is_empty() {
                *sent_counter += 1;
            }
            self.on_encoded();
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
            assert_eq!(frame.yuv()?, &[1]);
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
                &mut false,
                &mut 0,
                &mut VideoFrameController::new(0),
            )
            .unwrap();
    }

    #[test]
    fn long_network_waits_preserve_budget_and_new_frames_restart_it() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, &sp, &recorder, 0, 1, 1);
        let calls = Rc::new(Cell::new(0));
        let spf = Duration::from_millis(100);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        for _ in 0..300 {
            refresh.last_encode = Instant::now() - Duration::from_secs(60);
            attempt(&mut refresh, &calls, &[1], spf);
        }
        assert_eq!(calls.get(), 300);

        refresh.last_encode = Instant::now() - Duration::from_secs(600);
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), 300);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        attempt(&mut refresh, &calls, &[1], spf);
        assert_eq!(calls.get(), 301);
    }

    #[test]
    fn recent_encodes_and_qos_limit_refresh_rate() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, &sp, &recorder, 0, 1, 1);
        let calls = Rc::new(Cell::new(0));
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded();
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
    fn cameras_and_missing_frames_are_not_refreshed() {
        let sp = GenericService::new("static-refresh-test".to_owned(), false);
        let recorder = Arc::new(Mutex::new(None));
        let calls = Rc::new(Cell::new(0));
        for (source, yuv) in [
            (VideoSource::Camera, &[1][..]),
            (VideoSource::Monitor, &[][..]),
        ] {
            let mut refresh = StaticRefresh::new(source, &sp, &recorder, 0, 1, 1);
            refresh.last_encode = Instant::now() - Duration::from_secs(60);
            attempt(&mut refresh, &calls, yuv, Duration::from_millis(100));
        }
        assert_eq!(calls.get(), 0);
    }
}
