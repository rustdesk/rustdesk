use super::*;
use base::message_proto::VideoFrame;
use scrap::{
    codec::{EncoderApi, EncoderCfg},
    EncodeYuvFormat,
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Clone, Copy)]
enum Outcome {
    Frame,
    Empty,
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
    refresh.last_encode = Instant::now() - Duration::from_secs(1);
    refresh
        .try_encode(
            &[1],
            Duration::from_millis(100),
            Instant::now(),
            0,
            encoder,
            &mut 0,
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
            Instant::now(),
            0,
            &mut encoder,
            &mut 0,
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
fn empty_outputs_exhaust_the_budget_without_disabling_the_encoder() {
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
