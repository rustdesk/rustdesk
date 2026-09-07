use ringbuf::{ring_buffer::RbBase, Rb};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

pub(super) struct AudioBufferSource<'a> {
    pub(super) buffer: &'a Mutex<ringbuf::HeapRb<f32>>,
    pub(super) discontinuity_generation: &'a AtomicUsize,
}

pub(super) struct DrainedAudio {
    pub(super) samples: usize,
    pub(super) discontinuity_generation: usize,
}

pub(super) fn drain_audio_samples(
    source: AudioBufferSource<'_>,
    output: &mut [f32],
    channels: std::num::NonZeroUsize,
) -> DrainedAudio {
    let mut buffer = source.buffer.lock().unwrap();
    let discontinuity_generation = source.discontinuity_generation.load(Ordering::Relaxed);
    let channels = channels.get();
    let samples = buffer.occupied_len().min(output.len()) / channels * channels;
    buffer.pop_slice(&mut output[..samples]);
    DrainedAudio {
        samples,
        discontinuity_generation,
    }
}

#[cfg(test)]
mod tests {
    use super::{drain_audio_samples, AudioBufferSource};
    use ringbuf::Rb;
    use std::sync::atomic::AtomicUsize;

    const BUFFER_CAPACITY: usize = 8;
    const CHANNELS: usize = 2;
    const DISCONTINUITY_GENERATION: usize = 7;
    const INPUT: [f32; 4] = [0.1, 0.2, 0.3, 0.4];

    #[test]
    fn draining_audio_releases_the_buffer_lock() {
        let buffer = std::sync::Mutex::new(ringbuf::HeapRb::<f32>::new(BUFFER_CAPACITY));
        buffer.lock().unwrap().push_slice(&INPUT);
        let discontinuity_generation = AtomicUsize::new(DISCONTINUITY_GENERATION);
        let mut output = [0.0; INPUT.len()];

        let drained = drain_audio_samples(
            AudioBufferSource {
                buffer: &buffer,
                discontinuity_generation: &discontinuity_generation,
            },
            &mut output,
            std::num::NonZeroUsize::new(CHANNELS).unwrap(),
        );
        assert_eq!(drained.samples, INPUT.len());
        assert_eq!(drained.discontinuity_generation, DISCONTINUITY_GENERATION);
        assert_eq!(output, INPUT);
        assert!(buffer.try_lock().is_ok());
    }
}
