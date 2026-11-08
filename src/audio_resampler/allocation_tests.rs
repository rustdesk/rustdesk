use super::{AudioResamplerConfig, FixedFrameAudioResampler};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}

fn record_allocation() {
    let _ = ALLOCATIONS.try_with(|count| {
        if let Some(value) = count.get() {
            count.set(Some(value + 1));
        }
    });
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

pub(crate) fn assert_no_allocations(process: impl FnOnce()) {
    struct ResetCounter;
    impl Drop for ResetCounter {
        fn drop(&mut self) {
            ALLOCATIONS.with(|count| count.set(None));
        }
    }

    ALLOCATIONS.with(|count| assert!(count.replace(Some(0)).is_none()));
    let reset = ResetCounter;
    process();
    let allocations = ALLOCATIONS.with(|count| count.get().unwrap());
    drop(reset);
    assert_eq!(
        allocations, 0,
        "PCM processing allocated on the capture thread"
    );
}

#[test]
fn capture_resampling_reuses_buffers() {
    const PACKETS_PER_SECOND: usize = 100;
    const PACKET_COUNT: usize = 100;
    const MAX_STARTUP_DELAY_PACKETS: usize = 1;
    const SIGNAL_LEVEL: f32 = 0.25;
    const RATE_PAIRS: [(u32, u32); 6] = [
        (32_000, 24_000),
        (44_100, 24_000),
        (44_100, 48_000),
        (48_000, 24_000),
        (96_000, 48_000),
        (192_000, 48_000),
    ];

    for (input_rate, output_rate) in RATE_PAIRS {
        for channels in [1, 2, 4, 6, 8] {
            let config = AudioResamplerConfig {
                input_rate,
                output_rate,
                channels,
            };
            let input =
                vec![SIGNAL_LEVEL; input_rate as usize / PACKETS_PER_SECOND * channels as usize];
            let frames = output_rate as usize / PACKETS_PER_SECOND;
            let mut resampler = FixedFrameAudioResampler::new(config, frames).unwrap();
            let mut packets = 0;
            let mut energy = 0.0;
            assert_no_allocations(|| {
                for _ in 0..PACKET_COUNT {
                    resampler
                        .process_with(&input, |packet| {
                            assert_eq!(packet.len(), frames * channels as usize);
                            energy += packet.iter().map(|sample| sample * sample).sum::<f32>();
                            packets += 1;
                        })
                        .unwrap();
                }
            });
            assert!((PACKET_COUNT - MAX_STARTUP_DELAY_PACKETS..=PACKET_COUNT).contains(&packets));
            assert!(energy > SIGNAL_LEVEL);
        }
    }
}

#[cfg(all(feature = "use_samplerate", not(feature = "use_dasp")))]
#[test]
fn sinc_output_matches_the_existing_backend() {
    use super::AudioResampler;

    const INPUT_FRAMES: usize = 2_048;
    const CHUNK_FRAMES: usize = 73;
    const SIGNAL_STEP: f32 = 0.07;
    for (input_rate, output_rate) in [(44_100, 24_000), (44_100, 48_000), (96_000, 48_000)] {
        for channels in [1, 2, 4, 6, 8] {
            let config = AudioResamplerConfig {
                input_rate,
                output_rate,
                channels,
            };
            let input: Vec<_> = (0..INPUT_FRAMES * channels as usize)
                .map(|sample| (sample as f32 * SIGNAL_STEP).sin())
                .collect();
            let mut actual = AudioResampler::new(config).unwrap();
            let expected = samplerate::Samplerate::new(
                samplerate::ConverterType::SincBestQuality,
                input_rate,
                output_rate,
                channels as usize,
            )
            .unwrap();
            for chunk in input.chunks(CHUNK_FRAMES * channels as usize) {
                assert_eq!(
                    actual.process(chunk).unwrap(),
                    expected.process(chunk).unwrap()
                );
                assert_eq!(actual.process(&[]).unwrap(), expected.process(&[]).unwrap());
            }
        }
    }
}
