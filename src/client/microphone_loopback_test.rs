use super::*;

#[test]
#[ignore = "Requires installed BlackHole 2ch and macOS audio permission; sends a synthetic tone only"]
fn microphone_blackhole_opus_loopback() {
    let device = AUDIO_HOST.input_devices().unwrap()
        .find(|d| d.name().ok().as_deref() == Some("BlackHole 2ch"))
        .expect("Install BlackHole 2ch");
    let config = cpal::StreamConfig {
        channels: 2, sample_rate: cpal::SampleRate(48_000), buffer_size: cpal::BufferSize::Default,
    };
    let captured = Arc::new(Mutex::new(Vec::<f32>::new()));
    let samples = captured.clone();
    let input = device.build_input_stream(&config, move |data: &[f32], _| {
        let mut samples = samples.lock().unwrap();
        if samples.len() < 192_000 { samples.extend_from_slice(data); }
    }, |error| eprintln!("BlackHole input error: {error}"), None).unwrap();
    input.play().unwrap();
    let (sender, ready) = start_microphone_audio_thread("BlackHole 2ch".to_owned(), AudioFormat {
        sample_rate: 48_000, channels: 2, ..Default::default()
    });
    ready.blocking_recv().unwrap().unwrap();
    let mut encoder = magnum_opus::Encoder::new(48_000, Stereo, magnum_opus::Application::LowDelay).unwrap();
    for packet in 0..50 {
        let pcm: Vec<f32> = (0..960).flat_map(|i| {
            let sample = (2.0 * std::f32::consts::PI * 440.0 * (packet * 960 + i) as f32 / 48_000.0).sin() * 0.15;
            [sample, sample]
        }).collect();
        let frame = AudioFrame { data: encoder.encode_vec_float(&pcm, 4_000).unwrap().into(), ..Default::default() };
        assert!(sender.send(frame));
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(sender);
    std::thread::sleep(Duration::from_millis(200));
    drop(input);
    let samples = captured.lock().unwrap();
    let signal_samples = samples.iter().filter(|s| s.abs() > 0.02).count();
    assert!(signal_samples > 10_000, "No substantial decoded signal reached BlackHole: {signal_samples} / {}", samples.len());
}
