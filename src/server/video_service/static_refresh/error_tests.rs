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
    inputs: Vec<Vec<u8>>,
}

struct ScriptedEncoder(Rc<RefCell<State>>);

impl EncoderApi for ScriptedEncoder {
    fn new(_: EncoderCfg, _: bool) -> ResultType<Self> {
        unreachable!()
    }
    fn encode_to_message(&mut self, input: EncodeInput, _: i64) -> ResultType<VideoFrame> {
        let mut state = self.0.borrow_mut();
        state.calls += 1;
        if let EncodeInput::YUV(yuv) = input {
            state.inputs.push(yuv.to_vec());
        }
        match state.outcomes.pop_front().expect("unexpected encode") {
            Outcome::Frame => {
                let mut vf = VideoFrame::new();
                vf.set_h264s(Default::default());
                Ok(vf)
            }
            Outcome::Empty => Ok(VideoFrame::new()),
            Outcome::Dropped => Err(scrap::VpxNoOutput.into()),
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
        inputs: Vec::new(),
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
fn vpx_dropped_capture_repeats_the_latest_frame() {
    use super::super::handle_one_frame;
    use Outcome::*;
    let sp = GenericService::new("repeat-dropped-capture-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let (mut encoder, state) = encoder(&[Frame, Dropped, Dropped, Frame]);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(refresh.repeat_output_counter, 1);

        let latest_yuv = [2];
        refresh.on_frame(&EncodeInput::YUV(&latest_yuv));
        let mut normal_failures = 0;
        let mut first_frame = false;
        let result = handle_one_frame(
            0,
            &sp,
            EncodeInput::YUV(&latest_yuv),
            100,
            &mut encoder,
            recorder.clone(),
            &mut normal_failures,
            &mut first_frame,
            1,
            1,
        )
        .unwrap();
        assert!(result.send_conn_ids.is_empty());
        assert!(result.vpx_no_output);
        refresh.on_frame_encoded(&result);
        assert_eq!(refresh.repeat_output_counter, 0);
        for (elapsed_ms, expected_calls, expected_outputs, expected_no_outputs) in
            [(99, 2, 0, 0), (100, 3, 0, 1), (100, 4, 1, 1)]
        {
            refresh.elapsed_since_encode = Duration::from_millis(elapsed_ms);
            refresh
                .try_encode(
                    &latest_yuv,
                    Duration::from_nanos(33_333_333),
                    BR_BALANCED,
                    Instant::now(),
                    expected_calls as i64 * 100,
                    &mut encoder,
                    &mut VideoFrameController::new(0),
                )
                .unwrap();
            assert_eq!(state.borrow().calls, expected_calls, "{codec:?}");
            assert_eq!(refresh.repeat_output_counter, expected_outputs);
            assert_eq!(refresh.repeat_no_outputs, expected_no_outputs);
        }
        assert_eq!(state.borrow().inputs, vec![vec![1], vec![2], vec![2], vec![2]]);
        assert_eq!(state.borrow().disabled, 0);
        assert_eq!(normal_failures, 1);
    }
}

#[test]
fn captured_frame_errors_do_not_enable_repeat() {
    use super::super::handle_one_frame;
    let sp = GenericService::new("repeat-capture-error-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for (codec, outcome, vpx_no_output) in [
        (CodecFormat::VP8, Outcome::Error, false),
        (CodecFormat::VP9, Outcome::Error, false),
        (CodecFormat::H264, Outcome::Dropped, true),
        (CodecFormat::H265, Outcome::Dropped, true),
    ] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let (mut encoder, state) = encoder(&[outcome, Outcome::Frame]);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        let mut normal_failures = 0;
        let mut first_frame = false;
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
        )
        .unwrap();
        assert!(result.send_conn_ids.is_empty());
        assert_eq!(result.vpx_no_output, vpx_no_output);
        refresh.on_frame_encoded(&result);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 1, "{codec:?}");
        assert_eq!(refresh.repeat_output_counter, 0);
        assert_eq!(state.borrow().disabled, 0);
    }
}

