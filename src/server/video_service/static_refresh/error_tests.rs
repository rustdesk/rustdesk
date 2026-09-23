use super::*;
use base::message_proto::VideoFrame;
use scrap::{
    codec::{EncoderApi, EncoderCfg, BR_BALANCED},
    EncodeYuvFormat,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Copy)]
enum Outcome {
    Frame,
    Empty,
    Dropped,
    Error,
}

struct State {
    outcomes: VecDeque<Outcome>,
    calls: usize,
    disabled: usize,
}

struct ScriptedEncoder(Rc<RefCell<State>>);

impl EncoderApi for ScriptedEncoder {
    fn new(_: EncoderCfg, _: bool) -> ResultType<Self> {
        unreachable!()
    }
    fn encode_to_message(&mut self, _: EncodeInput, _: i64) -> ResultType<VideoFrame> {
        let mut state = self.0.borrow_mut();
        state.calls += 1;
        match state.outcomes.pop_front().expect("unexpected encode") {
            Outcome::Frame => {
                let mut vf = VideoFrame::new();
                vf.set_h264s(Default::default());
                Ok(vf)
            }
            Outcome::Empty => Ok(VideoFrame::new()),
            Outcome::Dropped => bail!("no valid frame"),
            Outcome::Error => bail!("injected encoding error"),
        }
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
        true
    }
    fn disable(&self) {
        self.0.borrow_mut().disabled += 1;
    }
}

fn encoder(outcomes: &[Outcome]) -> (Encoder, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State {
        outcomes: outcomes.iter().copied().collect(),
        calls: 0,
        disabled: 0,
    }));
    (
        Encoder {
            codec: Box::new(ScriptedEncoder(Rc::clone(&state))),
        },
        state,
    )
}

fn attempt(refresh: &mut StaticRefresh<'_>, encoder: &mut Encoder) {
    refresh.elapsed_since_encode = Duration::from_secs(1);
    refresh
        .try_encode(
            &[1],
            Duration::from_millis(100),
            BR_BALANCED,
            Instant::now(),
            0,
            encoder,
            &mut VideoFrameController::new(0),
        )
        .unwrap();
}

#[test]
fn failures_pause_refresh_until_a_real_frame_succeeds() {
    use Outcome::*;
    let sp = GenericService::new("repeat-errors-test".to_owned(), false);
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
    let (mut encoder, state) = encoder(&[Error, Error, Empty, Frame, Error, Error, Error, Frame]);
    refresh.on_frame(&EncodeInput::YUV(&[1]));
    refresh.on_encoded(true);
    attempt(&mut refresh, &mut encoder);
    refresh
        .try_encode(
            &[1],
            Duration::from_millis(100),
            BR_BALANCED,
            Instant::now(),
            0,
            &mut encoder,
            &mut VideoFrameController::new(0),
        )
        .unwrap();
    assert_eq!(
        state.borrow().calls,
        1,
        "failure must retain the rate limit"
    );
    attempt(&mut refresh, &mut encoder);
    attempt(&mut refresh, &mut encoder);
    assert_eq!(
        refresh.repeat_failures, 2,
        "empty output is not an encoding failure"
    );
    attempt(&mut refresh, &mut encoder);
    assert_eq!(refresh.repeat_failures, 0);
    for _ in 0..3 {
        attempt(&mut refresh, &mut encoder);
    }
    assert_eq!(refresh.repeat_failures, 3);
    attempt(&mut refresh, &mut encoder);
    assert_eq!(state.borrow().calls, 7);
    assert_eq!(state.borrow().disabled, 0);

    refresh.on_frame(&EncodeInput::YUV(&[1]));
    refresh.on_encoded(false);
    attempt(&mut refresh, &mut encoder);
    assert_eq!(
        state.borrow().calls,
        7,
        "a failed real frame must not restart refresh"
    );
    refresh.on_frame(&EncodeInput::YUV(&[1]));
    refresh.on_encoded(true);
    attempt(&mut refresh, &mut encoder);
    assert_eq!(state.borrow().calls, 8);
    assert_eq!(refresh.repeat_failures, 0);
    assert_eq!(state.borrow().disabled, 0);
}

#[test]
fn empty_outputs_exhaust_the_no_output_budget_without_disabling_the_encoder() {
    let sp = GenericService::new("repeat-empty-test".to_owned(), false);
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
    let (mut encoder, state) = encoder(&[Outcome::Empty; 100]);
    refresh.on_frame(&EncodeInput::YUV(&[1]));
    refresh.on_encoded(true);
    for _ in 0..101 {
        attempt(&mut refresh, &mut encoder);
    }
    assert_eq!(state.borrow().calls, 100);
    assert_eq!(state.borrow().disabled, 0);
    assert_eq!(refresh.repeat_failures, 0);
    assert_eq!(refresh.repeat_output_counter, 0);
}

