use std::{error::Error, fmt};

use hbb_common::log;
use ringbuf::Rb;

pub(super) const UNDERRUN_DECLICK_MS: usize = 5;
const MILLISECONDS_PER_SECOND: usize = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct AudioPlaybackConfig {
    pub sample_rate: u32,
    pub channels: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AudioPlaybackError {
    InvalidConfig(AudioPlaybackConfig),
    IncompleteFrame { samples: usize, channels: usize },
    FrameCountOverflow,
}

impl fmt::Display for AudioPlaybackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(config) => write!(
                formatter,
                "invalid audio playback configuration: sample_rate={}, channels={}",
                config.sample_rate, config.channels
            ),
            Self::IncompleteFrame { samples, channels } => write!(
                formatter,
                "audio playback frame has {samples} samples for {channels} channels"
            ),
            Self::FrameCountOverflow => {
                formatter.write_str("audio playback transition frame count overflow")
            }
        }
    }
}

impl Error for AudioPlaybackError {}

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
    channels: std::num::NonZeroUsize,
    buffered_input: Vec<f32>,
    recovery: AudioPlaybackRecovery,
}

impl AudioPlaybackWriter {
    pub(super) fn new(
        config: AudioPlaybackConfig,
        audio_buffer: std::sync::Arc<std::sync::Mutex<ringbuf::HeapRb<f32>>>,
    ) -> Result<Self, AudioPlaybackError> {
        let channels = std::num::NonZeroUsize::new(config.channels)
            .ok_or(AudioPlaybackError::InvalidConfig(config))?;
        let buffer_capacity = audio_buffer.lock().unwrap().capacity();
        Ok(Self {
            audio_buffer,
            channels,
            buffered_input: vec![0.0; buffer_capacity],
            recovery: AudioPlaybackRecovery::new(config)?,
        })
    }

    pub(super) fn write_output<T>(&mut self, output: &mut [T])
    where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        let requested_samples = output.len().min(self.buffered_input.len());
        let available_samples = super::audio_buffer::drain_audio_samples(
            &self.audio_buffer,
            &mut self.buffered_input[..requested_samples],
            self.channels,
        );
        let channel_count = self.channels.get();
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
    use super::{AudioPlaybackConfig, AudioPlaybackError, AudioPlaybackRecovery};

    const SAMPLE_RATE: u32 = 48_000;
    const CHANNELS: usize = 2;
    const ACTIVE_FRAME: [f32; CHANNELS] = [0.8, -0.8];
    const ACTIVE_FRAMES: usize = 300;
    const SILENT_FRAMES: usize = 300;
    const MAX_SAMPLE_STEP: f32 = 0.01;

    fn maximum_sample_step(samples: &[f32]) -> f32 {
        samples
            .chunks_exact(CHANNELS)
            .collect::<Vec<_>>()
            .windows(2)
            .fold(0.0, |maximum, frames| {
                (0..CHANNELS).fold(maximum, |maximum, channel| {
                    maximum.max((frames[1][channel] - frames[0][channel]).abs())
                })
            })
    }

    #[test]
    fn smooths_forced_underflow_edges() {
        let config = AudioPlaybackConfig {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
        };
        let mut recovery = AudioPlaybackRecovery::new(config).unwrap();
        let mut output = Vec::new();
        for _ in 0..ACTIVE_FRAMES {
            output.extend_from_slice(recovery.process_frame(Some(&ACTIVE_FRAME)).unwrap());
        }
        for _ in 0..SILENT_FRAMES {
            output.extend_from_slice(recovery.process_frame(None).unwrap());
        }
        for _ in 0..ACTIVE_FRAMES {
            output.extend_from_slice(recovery.process_frame(Some(&ACTIVE_FRAME)).unwrap());
        }
        let maximum = maximum_sample_step(&output);

        assert!(
            maximum <= MAX_SAMPLE_STEP,
            "underflow transition step {maximum} exceeded {MAX_SAMPLE_STEP}"
        );
    }

    #[test]
    fn preserves_input_after_fade_in() {
        let config = AudioPlaybackConfig {
            sample_rate: SAMPLE_RATE,
            channels: CHANNELS,
        };
        let mut recovery = AudioPlaybackRecovery::new(config).unwrap();
        let transition_frames = SAMPLE_RATE as usize * super::UNDERRUN_DECLICK_MS / 1_000;
        let mut last_frame = [0.0; CHANNELS];

        for _ in 0..transition_frames {
            last_frame.copy_from_slice(recovery.process_frame(Some(&ACTIVE_FRAME)).unwrap());
        }

        assert_eq!(last_frame, ACTIVE_FRAME);
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
