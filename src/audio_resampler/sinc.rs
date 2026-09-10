use super::{AudioResamplerConfig, AudioResamplerError};
use libsamplerate_sys as sys;
use std::ptr::NonNull;

const OUTPUT_MARGIN_FRAMES: usize = 1;

pub(super) struct SincAudioResampler {
    state: NonNull<sys::SRC_STATE>,
    config: AudioResamplerConfig,
}

impl SincAudioResampler {
    pub(super) fn new(config: AudioResamplerConfig) -> Result<Self, AudioResamplerError> {
        super::validate_config(config)?;
        let ratio = f64::from(config.output_rate) / f64::from(config.input_rate);
        if unsafe { sys::src_is_valid_ratio(ratio) } == 0 {
            return Err(backend_error(
                config,
                samplerate::ErrorCode::BadSrcRatio as _,
            ));
        }
        let mut error = 0;
        // SAFETY: src_new allocates independent state; this owner releases it in Drop.
        let state = unsafe {
            sys::src_new(
                sys::SRC_SINC_BEST_QUALITY as _,
                config.channels.into(),
                &mut error,
            )
        };
        let state = NonNull::new(state).ok_or_else(|| backend_error(config, error))?;
        Ok(Self { state, config })
    }

    pub(super) fn process_into(
        &mut self,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(), AudioResamplerError> {
        super::validate_input(input, self.config.channels as usize)?;
        let mut consumed = 0;
        loop {
            let (used, generated) = self.process_block(&input[consumed..], output)?;
            consumed += used;
            if consumed == input.len() {
                return Ok(());
            }
            if used == 0 && generated == 0 {
                return Err(AudioResamplerError::Backend(
                    "libsamplerate made no progress while input remained".to_owned(),
                ));
            }
        }
    }

    fn process_block(
        &mut self,
        input: &[f32],
        output: &mut Vec<f32>,
    ) -> Result<(usize, usize), AudioResamplerError> {
        let channels = self.config.channels as usize;
        let input_frames = input.len() / channels;
        let output_frames = input_frames
            .checked_mul(self.config.output_rate as usize)
            .map(|frames| frames / self.config.input_rate as usize)
            .and_then(|frames| frames.checked_add(OUTPUT_MARGIN_FRAMES))
            .ok_or(AudioResamplerError::CapacityOverflow)?;
        let start = output.len();
        let end = output_frames
            .checked_mul(channels)
            .and_then(|samples| start.checked_add(samples))
            .ok_or(AudioResamplerError::CapacityOverflow)?;
        let mut data = sys::SRC_DATA {
            data_in: input.as_ptr(),
            input_frames: input_frames
                .try_into()
                .map_err(|_| AudioResamplerError::CapacityOverflow)?,
            output_frames: output_frames
                .try_into()
                .map_err(|_| AudioResamplerError::CapacityOverflow)?,
            src_ratio: f64::from(self.config.output_rate) / f64::from(self.config.input_rate),
            ..Default::default()
        };
        output.resize(end, 0.0);
        data.data_out = output[start..].as_mut_ptr();
        // SAFETY: state is exclusively owned; disjoint slices cover the declared frame counts.
        let error = unsafe { sys::src_process(self.state.as_ptr(), &mut data) };
        let generated = data.output_frames_gen as usize * channels;
        output.truncate(start + generated);
        if error != 0 {
            return Err(backend_error(self.config, error));
        }
        Ok((data.input_frames_used as usize * channels, generated))
    }
}

impl Drop for SincAudioResampler {
    fn drop(&mut self) {
        // SAFETY: this owner holds the only handle returned by src_new.
        unsafe { sys::src_delete(self.state.as_ptr()) };
    }
}

fn backend_error(config: AudioResamplerConfig, code: i32) -> AudioResamplerError {
    AudioResamplerError::Backend(format!(
        "input_rate={}, output_rate={}, channels={}: {:?}",
        config.input_rate,
        config.output_rate,
        config.channels,
        samplerate::Error::from_int(code)
    ))
}
