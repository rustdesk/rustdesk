use hbb_common::{log, thiserror};
use ringbuf::{ring_buffer::RbBase, Rb};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    TryLockError,
};

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

#[derive(Default)]
pub(super) struct AudioPlaybackStatus {
    pub(super) ready: AtomicBool,
    contentions: AtomicUsize,
    buffer_poisoned: AtomicBool,
}

impl AudioPlaybackStatus {
    pub(super) fn report_errors(&self) {
        let contentions = self.contentions.swap(0, Ordering::Relaxed);
        if contentions != 0 {
            log::debug!("Audio playback PCM buffer contention: callbacks={contentions}");
        }
        if self.buffer_poisoned.swap(false, Ordering::Relaxed) {
            log::error!("Audio playback stopped reading a poisoned PCM buffer");
        }
    }
}

pub(super) struct AudioPlaybackWriter {
    audio_buffer: std::sync::Arc<std::sync::Mutex<ringbuf::HeapRb<f32>>>,
    discontinuity_generation: std::sync::Arc<AtomicUsize>,
    observed_discontinuity_generation: usize,
    buffered_input: Vec<f32>,
    recovery: AudioPlaybackRecovery,
    pub(super) status: std::sync::Arc<AudioPlaybackStatus>,
    buffer_failed: bool,
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
            status: Default::default(),
            buffer_failed: false,
        })
    }

    fn read_buffer(&mut self, requested_samples: usize) -> usize {
        if self.buffer_failed {
            return 0;
        }
        let mut buffer = match self.audio_buffer.try_lock() {
            Ok(buffer) => buffer,
            Err(TryLockError::WouldBlock) => {
                // Keep queued PCM and its generation for the next successful read.
                self.status.contentions.fetch_add(1, Ordering::Relaxed);
                return 0;
            }
            Err(TryLockError::Poisoned(_)) => {
                self.buffer_failed = true;
                self.status.ready.store(false, Ordering::Release);
                self.status.buffer_poisoned.store(true, Ordering::Relaxed);
                return 0;
            }
        };
        let generation = self.discontinuity_generation.load(Ordering::Relaxed);
        let channels = self.recovery.channels;
        let samples = buffer.occupied_len().min(requested_samples) / channels * channels;
        buffer.pop_slice(&mut self.buffered_input[..samples]);
        drop(buffer);
        if generation != self.observed_discontinuity_generation {
            self.recovery.begin_discontinuity();
            self.observed_discontinuity_generation = generation;
        }
        samples
    }

    pub(super) fn write_output<T>(&mut self, output: &mut [T])
    where
        T: cpal::Sample + cpal::FromSample<f32>,
    {
        self.status
            .ready
            .store(!self.buffer_failed, Ordering::Release);
        let requested_samples = output.len().min(self.buffered_input.len());
        let channel_count = self.recovery.channels;
        let available_samples = self.read_buffer(requested_samples);
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
#[path = "audio_playback_tests.rs"]
mod tests;
