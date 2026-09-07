use hbb_common::anyhow::{bail, Result};

const STEREO_CHANNELS: usize = 2;

pub(super) struct CaptureFrameBuffer {
    samples: Vec<f32>,
    filled: usize,
}

impl CaptureFrameBuffer {
    pub(super) fn new(samples: usize) -> Result<Self> {
        if samples == 0 {
            bail!("Audio capture frame must contain at least one sample");
        }
        Ok(Self {
            samples: vec![0.0; samples],
            filled: 0,
        })
    }

    pub(super) fn process(
        &mut self,
        input: impl Iterator<Item = f32>,
        mut on_frame: impl FnMut(&[f32]),
    ) {
        for sample in input {
            self.samples[self.filled] = sample;
            self.filled += 1;
            if self.filled == self.samples.len() {
                self.filled = 0;
                on_frame(&self.samples);
            }
        }
    }
}

pub(super) fn rechannel<'a>(
    input: &'a [f32],
    channels: u16,
    output: &'a mut Vec<f32>,
) -> &'a [f32] {
    let input = if channels > STEREO_CHANNELS as u16 {
        &input[..input.len() / channels as usize * channels as usize]
    } else {
        input
    };
    output.clear();
    match channels {
        3 => rechannel_frame::<3>(input, output),
        4 => rechannel_frame::<4>(input, output),
        5 => rechannel_frame::<5>(input, output),
        6 => rechannel_frame::<6>(input, output),
        7 => rechannel_frame::<7>(input, output),
        8 => rechannel_frame::<8>(input, output),
        // Preserve the existing passthrough for mono/stereo and unsupported layouts.
        _ => return input,
    }
    output
}

fn rechannel_frame<const CHANNELS: usize>(input: &[f32], output: &mut Vec<f32>) {
    use fon::{
        chan::{Ch32, Channel},
        Frame,
    };

    for samples in input.chunks_exact(CHANNELS) {
        let mut frame = Frame::<Ch32, CHANNELS>::default();
        for (channel, sample) in frame.channels_mut().iter_mut().zip(samples) {
            *channel = (*sample).into();
        }
        // Match the same-rate Stream::pipe conversion before fon's SinkTo conversion.
        let stereo = frame.to::<Ch32, CHANNELS>().to::<Ch32, STEREO_CHANNELS>();
        output.extend(stereo.channels().iter().map(|channel| channel.to_f32()));
    }
}

#[cfg(test)]
mod tests {
    use super::{rechannel, CaptureFrameBuffer};
    use crate::audio_resampler::allocation_tests::assert_no_allocations;

    #[test]
    fn capture_channel_conversion_reuses_storage_and_preserves_mapping() {
        const SAMPLE_RATE: u32 = 48_000;
        const FRAMES: usize = 7;
        const STEREO_CHANNELS: u16 = 2;
        const MIN_SAMPLE: f32 = -1.25;
        const SAMPLE_STEP: f32 = 0.2;

        for channels in [1, 2, 3, 4, 5, 6, 7, 8, 16] {
            let input: Vec<_> = (0..FRAMES * channels as usize + 1)
                .map(|sample| MIN_SAMPLE + sample as f32 * SAMPLE_STEP)
                .collect();
            let expected = crate::common::audio_rechannel(
                input.clone(),
                SAMPLE_RATE,
                SAMPLE_RATE,
                channels,
                channels.min(STEREO_CHANNELS),
            );
            let mut output = Vec::with_capacity(FRAMES * STEREO_CHANNELS as usize);
            assert_no_allocations(|| {
                let actual = rechannel(&input, channels, &mut output);
                assert_eq!(
                    actual, expected,
                    "channel mapping changed for {channels} channels"
                );
            });
        }
    }

    #[test]
    fn capture_framing_retains_partial_input_without_allocating() {
        const FRAME_SAMPLES: usize = 6;
        const CALLBACK_SIZES: [usize; 7] = [0, 1, 17, 2, 257, 492, 5];
        let input: Vec<_> = (0..CALLBACK_SIZES.iter().sum::<usize>())
            .map(|sample| sample as f32)
            .collect();
        let mut buffer = CaptureFrameBuffer::new(FRAME_SAMPLES).unwrap();
        let mut input_position = 0;
        let mut output_position = 0;

        assert!(CaptureFrameBuffer::new(0).is_err());
        assert_no_allocations(|| {
            for samples in CALLBACK_SIZES {
                let end = input_position + samples;
                buffer.process(input[input_position..end].iter().copied(), |frame| {
                    assert_eq!(
                        frame,
                        &input[output_position..output_position + FRAME_SAMPLES]
                    );
                    output_position += FRAME_SAMPLES;
                });
                input_position = end;
                assert_eq!(
                    output_position,
                    input_position / FRAME_SAMPLES * FRAME_SAMPLES
                );
            }
        });
        assert_eq!(output_position, input.len());
    }
}
