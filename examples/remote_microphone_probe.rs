//! Inject a synthetic tone into the virtual source without capturing a real microphone.
//! cargo run --example remote_microphone_probe --features remote-microphone,linux-pkg-config
#[cfg(all(target_os = "linux", feature = "remote-microphone"))]
#[path = "../src/remote_microphone.rs"]
mod remote_microphone;

#[cfg(all(target_os = "linux", feature = "remote-microphone"))]
fn main() -> hbb_common::ResultType<()> {
    use magnum_opus::{Application, Channels, Encoder};
    use std::{sync::Arc, time::Duration};
    let receiver = remote_microphone::Receiver::acquire("probe")?;
    receiver.activate(1)?;
    let mut encoder = Encoder::new(48000, Channels::Stereo, Application::LowDelay)?;
    println!("Select RustDeskRemoteMicrophone in an audio recorder. Tone for 3s, silence for 1s, tone for 4s.");
    for n in 0..800 {
        if n == 300 {
            receiver.silence();
        }
        if n == 400 {
            receiver.activate(2)?;
        }
        let mut samples = [0f32; 960];
        for i in 0..480 {
            let value =
                ((n * 480 + i) as f32 * std::f32::consts::TAU * 440.0 / 48000.0).sin() * 0.2;
            samples[i * 2] = value;
            samples[i * 2 + 1] = value;
        }
        receiver.push(
            if n < 400 { 1 } else { 2 },
            &encoder.encode_vec_float(&samples, 4096)?,
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    remote_microphone::Receiver::park("probe".into(), receiver.clone());
    let recovered = remote_microphone::Receiver::acquire("probe")?;
    assert!(Arc::ptr_eq(&receiver, &recovered));
    assert!(!recovered.is_active());
    Ok(())
}

#[cfg(not(all(target_os = "linux", feature = "remote-microphone")))]
fn main() {
    eprintln!("Requires Linux and --features remote-microphone");
}
