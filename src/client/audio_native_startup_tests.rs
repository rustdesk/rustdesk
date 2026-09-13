use super::*;
use crate::client::{
    audio_playback::{AudioPlaybackConfig, AudioPlaybackWriter},
    AUDIO_HOST,
};
use cpal::traits::{DeviceTrait, HostTrait};
use std::thread;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(10);
const PACKET_SAMPLES: usize = OUTPUT_RATE as usize / PACKETS_PER_SECOND * CHANNELS as usize;

fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + CALLBACK_TIMEOUT;
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "Native callback did not progress"
        );
        thread::sleep(POLL_INTERVAL);
    }
}

fn gated_candidate(
    candidate: &mut AudioHandler,
    gate: Arc<AtomicBool>,
    fail: bool,
) -> ResultType<()> {
    let device = AUDIO_HOST.default_output_device().unwrap();
    let supported = device.default_output_config()?;
    assert_eq!(supported.sample_format(), cpal::SampleFormat::F32);
    let config: cpal::StreamConfig = supported.into();
    assert_eq!(config.sample_rate.0, OUTPUT_RATE);
    assert_eq!(config.channels, CHANNELS);
    candidate.sample_rate = (OUTPUT_RATE, OUTPUT_RATE);
    candidate.device_channel = CHANNELS;
    candidate
        .audio_buffer
        .resize(OUTPUT_RATE as _, CHANNELS as _);
    let mut writer = AudioPlaybackWriter::new(
        AudioPlaybackConfig {
            sample_rate: OUTPUT_RATE,
            channels: CHANNELS as _,
        },
        candidate.audio_buffer.0.clone(),
        candidate.audio_buffer.3.clone(),
    )?;
    candidate.playback_status = writer.status.clone();
    let error_callback = candidate.playback_recovery.new_error_callback();
    let errors = candidate.playback_recovery.errors.clone();
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _| {
            data.fill(0.0);
            if !gate.load(Ordering::Acquire) {
                return;
            }
            // Test-only gate and failure exercise the owner before startup confirmation.
            if fail {
                errors.push(StreamError::DeviceNotAvailable);
                gate.store(false, Ordering::Release);
                panic!("Injected native candidate failure before confirmation");
            }
            writer.write_output(data);
        },
        error_callback,
        None,
    )?;
    stream.play()?;
    candidate.audio_stream = Some(Box::new(stream));
    Ok(())
}

fn assert_consumes_packet(handler: &mut AudioHandler) {
    handler.audio_decoder.as_mut().unwrap().1.fill(f32::NAN);
    handler.handle_frame(audio_frame());
    assert!(handler.audio_decoder.as_ref().unwrap().1[..PACKET_SAMPLES]
        .iter()
        .all(|sample| sample.is_finite()));
    wait_for(|| handler.audio_buffer.0.lock().unwrap().occupied_len() == 0);
    assert!(handler.playback_status.ready.load(Ordering::Acquire));
}

#[test]
#[ignore = "Requires 48 kHz stereo CABLE output; run alone with a 60-second process deadline"]
fn native_compatible_replacement_waits_for_confirmation() {
    assert_eq!(
        AUDIO_HOST.default_output_device().unwrap().name().unwrap(),
        "CABLE Input (VB-Audio Virtual Cable)"
    );
    for fail in [false, true] {
        let mut handler = AudioHandler::default();
        handler.handle_format(format(OUTPUT_RATE, CHANNELS));
        wait_for(|| handler.playback_status.ready.load(Ordering::Acquire));
        let old_buffer = handler.audio_buffer.0.clone();
        let gate = Arc::new(AtomicBool::new(false));
        let mut state = None;
        handler.handle_format_with_start(format(OUTPUT_RATE, CHANNELS), |candidate, _| {
            gated_candidate(candidate, gate.clone(), fail)?;
            state = Some(candidate.playback_status.clone());
            Ok(())
        });
        let status = state.unwrap();
        handler.recover_playback_with(Instant::now(), |_, _| panic!("Unexpected reopen"));
        assert!(Arc::ptr_eq(&old_buffer, &handler.audio_buffer.0));
        assert!(!status.ready.load(Ordering::Acquire));
        assert_consumes_packet(&mut handler);
        gate.store(true, Ordering::Release);
        wait_for(|| {
            if fail {
                !gate.load(Ordering::Acquire)
            } else {
                status.ready.load(Ordering::Acquire)
            }
        });
        handler.recover_playback_with(Instant::now(), |_, _| panic!("Unexpected reopen"));
        assert_eq!(Arc::ptr_eq(&old_buffer, &handler.audio_buffer.0), fail);
        assert_consumes_packet(&mut handler);
    }
}
