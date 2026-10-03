use super::*;
use scrap::{codec::EncoderApi, EncodeYuvFormat};

struct RejectOnceEncoder(bool);

impl EncoderApi for RejectOnceEncoder {
    fn new(_: EncoderCfg, _: bool) -> ResultType<Self> {
        unreachable!()
    }

    fn encode_to_message(&mut self, _: EncodeInput, _: i64) -> ResultType<VideoFrame> {
        unreachable!()
    }

    fn yuvfmt(&self) -> EncodeYuvFormat {
        unreachable!()
    }

    #[cfg(feature = "vram")]
    fn input_texture(&self) -> bool {
        false
    }

    fn set_quality(&mut self, _: f32) -> ResultType<()> {
        if std::mem::take(&mut self.0) {
            bail!("bitrate update rejected");
        }
        Ok(())
    }

    fn bitrate(&self) -> u32 {
        2000
    }

    fn support_changing_quality(&self) -> bool {
        true
    }

    fn latency_free(&self) -> bool {
        true
    }

    fn is_hardware(&self) -> bool {
        true
    }

    fn disable(&self) {}
}

#[test]
fn failed_quality_update_preserves_feedback_and_retries_same_target() {
    struct RestoreQos(VideoQoS);
    impl Drop for RestoreQos {
        fn drop(&mut self) {
            *VIDEO_QOS.lock().unwrap_or_else(|p| p.into_inner()) = std::mem::take(&mut self.0);
        }
    }
    let mut qos = VideoQoS::default();
    qos.store_bitrate(1000);
    let requested_ratio = qos.ratio();
    let _restore = RestoreQos(std::mem::replace(&mut *VIDEO_QOS.lock().unwrap(), qos));
    let mut ratio = requested_ratio * 2.;
    let mut encoder = Encoder {
        codec: Box::new(RejectOnceEncoder(true)),
    };
    let mut spf = Duration::ZERO;
    for expected in [(requested_ratio * 2., 1000), (requested_ratio, 2000)] {
        check_qos(
            &mut encoder,
            &mut ratio,
            &mut spf,
            false,
            &mut 0,
            &mut Instant::now(),
            "",
        )
        .unwrap();
        let bitrate = VIDEO_QOS.lock().unwrap().bitrate();
        assert_eq!((ratio, bitrate), expected);
    }
}
