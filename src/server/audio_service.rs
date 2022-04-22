// both soundio and cpal use wasapi on windows and coreaudio on mac, they do not support loopback.
// libpulseaudio support loopback because pulseaudio is a standalone audio service with some
// configuration, but need to install the library and start the service on OS, not a good choice.
// windows: https://docs.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording
// mac: https://github.com/mattingalls/Soundflower
// https://docs.microsoft.com/en-us/windows/win32/api/audioclient/nn-audioclient-iaudioclient
// https://github.com/ExistentialAudio/BlackHole

// if pactl not work, please run
// sudo apt-get --purge --reinstall install pulseaudio
// https://askubuntu.com/questions/403416/how-to-listen-live-sounds-from-input-from-external-sound-card
// https://wiki.debian.org/audio-loopback
// https://github.com/krruzic/pulsectl

use super::*;
use magnum_opus::{Application::*, Channels::*, Encoder};
use std::sync::atomic::{AtomicBool, Ordering};

pub const NAME: &'static str = "audio";
pub const AUDIO_DATA_SIZE_U8: usize = 960 * 4; // 10ms in 48000 stereo
static RESTARTING: AtomicBool = AtomicBool::new(false);

#[cfg(not(target_os = "linux"))]
pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.repeat::<cpal_impl::State, _>(33, cpal_impl::run);
    sp
}

#[cfg(target_os = "linux")]
pub fn new() -> GenericService {
    let sp = GenericService::new(NAME, true);
    sp.run(pa_impl::run);
    sp
}

pub fn restart() {
    log::info!("restart the audio service, freezing now...");
    if RESTARTING.load(Ordering::SeqCst) {
        return;
    }
    RESTARTING.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "linux")]
mod pa_impl {
    use super::*;
    #[tokio::main(flavor = "current_thread")]
    pub async fn run(sp: GenericService) -> ResultType<()> {
        hbb_common::sleep(0.1).await; // one moment to wait for _pa ipc
        RESTARTING.store(false, Ordering::SeqCst);
        let mut stream = crate::ipc::connect(1000, "_pa").await?;
        unsafe {
            AUDIO_ZERO_COUNT = 0;
        }
        let mut encoder = Encoder::new(crate::platform::linux::PA_SAMPLE_RATE, Stereo, LowDelay)?;
        allow_err!(
            stream
                .send(&crate::ipc::Data::Config((
                    "audio-input".to_owned(),
                    Some(Config::get_option("audio-input"))
                )))
                .await
        );
        let zero_audio_frame: Vec<f32> = vec![0.; AUDIO_DATA_SIZE_U8 / 4];
        while sp.ok() && !RESTARTING.load(Ordering::SeqCst) {
            sp.snapshot(|sps| {
                sps.send(create_format_msg(crate::platform::linux::PA_SAMPLE_RATE, 2));
                Ok(())
            })?;
            if let Ok(data) = stream.next_raw().await {
                if data.len() == 0 {
                    send_f32(&zero_audio_frame, &mut encoder, &sp);
                    continue;
                }
                if data.len() != AUDIO_DATA_SIZE_U8 {
                    continue;
                }
                let data = unsafe {
                    std::slice::from_raw_parts::<f32>(data.as_ptr() as _, data.len() / 4)
                };
                send_f32(data, &mut encoder, &sp);
            }
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod cpal_impl {
    use super::*;
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        Device, Host, SupportedStreamConfig,
    };

    lazy_static::lazy_static! {
        static ref HOST: Host = cpal::default_host();
    }

    #[derive(Default)]
    pub struct State {
        stream: Option<(Box<dyn StreamTrait>, Arc<Message>)>,
    }

    impl super::service::Reset for State {
        fn reset(&mut self) {
            self.stream.take();
        }
    }

    pub fn run(sp: GenericService, state: &mut State) -> ResultType<()> {
        sp.snapshot(|sps| {
            match &state.stream {
                None => {
                    state.stream = Some(play(&sp)?);
                }
                _ => {}
            }
            if let Some((_, format)) = &state.stream {
                sps.send_shared(format.clone());
            }
            Ok(())
        })?;
        Ok(())
    }

    fn send(
        data: &[f32],
        sample_rate0: u32,
        sample_rate: u32,
        channels: u16,
        encoder: &mut Encoder,
        sp: &GenericService,
    ) {
        let buffer;
        let data = if sample_rate0 != sample_rate {
            buffer = crate::common::resample_channels(data, sample_rate0, sample_rate, channels);
            &buffer
        } else {
            data
        };
        send_f32(data, encoder, sp);
    }

    #[cfg(windows)]
    fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
        let audio_input = Config::get_option("audio-input");
        if !audio_input.is_empty() {
            return get_audio_input(&audio_input);
        }
        let device = HOST
            .default_output_device()
            .with_context(|| "Failed to get default output device for loopback")?;
        log::info!(
            "Default output device: {}",
            device.name().unwrap_or("".to_owned())
        );
        let format = device
            .default_output_config()
            .map_err(|e| anyhow!(e))
            .with_context(|| "Failed to get default output format")?;
        log::info!("Default output format: {:?}", format);
        Ok((device, format))
    }

    #[cfg(not(windows))]
    fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
        let audio_input = Config::get_option("audio-input");
        get_audio_input(&audio_input)
    }