#[test]
fn vpx_drops_exhaust_the_no_output_budget_without_disabling_the_encoder() {
    let sp = GenericService::new("repeat-vpx-drops-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let limit = MAX_REPEAT_NO_OUTPUTS;
        let (mut encoder, state) = encoder(&vec![Outcome::Dropped; limit]);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..=limit {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, limit, "{codec:?}");
        assert_eq!(refresh.repeat_failures, limit);
        assert_eq!(state.borrow().disabled, 0);
        assert_eq!(refresh.repeat_output_counter, 0);
    }
}

#[test]
fn unsuccessful_repeats_preserve_the_frame_budget() {
    let sp = GenericService::new("repeat-frame-budget-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let mut outcomes = vec![Outcome::Dropped; 40];
        outcomes.extend(vec![Outcome::Empty; 40]);
        outcomes.extend(vec![Outcome::Frame; 100]);
        let (mut encoder, state) = encoder(&outcomes);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..80 {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(refresh.repeat_output_counter, 0, "{codec:?}");
        for _ in 0..101 {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, 180, "{codec:?}");
        assert_eq!(refresh.repeat_output_counter, 100);
        assert_eq!(refresh.repeat_failures, 0);
        assert_eq!(state.borrow().disabled, 0);
    }
}

#[test]
fn no_output_budget_is_cumulative_and_restarts_with_a_new_frame() {
    let sp = GenericService::new("repeat-no-output-budget-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let mut outcomes = Vec::new();
        for _ in 0..50 {
            outcomes.extend([Outcome::Frame, Outcome::Empty, Outcome::Dropped]);
        }
        outcomes.push(Outcome::Empty);
        outcomes.push(Outcome::Frame);
        let (mut encoder, state) = encoder(&outcomes);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..151 {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, 150, "{codec:?}");
        assert_eq!(refresh.repeat_output_counter, 50);
        assert_eq!(state.borrow().disabled, 0);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(false);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 150);
        refresh.on_encoded(true);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 151);
        assert_eq!(refresh.repeat_output_counter, 0);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 152);
        assert_eq!(refresh.repeat_output_counter, 1);
    }
}

#[test]
fn repeat_error_limits_are_codec_specific() {
    let sp = GenericService::new("repeat-codec-errors-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for (codec, expected_calls) in [
        (CodecFormat::VP8, 100),
        (CodecFormat::VP9, 100),
        (CodecFormat::AV1, 0),
        (CodecFormat::H264, 3),
        (CodecFormat::H265, 3),
    ] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let limit = MAX_REPEAT_NO_OUTPUTS;
        let (mut encoder, state) = encoder(&vec![Outcome::Error; limit]);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..=limit {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, expected_calls, "{codec:?}");
        assert_eq!(refresh.repeat_failures, expected_calls);
        assert_eq!(state.borrow().disabled, 0);
    }
}

#[test]
fn repeats_leave_normal_failure_tracking_and_switching_intact() {
    use super::super::handle_one_frame;
    use Outcome::*;
    let sp = GenericService::new("repeat-normal-errors-test".to_owned(), false);
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
    let normal_limit = if cfg!(target_os = "android") { 9 } else { 3 };
    let mut outcomes = vec![Error, Error, Frame];
    outcomes.extend(vec![Error; normal_limit - 1]);
    let (mut encoder, state) = encoder(&outcomes);
    refresh.on_frame(&EncodeInput::YUV(&[1]));
    refresh.on_encoded(true);
    let mut normal_failures = 0;
    let mut first_frame = false;
    handle_one_frame(
        0,
        &sp,
        EncodeInput::YUV(&[1]),
        0,
        &mut encoder,
        recorder.clone(),
        &mut normal_failures,
        &mut first_frame,
        1,
        1,
    )
    .unwrap();
    assert_eq!(normal_failures, 1);
    attempt(&mut refresh, &mut encoder);
    attempt(&mut refresh, &mut encoder);
    assert_eq!(normal_failures, 1);
    assert_eq!(state.borrow().disabled, 0);
    for expected in 2..normal_limit {
        handle_one_frame(
            0,
            &sp,
            EncodeInput::YUV(&[1]),
            0,
            &mut encoder,
            recorder.clone(),
            &mut normal_failures,
            &mut first_frame,
            1,
            1,
        )
        .unwrap();
        assert_eq!(normal_failures, expected);
    }
    let result = handle_one_frame(
        0,
        &sp,
        EncodeInput::YUV(&[1]),
        0,
        &mut encoder,
        recorder.clone(),
        &mut normal_failures,
        &mut first_frame,
        1,
        1,
    );
    assert_eq!(result.unwrap_err().to_string(), "SWITCH");
    assert_eq!(state.borrow().disabled, 1);
}
