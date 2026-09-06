use ringbuf::{ring_buffer::RbBase, Rb};

pub(super) fn drain_audio_samples(
    buffer: &std::sync::Mutex<ringbuf::HeapRb<f32>>,
    output: &mut [f32],
    channels: std::num::NonZeroUsize,
) -> usize {
    let mut buffer = buffer.lock().unwrap();
    let channels = channels.get();
    let samples = buffer.occupied_len().min(output.len()) / channels * channels;
    buffer.pop_slice(&mut output[..samples]);
    samples
}

#[cfg(test)]
mod tests {
    use super::drain_audio_samples;
    use ringbuf::Rb;

    const BUFFER_CAPACITY: usize = 8;
    const CHANNELS: usize = 2;
    const INPUT: [f32; 4] = [0.1, 0.2, 0.3, 0.4];

    #[test]
    fn draining_audio_releases_the_buffer_lock() {
        let buffer = std::sync::Mutex::new(ringbuf::HeapRb::<f32>::new(BUFFER_CAPACITY));
        buffer.lock().unwrap().push_slice(&INPUT);
        let mut output = [0.0; INPUT.len()];

        assert_eq!(
            drain_audio_samples(
                &buffer,
                &mut output,
                std::num::NonZeroUsize::new(CHANNELS).unwrap(),
            ),
            INPUT.len()
        );
        assert_eq!(output, INPUT);
        assert!(buffer.try_lock().is_ok());
    }
}
