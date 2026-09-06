use std::{collections::VecDeque, error::Error, fmt};

const INTERPOLATION_MARGIN_FRAMES: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AudioResamplerConfig {
    pub input_rate: u32,
    pub output_rate: u32,
    pub channels: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AudioResamplerError {
    InvalidConfig(AudioResamplerConfig),
    InvalidOutputFrameSize { output_frames: usize },
    IncompleteFrame { samples: usize, channels: usize },
    CapacityOverflow,
}

impl fmt::Display for AudioResamplerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(config) => write!(
                formatter,
                "invalid audio resampler configuration: input_rate={}, output_rate={}, channels={}",
                config.input_rate, config.output_rate, config.channels
            ),
            Self::InvalidOutputFrameSize { output_frames } => {
                write!(formatter, "invalid resampler output frame size: {output_frames}")
            }
            Self::IncompleteFrame { samples, channels } => write!(
                formatter,
                "audio resampler input length {samples} is not divisible by channel count {channels}"
            ),
            Self::CapacityOverflow => formatter.write_str("audio resampler output capacity overflow"),
        }
    }
}

impl Error for AudioResamplerError {}

pub(crate) struct FixedFrameAudioResampler {
    resampler: StreamingLinearAudioResampler,
    output_samples: usize,
    pending_samples: VecDeque<f32>,
}

impl FixedFrameAudioResampler {
    pub(crate) fn new(
        config: AudioResamplerConfig,
        output_frames: usize,
    ) -> Result<Self, AudioResamplerError> {
        if output_frames == 0 {
            return Err(AudioResamplerError::InvalidOutputFrameSize { output_frames });
        }
        let output_samples = output_frames
            .checked_mul(config.channels as usize)
            .ok_or(AudioResamplerError::CapacityOverflow)?;
        Ok(Self {
            resampler: StreamingLinearAudioResampler::new(config)?,
            output_samples,
            pending_samples: VecDeque::new(),
        })
    }

    pub(crate) fn process(&mut self, input: &[f32]) -> Result<Vec<Vec<f32>>, AudioResamplerError> {
        self.pending_samples.extend(self.resampler.process(input)?);
        let packet_count = self.pending_samples.len() / self.output_samples;
        let mut packets = Vec::with_capacity(packet_count);
        for _ in 0..packet_count {
            packets.push(self.pending_samples.drain(..self.output_samples).collect());
        }
        Ok(packets)
    }
}

pub(crate) struct AudioResampler {
    backend: StreamingLinearAudioResampler,
}

impl AudioResampler {
    pub(crate) fn new(config: AudioResamplerConfig) -> Result<Self, AudioResamplerError> {
        Ok(Self {
            backend: StreamingLinearAudioResampler::new(config)?,
        })
    }

    pub(crate) fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, AudioResamplerError> {
        self.backend.process(input)
    }
}

struct StreamingLinearAudioResampler {
    config: AudioResamplerConfig,
    channels: usize,
    buffered_samples: Vec<f32>,
    next_position: u64,
}

impl StreamingLinearAudioResampler {
    fn new(config: AudioResamplerConfig) -> Result<Self, AudioResamplerError> {
        Ok(Self {
            config,
            channels: validate_config(config)?,
            buffered_samples: Vec::new(),
            next_position: 0,
        })
    }

    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, AudioResamplerError> {
        validate_input(input, self.channels)?;
        self.buffered_samples.extend_from_slice(input);
        let capacity = self.output_capacity(input.len())?;
        let mut output = Vec::with_capacity(capacity);
        while self.write_next_frame(&mut output) {
            self.next_position += self.config.input_rate as u64;
        }
        self.discard_consumed_frames();
        Ok(output)
    }

    fn output_capacity(&self, input_samples: usize) -> Result<usize, AudioResamplerError> {
        let input_frames = input_samples / self.channels;
        let scaled_frames = input_frames
            .checked_mul(self.config.output_rate as usize)
            .ok_or(AudioResamplerError::CapacityOverflow)?
            / self.config.input_rate as usize;
        scaled_frames
            .checked_add(INTERPOLATION_MARGIN_FRAMES)
            .and_then(|frames| frames.checked_mul(self.channels))
            .ok_or(AudioResamplerError::CapacityOverflow)
    }

    fn write_next_frame(&self, output: &mut Vec<f32>) -> bool {
        let output_rate = self.config.output_rate as u64;
        let frame_count = self.buffered_samples.len() / self.channels;
        let frame = (self.next_position / output_rate) as usize;
        let fraction = self.next_position % output_rate;
        if frame >= frame_count || (fraction != 0 && frame + 1 >= frame_count) {
            return false;
        }
        let weight = fraction as f32 / output_rate as f32;
        for channel in 0..self.channels {
            let current = self.buffered_samples[frame * self.channels + channel];
            let next_frame = frame + usize::from(fraction != 0);
            let next = self.buffered_samples[next_frame * self.channels + channel];
            output.push(current + (next - current) * weight);
        }
        true
    }

    fn discard_consumed_frames(&mut self) {
        let output_rate = self.config.output_rate as u64;
        let available_frames = self.buffered_samples.len() / self.channels;
        let consumed_frames = ((self.next_position / output_rate) as usize).min(available_frames);
        self.buffered_samples
            .drain(0..consumed_frames * self.channels);
        self.next_position -= consumed_frames as u64 * output_rate;
    }
}

fn validate_config(config: AudioResamplerConfig) -> Result<usize, AudioResamplerError> {
    if config.input_rate == 0 || config.output_rate == 0 || config.channels == 0 {
        return Err(AudioResamplerError::InvalidConfig(config));
    }
    Ok(config.channels as usize)
}

fn validate_input(input: &[f32], channels: usize) -> Result<(), AudioResamplerError> {
    if input.len() % channels != 0 {
        return Err(AudioResamplerError::IncompleteFrame {
            samples: input.len(),
            channels,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "audio_resampler_tests.rs"]
mod tests;
