use hbb_common::thiserror;

#[cfg(test)]
pub(crate) mod allocation_tests;

#[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
mod sinc;

const INTERPOLATION_MARGIN_FRAMES: usize = 2;
const PENDING_PACKET_CAPACITY: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AudioResamplerConfig {
    pub input_rate: u32,
    pub output_rate: u32,
    pub channels: u16,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum AudioResamplerError {
    #[error(
        "invalid audio resampler configuration: input_rate={}, output_rate={}, channels={}",
        .0.input_rate, .0.output_rate, .0.channels
    )]
    InvalidConfig(AudioResamplerConfig),
    #[error("invalid resampler output frame size: {output_frames}")]
    InvalidOutputFrameSize { output_frames: usize },
    #[error("audio resampler input length {samples} is not divisible by channel count {channels}")]
    IncompleteFrame { samples: usize, channels: usize },
    #[error("audio resampler output capacity overflow")]
    CapacityOverflow,
    #[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
    #[error("audio resampler backend failed: {0}")]
    Backend(String),
}

pub(crate) struct FixedFrameAudioResampler {
    resampler: AudioResampler,
    output_samples: usize,
    pending_samples: Vec<f32>,
}

#[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
// SAFETY: libsamplerate's src_new state owns heap data and has no thread affinity.
// This wrapper never exposes or shares that state; processing requires &mut self.
unsafe impl Send for FixedFrameAudioResampler {}

impl FixedFrameAudioResampler {
    pub(crate) fn new(
        config: AudioResamplerConfig,
        output_frames: usize,
    ) -> Result<Self, AudioResamplerError> {
        if output_frames == 0 {
            return Err(AudioResamplerError::InvalidOutputFrameSize { output_frames });
        }
        let channels = validate_config(config)?;
        let output_samples = output_frames
            .checked_mul(channels)
            .ok_or(AudioResamplerError::CapacityOverflow)?;
        let input_frames = output_frames
            .checked_mul(config.input_rate as usize)
            .ok_or(AudioResamplerError::CapacityOverflow)?
            .div_ceil(config.output_rate as usize);
        let capacity = output_samples
            .checked_mul(PENDING_PACKET_CAPACITY)
            .and_then(|samples| samples.checked_add(channels * INTERPOLATION_MARGIN_FRAMES))
            .ok_or(AudioResamplerError::CapacityOverflow)?;
        let mut resampler = AudioResampler::new(config)?;
        resampler.reserve_input(input_frames)?;
        Ok(Self {
            resampler,
            output_samples,
            pending_samples: Vec::with_capacity(capacity),
        })
    }

    pub(crate) fn process_with(
        &mut self,
        input: &[f32],
        mut on_packet: impl FnMut(&[f32]),
    ) -> Result<(), AudioResamplerError> {
        self.resampler
            .process_into(input, &mut self.pending_samples)?;
        let complete_samples =
            self.pending_samples.len() / self.output_samples * self.output_samples;
        for packet in self.pending_samples[..complete_samples].chunks_exact(self.output_samples) {
            on_packet(packet);
        }
        self.pending_samples.drain(..complete_samples);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn process(&mut self, input: &[f32]) -> Result<Vec<Vec<f32>>, AudioResamplerError> {
        let mut packets = Vec::new();
        self.process_with(input, |packet| packets.push(packet.to_owned()))?;
        Ok(packets)
    }
}

pub(crate) struct AudioResampler {
    #[cfg(not(all(feature = "use_samplerate", not(feature = "use_dasp"))))]
    backend: StreamingLinearAudioResampler,
    #[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
    backend: sinc::SincAudioResampler,
}

impl AudioResampler {
    pub(crate) fn new(config: AudioResamplerConfig) -> Result<Self, AudioResamplerError> {
        Ok(Self {
            #[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
            backend: sinc::SincAudioResampler::new(config)?,
            #[cfg(not(all(feature = "use_samplerate", not(feature = "use_dasp"))))]
            backend: StreamingLinearAudioResampler::new(config)?,
        })
    }

    pub(crate) fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, AudioResamplerError> {
        let mut output = Vec::new();
        self.process_into(input, &mut output)?;
        Ok(output)
    }

    // Append samples so capture can retain an incomplete output packet in the same buffer.
    fn process_into(
        &mut self,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), AudioResamplerError> {
        self.backend.process_into(input, output)
    }

    fn reserve_input(&mut self, _frames: usize) -> Result<(), AudioResamplerError> {
        #[cfg(not(all(feature = "use_samplerate", not(feature = "use_dasp"))))]
        {
            let capacity = _frames
                .checked_add(INTERPOLATION_MARGIN_FRAMES)
                .and_then(|frames| frames.checked_mul(self.backend.channels))
                .ok_or(AudioResamplerError::CapacityOverflow)?;
            self.backend.buffered_samples.reserve(capacity);
        }
        Ok(())
    }
}

#[cfg(not(all(feature = "use_samplerate", not(feature = "use_dasp"))))]
struct StreamingLinearAudioResampler {
    config: AudioResamplerConfig,
    channels: usize,
    buffered_samples: Vec<f32>,
    next_position: u64,
}

#[cfg(not(all(feature = "use_samplerate", not(feature = "use_dasp"))))]
impl StreamingLinearAudioResampler {
    fn new(config: AudioResamplerConfig) -> Result<Self, AudioResamplerError> {
        Ok(Self {
            config,
            channels: validate_config(config)?,
            buffered_samples: Vec::new(),
            next_position: 0,
        })
    }

    fn process_into(
        &mut self,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), AudioResamplerError> {
        validate_input(input, self.channels)?;
        let capacity = self.output_capacity(input.len())?;
        output.reserve(capacity);
        self.buffered_samples.extend_from_slice(input);
        while self.write_next_frame(output) {
            self.next_position += self.config.input_rate as u64;
        }
        self.discard_consumed_frames();
        Ok(())
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

#[cfg(all(test, not(all(feature = "use_samplerate", not(feature = "use_dasp")))))]
mod tests;

#[cfg(all(test, feature = "use_samplerate", not(feature = "use_dasp")))]
mod samplerate_tests;