    fn get_audio_input(audio_input: &str) -> ResultType<(Device, SupportedStreamConfig)> {
        let mut device = None;
        if !audio_input.is_empty() {
            for d in HOST
                .devices()
                .with_context(|| "Failed to get audio devices")?
            {
                if d.name().unwrap_or("".to_owned()) == audio_input {
                    device = Some(d);
                    break;
                }
            }
        }
        if device.is_none() {
            device = Some(
                HOST.default_input_device()
                    .with_context(|| "Failed to get default input device for loopback")?,
            );
        }
        let device = device.unwrap();
        log::info!("Input device: {}", device.name().unwrap_or("".to_owned()));
        let format = device
            .default_input_config()
            .map_err(|e| anyhow!(e))
            .with_context(|| "Failed to get default input format")?;
        log::info!("Default input format: {:?}", format);
        Ok((device, format))
    }

    fn play(sp: &GenericService) -> ResultType<(Box<dyn StreamTrait>, Arc<Message>)> {
        let (device, config) = get_device()?;
        let sp = sp.clone();
        let err_fn = move |err| {
            // too many UnknownErrno, will improve later
            log::trace!("an error occurred on stream: {}", err);
        };
        // Sample rate must be one of 8000, 12000, 16000, 24000, or 48000.
        let sample_rate_0 = config.sample_rate().0;
        let sample_rate = if sample_rate_0 < 12000 {
            8000
        } else if sample_rate_0 < 16000 {
            12000
        } else if sample_rate_0 < 24000 {
            16000
        } else if sample_rate_0 < 48000 {
            24000
        } else {
            48000
        };
        log::debug!("Audio sample rate : {}", sample_rate);
        unsafe {
            AUDIO_ZERO_COUNT = 0;
        }
        let mut encoder = Encoder::new(
            sample_rate,
            if config.channels() > 1 { Stereo } else { Mono },
            LowDelay,
        )?;
        let channels = config.channels();
        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_input_stream(
                &config.into(),
                move |data, _: &_| {
                    send(
                        data,
                        sample_rate_0,
                        sample_rate,
                        channels,
                        &mut encoder,
                        &sp,
                    );
                },
                err_fn,
            )?,
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    let buffer: Vec<_> = data.iter().map(|s| cpal::Sample::to_f32(s)).collect();
                    send(
                        &buffer,
                        sample_rate_0,
                        sample_rate,
                        channels,
                        &mut encoder,
                        &sp,
                    );
                },
                err_fn,
            )?,
            cpal::SampleFormat::U16 => device.build_input_stream(
                &config.into(),
                move |data: &[u16], _: &_| {
                    let buffer: Vec<_> = data.iter().map(|s| cpal::Sample::to_f32(s)).collect();
                    send(
                        &buffer,
                        sample_rate_0,
                        sample_rate,
                        channels,
                        &mut encoder,
                        &sp,
                    );
                },
                err_fn,
            )?,
        };
        stream.play()?;
        Ok((
            Box::new(stream),
            Arc::new(create_format_msg(sample_rate, channels)),
        ))
    }
}

