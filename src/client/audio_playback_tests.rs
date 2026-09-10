use super::{AudioPlaybackConfig, AudioPlaybackError, AudioPlaybackRecovery, AudioPlaybackWriter};
use ringbuf::{ring_buffer::RbBase, Rb};
use std::{
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    time::Duration,
};

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;
const ACTIVE_FRAME: [f32; CHANNELS] = [0.8, -0.8];
const OPPOSITE_ACTIVE_FRAME: [f32; CHANNELS] = [-0.8, 0.8];
const ACTIVE_FRAMES: usize = 300;
const SILENT_FRAMES: usize = 300;
const TRANSITION_FRAMES: usize =
    SAMPLE_RATE as usize * super::UNDERRUN_DECLICK_MS / super::MILLISECONDS_PER_SECOND;
const MAX_SAMPLE_STEP: f32 = 0.01;

#[test]
fn writing_audio_observes_discard_and_releases_buffer_lock() {
    const INPUT: [f32; 4] = [0.1, 0.2, 0.3, 0.4];
    const GENERATION: usize = 7;
    let buffer = Arc::new(Mutex::new(ringbuf::HeapRb::new(INPUT.len())));
    let generation = Arc::new(super::AtomicUsize::new(0));
    let config = AudioPlaybackConfig {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    let mut writer = AudioPlaybackWriter::new(config, buffer.clone(), generation.clone()).unwrap();
    {
        let mut buffer = buffer.lock().unwrap();
        buffer.push_slice(&INPUT);
        generation.store(GENERATION, Ordering::Relaxed);
    }
    let mut output = [0.0_f32; INPUT.len()];

    writer.write_output(&mut output);

    assert_eq!(writer.buffered_input, INPUT);
    assert_eq!(writer.observed_discontinuity_generation, GENERATION);
    assert_eq!(buffer.try_lock().unwrap().occupied_len(), 0);
}

fn maximum_sample_step(samples: &[f32]) -> f32 {
    samples
        .windows(CHANNELS + 1)
        .map(|window| (window[CHANNELS] - window[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn smooths_underflow_and_explicit_audio_discontinuities() {
    for explicit_discontinuity in [false, true] {
        let config = AudioPlaybackConfig {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
        };
        let mut recovery = AudioPlaybackRecovery::new(config).unwrap();
        let mut output = Vec::new();
        for _ in 0..ACTIVE_FRAMES {
            output.extend_from_slice(recovery.process_frame(Some(&ACTIVE_FRAME)).unwrap());
        }
        let transition_end = TRANSITION_FRAMES * CHANNELS;
        assert_eq!(
            &output[transition_end - CHANNELS..transition_end],
            ACTIVE_FRAME.as_slice(),
            "explicit_discontinuity={explicit_discontinuity}"
        );
        let resumed_frame = if explicit_discontinuity {
            recovery.begin_discontinuity();
            &OPPOSITE_ACTIVE_FRAME
        } else {
            for _ in 0..SILENT_FRAMES {
                output.extend_from_slice(recovery.process_frame(None).unwrap());
            }
            &ACTIVE_FRAME
        };
        for _ in 0..ACTIVE_FRAMES {
            output.extend_from_slice(recovery.process_frame(Some(resumed_frame)).unwrap());
        }
        let maximum = maximum_sample_step(&output);
        assert!(
            maximum <= MAX_SAMPLE_STEP,
            "step {maximum} exceeded {MAX_SAMPLE_STEP}, explicit={explicit_discontinuity}"
        );
        assert_eq!(
            &output[output.len() - CHANNELS..],
            resumed_frame,
            "explicit_discontinuity={explicit_discontinuity}"
        );
    }
}

#[test]
fn validates_configuration_and_frame_size() {
    let invalid_config = AudioPlaybackConfig {
        sample_rate: 0,
        channels: CHANNELS,
    };
    assert_eq!(
        AudioPlaybackRecovery::new(invalid_config).err(),
        Some(AudioPlaybackError::InvalidConfig(invalid_config))
    );

    let config = AudioPlaybackConfig {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    let mut recovery = AudioPlaybackRecovery::new(config).unwrap();
    assert_eq!(
        recovery.process_frame(Some(&[0.5])).err(),
        Some(AudioPlaybackError::IncompleteFrame {
            samples: 1,
            channels: CHANNELS,
        })
    );
}

const CALLBACK_SAMPLES: usize = 64;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(2);
const DISCARD_GENERATION: usize = 1;

fn write_while_buffer_is_locked(
    mut writer: AudioPlaybackWriter,
    buffer: &Arc<Mutex<ringbuf::HeapRb<f32>>>,
    generation: &Arc<super::AtomicUsize>,
) -> (AudioPlaybackWriter, [f32; CALLBACK_SAMPLES]) {
    let mut guard = buffer.lock().unwrap();
    let queued = OPPOSITE_ACTIVE_FRAME.repeat(ACTIVE_FRAMES);
    guard.push_slice(&queued);
    generation.store(DISCARD_GENERATION, Ordering::Relaxed);
    let (completed_tx, completed_rx) = mpsc::channel();
    let callback = std::thread::spawn(move || {
        let mut output = [0.0; CALLBACK_SAMPLES];
        crate::audio_resampler::allocation_tests::assert_no_allocations(|| {
            writer.write_output(&mut output);
        });
        completed_tx.send((writer, output)).unwrap();
    });
    let completed = completed_rx.recv_timeout(CALLBACK_TIMEOUT);
    let retained = guard.occupied_len();
    drop(guard);
    callback.join().unwrap();
    let result = completed.expect("playback callback waited for the buffer owner");
    assert_eq!(retained, queued.len());
    result
}

#[test]
fn playback_contention_preserves_queued_audio_and_recovers_after_release() {
    let samples = ACTIVE_FRAMES * CHANNELS;
    let buffer = Arc::new(Mutex::new(ringbuf::HeapRb::new(samples)));
    let generation = Arc::new(super::AtomicUsize::new(0));
    let config = AudioPlaybackConfig {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    let mut writer = AudioPlaybackWriter::new(config, buffer.clone(), generation.clone()).unwrap();
    buffer
        .lock()
        .unwrap()
        .push_slice(&ACTIVE_FRAME.repeat(ACTIVE_FRAMES));
    let mut output = vec![0.0; samples];
    writer.write_output(&mut output);
    assert_eq!(&output[samples - CHANNELS..], &ACTIVE_FRAME);

    let (mut writer, gap) = write_while_buffer_is_locked(writer, &buffer, &generation);

    assert_eq!(writer.status.contentions.load(Ordering::Relaxed), 1);
    assert!(writer.status.ready.load(Ordering::Acquire));
    assert_eq!(writer.observed_discontinuity_generation, 0);
    assert!(maximum_sample_step(&gap) <= MAX_SAMPLE_STEP);
    assert!(gap[0] > 0.0 && gap[0] < ACTIVE_FRAME[0]);
    assert_eq!(gap[1], -gap[0]);
    assert!(gap[CALLBACK_SAMPLES - CHANNELS] > 0.0);
    writer.write_output(&mut output);
    let mut transition = gap[gap.len() - CHANNELS..].to_vec();
    transition.extend_from_slice(&output[..TRANSITION_FRAMES * CHANNELS]);
    assert!(maximum_sample_step(&transition) <= MAX_SAMPLE_STEP);
    assert_eq!(
        writer.buffered_input,
        OPPOSITE_ACTIVE_FRAME.repeat(ACTIVE_FRAMES)
    );
    assert_eq!(writer.observed_discontinuity_generation, DISCARD_GENERATION);
    assert_eq!(&output[samples - CHANNELS..], &OPPOSITE_ACTIVE_FRAME);
    assert_eq!(buffer.lock().unwrap().occupied_len(), 0);
}

#[test]
fn poisoned_playback_buffer_reports_once_without_panicking_in_the_callback() {
    let buffer = Arc::new(Mutex::new(ringbuf::HeapRb::new(CALLBACK_SAMPLES)));
    let config = AudioPlaybackConfig {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
    };
    let mut writer =
        AudioPlaybackWriter::new(config, buffer.clone(), Arc::new(super::AtomicUsize::new(0)))
            .unwrap();
    assert!(std::thread::spawn(move || {
        let _guard = buffer.lock().unwrap();
        panic!("Injected PCM buffer failure");
    })
    .join()
    .is_err());
    let mut output = [ACTIVE_FRAME[0]; CALLBACK_SAMPLES];

    crate::audio_resampler::allocation_tests::assert_no_allocations(|| {
        writer.write_output(&mut output);
    });

    assert_eq!(output, [0.0; CALLBACK_SAMPLES]);
    assert!(!writer.status.ready.load(Ordering::Acquire));
    assert!(writer.status.buffer_poisoned.load(Ordering::Relaxed));
    writer.status.report_errors();
    writer.write_output(&mut output);
    assert!(!writer.status.buffer_poisoned.load(Ordering::Relaxed));
    assert_eq!(writer.status.contentions.load(Ordering::Relaxed), 0);
    assert!(!writer.status.ready.load(Ordering::Acquire));
}