#[test]
fn unsuccessful_repeats_preserve_the_frame_budget() {
    let sp = GenericService::new("repeat-frame-budget-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let limit = max_repeat_frames(codec, BR_BALANCED);
        let mut outcomes = vec![Outcome::Dropped; 40];
        outcomes.extend(vec![Outcome::Empty; 40]);
        outcomes.extend(vec![Outcome::Frame; limit + 1]);
        let (mut encoder, state) = encoder(&outcomes);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..80 {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(refresh.repeat_output_counter, 0, "{codec:?}");
        for _ in 0..=limit {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, 80 + limit, "{codec:?}");
        assert_eq!(refresh.repeat_output_counter, limit);
        assert_eq!(refresh.repeat_failures, 0);
        assert_eq!(state.borrow().disabled, 0);
    }
}

#[test]
fn repeat_intervals_are_codec_specific_after_the_first_attempt() {
    let sp = GenericService::new("repeat-interval-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for (codec, spf_ms, first_interval_ms, repeat_interval_ms) in [
        (CodecFormat::VP8, 10, 100, 100),
        (CodecFormat::VP9, 10, 100, 100),
        (CodecFormat::VP8, 200, 200, 200),
        (CodecFormat::VP9, 200, 200, 200),
        (CodecFormat::H264, 10, 100, 10),
        (CodecFormat::H265, 10, 100, 10),
    ] {
        for first in [
            Outcome::Frame,
            Outcome::Empty,
            Outcome::Dropped,
            Outcome::Error,
        ] {
            let mut refresh =
                StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
            let (mut encoder, state) = encoder(&[first, Outcome::Frame]);
            refresh.on_frame(&EncodeInput::YUV(&[1]));
            refresh.on_encoded(true);
            for (elapsed_ms, expected_calls) in [
                (first_interval_ms - 1, 0),
                (first_interval_ms, 1),
                (repeat_interval_ms - 1, 1),
                (repeat_interval_ms, 2),
            ] {
                refresh.elapsed_since_encode = Duration::from_millis(elapsed_ms);
                refresh
                    .try_encode(
                        &[1],
                        Duration::from_millis(spf_ms),
                        BR_BALANCED,
                        Instant::now(),
                        0,
                        &mut encoder,
                        &mut VideoFrameController::new(0),
                    )
                    .unwrap();
                assert_eq!(state.borrow().calls, expected_calls, "{codec:?}");
            }
        }
    }
}

#[test]
fn repeat_timing_uses_the_capture_loop_start() {
    let sp = GenericService::new("repeat-loop-clock-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    let spf = Duration::from_nanos(33_333_333);
    let clock_start = Instant::now() + Duration::from_secs(1);
    for first in [
        Outcome::Frame,
        Outcome::Empty,
        Outcome::Dropped,
        Outcome::Error,
    ] {
        let mut refresh = StaticRefresh::new(
            VideoSource::Monitor,
            CodecFormat::H264,
            &sp,
            &recorder,
            0,
            1,
            1,
        );
        let (mut encoder, state) = encoder(&[first, Outcome::Frame]);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        refresh.last_encode = clock_start;
        for (index, loop_start) in [
            clock_start + INITIAL_REPEAT_INTERVAL,
            clock_start + INITIAL_REPEAT_INTERVAL + spf,
        ]
        .into_iter()
        .enumerate()
        {
            // Capture processing finishes partway through the current loop.
            let capture_end = loop_start + Duration::from_millis(5);
            refresh.elapsed_since_encode = capture_end.duration_since(refresh.last_encode);
            refresh
                .try_encode(
                    &[1],
                    spf,
                    BR_BALANCED,
                    loop_start,
                    0,
                    &mut encoder,
                    &mut VideoFrameController::new(0),
                )
                .unwrap();
            assert_eq!(state.borrow().calls, index + 1);
            assert_eq!(refresh.last_encode, loop_start);
        }
    }
}

#[test]
fn vpx_repeat_interval_starts_after_encoding() {
    let sp = GenericService::new("repeat-vpx-clock-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        for outcome in [
            Outcome::Frame,
            Outcome::Empty,
            Outcome::Dropped,
            Outcome::Error,
        ] {
            let mut refresh =
                StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
            let (mut encoder, state) = encoder(&[outcome]);
            refresh.on_frame(&EncodeInput::YUV(&[1]));
            refresh.on_encoded(true);
            refresh.elapsed_since_encode = INITIAL_REPEAT_INTERVAL;
            let loop_start = Instant::now();
            std::thread::sleep(Duration::from_millis(1));
            let before_encode = Instant::now();
            refresh
                .try_encode(
                    &[1],
                    Duration::from_nanos(33_333_333),
                    BR_BALANCED,
                    loop_start,
                    0,
                    &mut encoder,
                    &mut VideoFrameController::new(0),
                )
                .unwrap();
            assert_eq!(state.borrow().calls, 1);
            assert!(refresh.last_encode >= before_encode, "{codec:?}");
            assert!(refresh.last_encode <= Instant::now());
        }
    }
}

#[test]
fn no_output_budget_is_cumulative_and_restarts_with_a_new_frame() {
    let sp = GenericService::new("repeat-no-output-budget-test".to_owned(), false);
    let recorder = Arc::new(Mutex::new(None));
    for codec in [CodecFormat::VP8, CodecFormat::VP9] {
        let mut refresh = StaticRefresh::new(VideoSource::Monitor, codec, &sp, &recorder, 0, 1, 1);
        let mut outcomes = Vec::new();
        for _ in 0..10 {
            outcomes.push(Outcome::Frame);
            for _ in 0..5 {
                outcomes.extend([Outcome::Empty, Outcome::Dropped]);
            }
        }
        outcomes.push(Outcome::Empty);
        outcomes.push(Outcome::Frame);
        let (mut encoder, state) = encoder(&outcomes);
        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(true);
        for _ in 0..111 {
            attempt(&mut refresh, &mut encoder);
        }
        assert_eq!(state.borrow().calls, 110, "{codec:?}");
        assert_eq!(refresh.repeat_output_counter, 10);
        assert_eq!(state.borrow().disabled, 0);

        refresh.on_frame(&EncodeInput::YUV(&[1]));
        refresh.on_encoded(false);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 110);
        refresh.on_encoded(true);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 111);
        assert_eq!(refresh.repeat_output_counter, 0);
        attempt(&mut refresh, &mut encoder);
        assert_eq!(state.borrow().calls, 112);
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