fn create_format_msg(sample_rate: u32, channels: u16) -> Message {
    let format = AudioFormat {
        sample_rate,
        channels: channels as _,
        ..Default::default()
    };
    let mut misc = Misc::new();
    misc.set_audio_format(format);
    let mut msg = Message::new();
    msg.set_misc(misc);
    msg
}

// use AUDIO_ZERO_COUNT for the Noise(Zero) Gate Attack Time
// every audio data length is set to 480
// MAX_AUDIO_ZERO_COUNT=800 is similar as Gate Attack Time 3~5s(Linux) || 6~8s(Windows)
const MAX_AUDIO_ZERO_COUNT: u16 = 800;
static mut AUDIO_ZERO_COUNT: u16 = 0;

fn send_f32(data: &[f32], encoder: &mut Encoder, sp: &GenericService) {
    if data.iter().filter(|x| **x != 0.).next().is_some() {
        unsafe {
            AUDIO_ZERO_COUNT = 0;
        }
    } else {
        unsafe {
            if AUDIO_ZERO_COUNT > MAX_AUDIO_ZERO_COUNT {
                if AUDIO_ZERO_COUNT == MAX_AUDIO_ZERO_COUNT + 1 {
                    log::debug!("Audio Zero Gate Attack");
                    AUDIO_ZERO_COUNT += 1;
                }
                return;
            }
            AUDIO_ZERO_COUNT += 1;
        }
    }
    match encoder.encode_vec_float(data, data.len() * 6) {
        Ok(data) => {
            let mut msg_out = Message::new();
            msg_out.set_audio_frame(AudioFrame {
                data,
                ..Default::default()
            });
            sp.send(msg_out);
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn test_pulse() {
        let spec = pulse::sample::Spec {
            format: pulse::sample::SAMPLE_FLOAT32NE,
            channels: 2,
            rate: 24000,
        };
        let hspec = hound::WavSpec {
            channels: spec.channels as _,
            sample_rate: spec.rate as _,
            bits_per_sample: (4 * 8) as _,
            sample_format: hound::SampleFormat::Float,
        };
        const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/recorded.wav");
        let mut writer =
            hound::WavWriter::create(PATH, hspec).expect("Could not create hsound writer");
        let device = crate::platform::linux::get_pa_monitor();
        let s = psimple::Simple::new(
            None,                             // Use the default server
            "Test",                           // Our application’s name
            pulse::stream::Direction::Record, // We want a record stream
            Some(&device),                    // Use the default device
            "Test",                           // Description of our stream
            &spec,                            // Our sample format
            None,                             // Use default channel map
            None,                             // Use default buffering attributes
        )
        .expect("Could not create simple pulse");
        let mut out: Vec<u8> = Vec::with_capacity(1024);
        unsafe {
            out.set_len(out.capacity());
        }
        for _ in 0..600 {
            s.read(&mut out).expect("Could not read pcm");
            let out2 =
                unsafe { std::slice::from_raw_parts::<f32>(out.as_ptr() as _, out.len() / 4) };
            for v in out2 {
                writer.write_sample(*v).ok();
            }
        }
        println!("{:?} {}", device, out.len());
        writer.finalize().expect("Could not finalize writer");
    }
}
