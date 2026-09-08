use hbb_common::{log, thiserror};
use ringbuf::{ring_buffer::RbBase, Rb};
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) const UNDERRUN_DECLICK_MS: usize = 5;
const MILLISECONDS_PER_SECOND: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AudioPlaybackConfig {
    pub sample_rate: u32,
    pub channels: usize,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(super) enum AudioPlaybackError {
    #[error(
        "invalid audio playback configuration: sample_rate={}, channels={}",
        .0.sample_rate, .0.channels
    )]
    InvalidConfig(AudioPlaybackConfig),
    #[error("audio playback frame has {samples} samples for {channels} channels")]
    IncompleteFrame { samples: usize, channels: usize },
    #[error("audio playback transition frame count overflow")]
    FrameCountOverflow,
}

pub(super) struct AudioPlaybackRecovery {
    channels: usize,
    transition_frames: usize,
    transition_frame: usize,
    had_input: bool,
    transition_start: Vec<f32>,
    output_frame: Vec<f32>,
}

pub(super) struct AudioPlaybackWriter {
    audio_buffer: std::sync::Arc<std::sync::Mutex<ringbuf::HeapRb<f32>>>,
    discontinuity_generation: std::sync::Arc<AtomicUsize>,
    observed_discontinuity_generation: usize,
    buffered_input: Vec<f32>,
    recovery: AudioPlaybackRecovery,
}

impl AudioPlaybackWriter {
    pub(super) fn new(
        config: AudioPlaybackConfig,
        audio_buffer: std::sync::Arc<std::sync::Mutex<ringbuf::HeapRb<f32>>>,
        discontinuity_generation: std::sync::Arc<AtomicUsize>,
    ) -> Result<Self, AudioPlaybackError> {
        let recovery = AudioPlaybackRecovery::new(config)?;
        let buffer_capacity = audio_buffer.lock().unwrap().capacity();
        let observed_discontinuity_generation = discontinuity_generation.load(Ordering::Relaxed);
        Ok(Self {
            audio_buffer,
            discontinuity_generation,
            observed_discontinuity_generation,
            buffered_input: vec![0.0; buffer_capacity],
            recovery,
        })
    }

    pub(super) fn write_output<T>(&mut self, output: &mut [T])
    where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        let requested_samples = output.len().min(self.buffered_input.len());
        let channel_count = self.recovery.channels;
        let (available_samples, generation) = {
            let mut buffer = self.audio_buffer.lock().unwrap();
            let generation = self.discontinuity_generation.load(Ordering::Relaxed);
            let samples =
                buffer.occupied_len().min(requested_samples) / channel_count * channel_count;
            buffer.pop_slice(&mut self.buffered_input[..samples]);
            (samples, generation)
        };
        if generation != self.observed_discontinuity_generation {
            self.recovery.begin_discontinuity();
            self.observed_discontinuity_generation = generation;
        }
        let available_frames = available_samples / channel_count;
        for (frame_index, output_frame) in output.chunks_mut(channel_count).enumerate() {
            let input = if frame_index < available_frames {
                let start = frame_index * channel_count;
                Some(&self.buffered_input[start..start + channel_count])
            } else {
                None
            };
            match self.recovery.process_frame(input) {
                Ok(recovered) => {
                    for (output, sample) in output_frame.iter_mut().zip(recovered) {
                        *output = T::from_sample(*sample);
                    }
                }
                Err(error) => {
                    log::error!("Failed to recover audio underflow: {error}");
                    output_frame.fill(T::from_sample(0.0));
                }
            }
        }
    }
}

impl AudioPlaybackRecovery {
    pub(super) fn new(config: AudioPlaybackConfig) -> Result<Self, AudioPlaybackError> {
        if config.sample_rate == 0 || config.channels == 0 {
            return Err(AudioPlaybackError::InvalidConfig(config));
        }
        let transition_frames = (config.sample_rate as usize)
            .checked_mul(UNDERRUN_DECLICK_MS)
            .ok_or(AudioPlaybackError::FrameCountOverflow)?
            / MILLISECONDS_PER_SECOND;
        if transition_frames == 0 {
            return Err(AudioPlaybackError::InvalidConfig(config));
        }
        Ok(Self {
            channels: config.channels,
            transition_frames,
            transition_frame: transition_frames,
            had_input: false,
            transition_start: vec![0.0; config.channels],
            output_frame: vec![0.0; config.channels],
        })
    }

    pub(super) fn process_frame(
        &mut self,
        input: Option<&[f32]>,
    ) -> Result<&[f32], AudioPlaybackError> {
        if input.is_some_and(|frame| frame.len() != self.channels) {
            return Err(AudioPlaybackError::IncompleteFrame {
                samples: input.map_or(0, <[f32]>::len),
                channels: self.channels,
            });
        }
        self.begin_transition(input.is_some());
        let target_weight = self.advance_transition();
        for channel in 0..self.channels {
            let target = input.map_or(0.0, |frame| frame[channel]);
            self.output_frame[channel] =
                self.transition_start[channel] * (1.0 - target_weight) + target * target_weight;
        }
        Ok(&self.output_frame)
    }

    pub(super) fn begin_discontinuity(&mut self) {
        self.transition_start.copy_from_slice(&self.output_frame);
        self.transition_frame = 0;
    }

    fn begin_transition(&mut self, has_input: bool) {
        if has_input == self.had_input {
            return;
        }
        self.transition_start.copy_from_slice(&self.output_frame);
        self.transition_frame = 0;
        self.had_input = has_input;
    }

    fn advance_transition(&mut self) -> f32 {
        if self.transition_frame >= self.transition_frames {
            return 1.0;
        }
        self.transition_frame += 1;
        self.transition_frame as f32 / self.transition_frames as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioPlaybackConfig, AudioPlaybackError, AudioPlaybackRecovery, AudioPlaybackWriter,
    };
    use ringbuf::{ring_buffer::RbBase, Rb};
    use std::sync::{atomic::Ordering, Arc, Mutex};

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
        let mut writer =
            AudioPlaybackWriter::new(config, buffer.clone(), generation.clone()).unwrap();
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
}
