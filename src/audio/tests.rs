#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_select_preferred_host_windows_asio() {
        // This test only runs on Windows; on other platforms we simply ensure the
        // function returns a host without panicking.
        #[cfg(target_os = "windows")]
        {
            let host = select_preferred_host();
            // The host should be either ASIO or the default host; both are valid.
            // We assert that the host ID is one of the known Windows host IDs.
            let id = host.id();
            assert!(
                matches!(id, cpal::HostId::Asio | cpal::HostId::Wasapi | cpal::HostId::Wdm),
                "Unexpected host ID on Windows: {:?}",
                id
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            // On non‑Windows platforms the function must return a host.
            let _ = select_preferred_host();
        }
    }

    #[test]
    fn test_audio_playback_starts() {
        // Simple sanity test that the playback stream can be created.
        // We generate silence for a few callbacks and then drop the stream.
        let counter = AtomicUsize::new(0);
        let generator = move |buf: &mut [f32]| {
            for sample in buf.iter_mut() {
                *sample = 0.0;
            }
            counter.fetch_add(1, Ordering::SeqCst);
        };

        let playback = start_playback(generator).expect("Failed to start playback");
        // Let the stream run briefly.
        std::thread::sleep(std::time::Duration::from_millis(100));
        // Ensure the callback was invoked at least once.
        assert!(counter.load(Ordering::SeqCst) > 0);
        drop(playback); // Stream stops when the handle is dropped.
    }
}
