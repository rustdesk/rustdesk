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
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use hbb_common::anyhow::anyhow;
#[cfg(any(target_os = "linux", target_os = "android"))]
use magnum_opus::Application::LowDelay;
use magnum_opus::{Channels::*, Encoder};
use std::sync::atomic::{AtomicBool, Ordering};

pub const NAME: &'static str = "audio";
pub const AUDIO_DATA_SIZE_U8: usize = 960 * 4; // 10ms in 48000 stereo
static RESTARTING: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    static ref VOICE_CALL_INPUT_DEVICE: Arc::<Mutex::<Option<String>>> = Default::default();
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::repeat::<cpal_impl::State, _, _>(&svc.clone(), 33, cpal_impl::run);
    svc.sp
}

#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), pa_impl::run);
    svc.sp
}

#[inline]
pub fn get_voice_call_input_device() -> Option<String> {
    VOICE_CALL_INPUT_DEVICE.lock().unwrap().clone()
}

#[inline]
pub fn set_voice_call_input_device(device: Option<String>, set_if_present: bool) {
    if !set_if_present && VOICE_CALL_INPUT_DEVICE.lock().unwrap().is_some() {
        return;
    }

    if *VOICE_CALL_INPUT_DEVICE.lock().unwrap() == device {
        return;
    }
    *VOICE_CALL_INPUT_DEVICE.lock().unwrap() = device;
    restart();
}

#[inline]
fn get_audio_input() -> String {
    VOICE_CALL_INPUT_DEVICE
        .lock()
        .unwrap()
        .clone()
        .unwrap_or(Config::get_option("audio-input"))
}

pub fn restart() {
    log::info!("restart the audio service, freezing now...");
    if RESTARTING.load(Ordering::SeqCst) {
        return;
    }
    RESTARTING.store(true, Ordering::SeqCst);
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod pa_impl {
    use super::*;

    /// Reading the sample bytes back as `f32` needs a 4-byte aligned pointer.
    /// Returns an aligned copy only when `data` is not already aligned; `None`
    /// means the caller can reinterpret `data` where it is, with no copy.
    fn align_to_32_if_needed(data: &[u8]) -> Option<hbb_common::mem::AlignedU8Vec> {
        if (data.as_ptr() as usize & 3) == 0 {
            return None;
        }
        let mut buf = hbb_common::mem::aligned_u8_vec(data.len(), 4);
        buf.extend_from_slice(data);
        Some(buf)
    }

    #[tokio::main(flavor = "current_thread")]
    pub async fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
        hbb_common::sleep(0.1).await; // one moment to wait for _pa ipc
        RESTARTING.store(false, Ordering::SeqCst);
        #[cfg(target_os = "linux")]
        let mut stream = crate::ipc::connect(1000, "_pa").await?;
        let mut encoder = AudioEncoder::new(Encoder::new(
            crate::platform::PA_SAMPLE_RATE,
            Stereo,
            LowDelay,
        )?);
        #[cfg(target_os = "linux")]
        allow_err!(
            stream
                .send(&crate::ipc::Data::Config((
                    "audio-input".to_owned(),
                    Some(super::get_audio_input())
                )))
                .await
        );
        #[cfg(target_os = "linux")]
        let zero_audio_frame: Vec<f32> = vec![0.; AUDIO_DATA_SIZE_U8 / 4];
        #[cfg(target_os = "android")]
        let mut android_data = vec![];
        while sp.ok() && !RESTARTING.load(Ordering::SeqCst) {
            sp.snapshot(|sps| {
                sps.send(create_format_msg(crate::platform::PA_SAMPLE_RATE, 2));
                Ok(())
            })?;

            #[cfg(target_os = "linux")]
            if let Ok(data) = stream.next_raw().await {
                if data.len() == 0 {
                    send_f32(&zero_audio_frame, &mut encoder, &sp);
                    continue;
                }

                if data.len() != AUDIO_DATA_SIZE_U8 {
                    continue;
                }

                let data: Vec<u8> = data.into();
                let aligned = align_to_32_if_needed(&data);
                let bytes = aligned.as_deref().unwrap_or(&data[..]);
                // SAFETY: `bytes` is 4-byte aligned (either checked above or freshly
                // allocated with align 4), and only whole f32s are read from it.
                let data = unsafe {
                    std::slice::from_raw_parts::<f32>(bytes.as_ptr() as _, bytes.len() / 4)
                };
                send_f32(data, &mut encoder, &sp);
            }

            #[cfg(target_os = "android")]
            if scrap::android::ffi::get_audio_raw(&mut android_data, &mut vec![]).is_some() {
                // Keep `android_data` as the reusable receive buffer: overwriting it with
                // an exact-capacity aligned buffer only made the next `get_audio_raw`
                // reallocate it, which dropped the alignment again.
                let aligned = align_to_32_if_needed(&android_data);
                let bytes = aligned.as_deref().unwrap_or(&android_data[..]);
                // SAFETY: `bytes` is 4-byte aligned (either checked above or freshly
                // allocated with align 4), and only whole f32s are read from it.
                let data = unsafe {
                    std::slice::from_raw_parts::<f32>(bytes.as_ptr() as _, bytes.len() / 4)
                };
                send_f32(data, &mut encoder, &sp);
            } else {
                hbb_common::sleep(0.1).await;
            }
        }
        Ok(())
    }
}

#[inline]
#[cfg(feature = "screencapturekit")]
pub fn is_screen_capture_kit_available() -> bool {
    cpal::available_hosts()
        .iter()
        .any(|host| *host == cpal::HostId::ScreenCaptureKit)
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture_error;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod audio_capture_queue;

#[cfg(not(any(target_os = "linux", target_os = "android")))]
mod cpal_impl {
    use self::service::{Reset, ServiceSwap};
    use super::audio_capture_error::CaptureErrorHandler;
    use super::*;
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        Device, Host, InputCallbackInfo, SupportedStreamConfig,
    };

    lazy_static::lazy_static! {
        static ref HOST: Host = cpal::default_host();
    }

    const AUDIO_PACKETS_PER_SECOND: usize = 100;

    #[cfg(feature = "screencapturekit")]
    lazy_static::lazy_static! {
        static ref HOST_SCREEN_CAPTURE_KIT: Result<Host, cpal::HostUnavailable> = cpal::host_from_id(cpal::HostId::ScreenCaptureKit);
    }

    #[derive(Default)]
    pub struct State {
        stream: Option<ActiveCaptureStream>,
    }

    struct ActiveCaptureStream {
        stream: Option<Box<dyn StreamTrait>>,
        format: Arc<Message>,
        _encoder_worker: audio_capture_queue::CaptureEncoderWorker,
        errors: CaptureErrorHandler,
    }

    impl Drop for ActiveCaptureStream {
        fn drop(&mut self) {
            self.stream.take();
        }
    }

    impl super::service::Reset for State {
        fn reset(&mut self) {
            self.stream.take();
        }
    }

    fn run_restart(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
        state.reset();
        sp.snapshot(|_sps: ServiceSwap<_>| Ok(()))?;
        match &state.stream {
            None => {
                state.stream = Some(play(&sp)?);
            }
            _ => {}
        }
        if let Some(stream) = &state.stream {
            sp.send_shared(stream.format.clone());
            #[cfg(target_os = "macos")]
            log::info!("Audio capture stream recreated; replacement format sent");
        }
        RESTARTING.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn run_serv_snapshot(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
        sp.snapshot(|sps| {
            match &state.stream {
                None => {
                    state.stream = Some(play(&sp)?);
                }
                _ => {}
            }
            if let Some(stream) = &state.stream {
                sps.send_shared(stream.format.clone());
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn run(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
        if let Some(stream) = &state.stream {
            if stream.errors.needs_restart() {
                // Recreate on the service thread, outside the capture callbacks.
                log::warn!("Recreating audio capture stream after an error");
                super::restart();
            }
        }
        if !RESTARTING.load(Ordering::SeqCst) {
            run_serv_snapshot(sp, state)
        } else {
            run_restart(sp, state)
        }
    }

    #[derive(Clone, Copy)]
    struct CaptureFrameProcessorConfig {
        input_rate: u32,
        output_rate: u32,
        device_channel: u16,
        encode_channel: u16,
    }

    struct CaptureFrameProcessor {
        config: CaptureFrameProcessorConfig,
        resampler: Option<crate::audio_resampler::FixedFrameAudioResampler>,
        sender: audio_capture_queue::CapturePcmSender,
        rechannel_buffer: Vec<f32>,
    }

    struct CaptureStreamOutput {
        sender: audio_capture_queue::CapturePcmSender,
        sample_rate: u32,
        encode_channel: magnum_opus::Channels,
    }

    impl CaptureFrameProcessor {
        fn new(
            config: CaptureFrameProcessorConfig,
            sender: audio_capture_queue::CapturePcmSender,
        ) -> ResultType<Self> {
            let resampler = if config.input_rate == config.output_rate {
                None
            } else {
                let output_frames = config.output_rate as usize / AUDIO_PACKETS_PER_SECOND;
                Some(crate::audio_resampler::FixedFrameAudioResampler::new(
                    crate::audio_resampler::AudioResamplerConfig {
                        input_rate: config.input_rate,
                        output_rate: config.output_rate,
                        channels: config.device_channel,
                    },
                    output_frames,
                )?)
            };
            Ok(Self {
                config,
                resampler,
                sender,
                rechannel_buffer: Vec::with_capacity(
                    capture_packet_layout(config.output_rate, config.encode_channel)?.1,
                ),
            })
        }

        fn process(&mut self, data: &[f32]) -> ResultType<()> {
            let config = self.config;
            let sender = &mut self.sender;
            let rechannel_buffer = &mut self.rechannel_buffer;
            let mut send_packet = |packet: &[f32]| {
                let packet =
                    audio_capture::rechannel(packet, config.device_channel, rechannel_buffer);
                sender.submit(packet);
            };
            if let Some(resampler) = self.resampler.as_mut() {
                resampler.process_with(data, send_packet).with_context(|| {
                    format!(
                        "Failed to resample captured audio from {} Hz to {} Hz",
                        config.input_rate, config.output_rate
                    )
                })?;
            } else {
                send_packet(data);
            }
            Ok(())
        }
    }

    fn capture_packet_layout(sample_rate: u32, channels: u16) -> ResultType<(usize, usize)> {
        if sample_rate < AUDIO_PACKETS_PER_SECOND as u32 || channels == 0 {
            bail!("Invalid audio capture layout: sample_rate={sample_rate}, channels={channels}");
        }
        let frames = sample_rate as usize / AUDIO_PACKETS_PER_SECOND;
        let samples = frames.checked_mul(channels as usize).with_context(|| {
            format!(
                "Audio capture frame size overflow: sample_rate={sample_rate}, channels={channels}"
            )
        })?;
        Ok((frames, samples))
    }

    #[cfg(feature = "screencapturekit")]
    fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
        let audio_input = super::get_audio_input();
        if !audio_input.is_empty() {
            return get_audio_input(&audio_input);
        }
        if !is_screen_capture_kit_available() {
            return get_audio_input("");
        }
        let device = HOST_SCREEN_CAPTURE_KIT
            .as_ref()?
            .default_input_device()
            .with_context(|| "Failed to get default input device for loopback")?;
        let format = device
            .default_input_config()
            .map_err(|e| anyhow!(e))
            .with_context(|| "Failed to get input output format")?;
        log::info!("Default input format: {:?}", format);
        Ok((device, format))
    }

    #[cfg(windows)]
    fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
        let audio_input = super::get_audio_input();
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

    #[cfg(not(any(windows, feature = "screencapturekit")))]
    fn get_device() -> ResultType<(Device, SupportedStreamConfig)> {
        let audio_input = super::get_audio_input();
        get_audio_input(&audio_input)
    }

    fn get_audio_input(audio_input: &str) -> ResultType<(Device, SupportedStreamConfig)> {
        let mut device = None;
        #[cfg(feature = "screencapturekit")]
        if !audio_input.is_empty() && is_screen_capture_kit_available() {
            for d in HOST_SCREEN_CAPTURE_KIT
                .as_ref()?
                .devices()
                .with_context(|| "Failed to get audio devices")?
            {
                if d.name().unwrap_or("".to_owned()) == audio_input {
                    device = Some(d);
                    break;
                }
            }
        }
        if device.is_none() && !audio_input.is_empty() {
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
        let device = device.unwrap_or(
            HOST.default_input_device()
                .with_context(|| "Failed to get default input device for loopback")?,
        );
        log::info!("Input device: {}", device.name().unwrap_or("".to_owned()));
        let format = device
            .default_input_config()
            .map_err(|e| anyhow!(e))
            .with_context(|| "Failed to get default input format")?;
        log::info!("Default input format: {:?}", format);
        Ok((device, format))
    }

    fn play(sp: &GenericService) -> ResultType<ActiveCaptureStream> {
        use cpal::SampleFormat::*;
        let (device, config) = get_device()?;
        let sp = sp.clone();
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
        let ch = if config.channels() > 1 { Stereo } else { Mono };
        let max_channels = config.channels().max(ch as u16);
        let (_, max_packet_samples) = capture_packet_layout(sample_rate, max_channels)?;
        let encoder_config = audio_capture_queue::CaptureEncoderConfig {
            sample_rate,
            encode_channel: ch,
            max_packet_samples,
        };
        let (sender, encoder_worker) =
            audio_capture_queue::start_capture_encoder(encoder_config, sp)?;
        let output = CaptureStreamOutput {
            sender,
            sample_rate,
            encode_channel: ch,
        };
        let (stream, errors) = match config.sample_format() {
            I8 => build_input_stream::<i8>(device, &config, output)?,
            I16 => build_input_stream::<i16>(device, &config, output)?,
            I32 => build_input_stream::<i32>(device, &config, output)?,
            I64 => build_input_stream::<i64>(device, &config, output)?,
            U8 => build_input_stream::<u8>(device, &config, output)?,
            U16 => build_input_stream::<u16>(device, &config, output)?,
            U32 => build_input_stream::<u32>(device, &config, output)?,
            U64 => build_input_stream::<u64>(device, &config, output)?,
            F32 => build_input_stream::<f32>(device, &config, output)?,
            F64 => build_input_stream::<f64>(device, &config, output)?,
            f => bail!("unsupported audio format: {:?}", f),
        };
        stream.play()?;
        #[cfg(target_os = "macos")]
        log::info!("Audio capture start call succeeded");
        Ok(ActiveCaptureStream {
            stream: Some(Box::new(stream)),
            format: Arc::new(create_format_msg(sample_rate, ch as _)),
            _encoder_worker: encoder_worker,
            errors,
        })
    }

    fn convert_input_samples<T>(data: &[T]) -> impl Iterator<Item = f32> + '_
    where
        T: cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        data.iter()
            .map(|sample| <f32 as cpal::FromSample<T>>::from_sample_(*sample))
    }

    #[cfg(target_os = "macos")]
    fn log_capture_startup<T>(
        data: &[T],
        received_samples: bool,
        received_signal: bool,
    ) -> (bool, bool)
    where
        T: cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        // Starting capture does not guarantee sample delivery or audible data.
        if !received_samples && !data.is_empty() {
            log::info!(
                "Audio capture received first PCM block: {} samples",
                data.len()
            );
        }
        let has_signal = received_signal
            || convert_input_samples(data).any(|sample| sample.is_finite() && sample != 0.0);
        if !received_signal && has_signal {
            log::info!("Audio capture received first nonzero PCM");
        }
        (received_samples || !data.is_empty(), has_signal)
    }

    fn build_input_stream<T>(
        device: cpal::Device,
        config: &cpal::SupportedStreamConfig,
        output: CaptureStreamOutput,
    ) -> ResultType<(cpal::Stream, CaptureErrorHandler)>
    where
        T: cpal::SizedSample,
        f32: cpal::FromSample<T>,
    {
        let errors = CaptureErrorHandler::default();
        let callback_errors = errors.clone();
        let err_fn = move |err| callback_errors.handle(err);
        let processor_errors = errors.clone();
        #[cfg(target_os = "macos")]
        let (mut received_samples, mut received_signal) = (false, false);
        let sample_rate_0 = config.sample_rate().0;
        log::debug!("Audio sample rate : {}", output.sample_rate);
        let device_channel = config.channels();
        let (_, capture_frame_samples) = capture_packet_layout(sample_rate_0, device_channel)?;
        let mut frame = audio_capture::CaptureFrameBuffer::new(capture_frame_samples)?;
        let processor_config = CaptureFrameProcessorConfig {
            input_rate: sample_rate_0,
            output_rate: output.sample_rate,
            device_channel,
            encode_channel: output.encode_channel as _,
        };
        let mut processor = CaptureFrameProcessor::new(processor_config, output.sender)?;
        let timeout = None;
        let stream = device.build_input_stream(
            &config.config(),
            move |data: &[T], _: &InputCallbackInfo| {
                if processor_errors.needs_restart() {
                    return;
                }
                #[cfg(target_os = "macos")]
                {
                    (received_samples, received_signal) =
                        log_capture_startup(data, received_samples, received_signal);
                }
                frame.process(convert_input_samples(data), |frame| {
                    processor_errors.process_frame(|| processor.process(frame));
                });
            },
            err_fn,
            timeout,
        )?;
        Ok((stream, errors))
    }

    #[cfg(test)]
    mod tests {
        use super::super::audio_capture_queue::{
            new_pcm_handoff, start_capture_encoder, CaptureEncoderConfig,
        };
        use super::{
            capture_packet_layout, convert_input_samples, CaptureFrameProcessor,
            CaptureFrameProcessorConfig,
        };
        use crate::audio_resampler::allocation_tests::assert_no_allocations;
        use crate::server::EmptyExtraFieldService;
        use magnum_opus::Channels::{Mono, Stereo};

        const INVALID_CAPTURE_RATE: u32 = 99;
        const RATE_24_KHZ: u32 = 24_000;
        const RATE_44_1_KHZ: u32 = 44_100;
        const RATE_48_KHZ: u32 = 48_000;
        const MONO_CHANNELS: u16 = 1;
        const NEGATIVE_FULL_SCALE_LIMIT: f32 = -0.99;
        const POSITIVE_FULL_SCALE_LIMIT: f32 = 0.99;
        const STEREO_CHANNELS: u16 = 2;
        const SURROUND_CHANNELS: u16 = 6;
        const ZERO_CHANNELS: u16 = 0;

        #[test]
        fn capture_sample_conversion_uses_cpal_traits() {
            let input = [i16::MIN, 0, i16::MAX];
            let output: Vec<_> = convert_input_samples(&input).collect();

            assert_eq!(output.len(), input.len());
            assert!(output[0] <= NEGATIVE_FULL_SCALE_LIMIT);
            assert_eq!(output[1], 0.0);
            assert!(output[2] >= POSITIVE_FULL_SCALE_LIMIT);
        }

        #[test]
        fn capture_packet_layout_validates_rate_and_channels() {
            let expected_frames = RATE_48_KHZ as usize / super::AUDIO_PACKETS_PER_SECOND;
            assert_eq!(
                capture_packet_layout(RATE_48_KHZ, STEREO_CHANNELS).unwrap(),
                (expected_frames, expected_frames * STEREO_CHANNELS as usize)
            );
            assert!(capture_packet_layout(INVALID_CAPTURE_RATE, MONO_CHANNELS).is_err());
            assert!(capture_packet_layout(RATE_48_KHZ, ZERO_CHANNELS).is_err());
        }

        #[test]
        fn capture_callback_pipeline_does_not_allocate_after_warmup() {
            for (input_rate, output_rate, device_channel, encode_channel) in [
                (RATE_48_KHZ, RATE_48_KHZ, MONO_CHANNELS, MONO_CHANNELS),
                (RATE_48_KHZ, RATE_48_KHZ, STEREO_CHANNELS, STEREO_CHANNELS),
                (RATE_44_1_KHZ, RATE_24_KHZ, STEREO_CHANNELS, STEREO_CHANNELS),
                (RATE_48_KHZ, RATE_48_KHZ, SURROUND_CHANNELS, STEREO_CHANNELS),
            ] {
                assert_capture_processor_does_not_allocate(CaptureFrameProcessorConfig {
                    input_rate,
                    output_rate,
                    device_channel,
                    encode_channel,
                });
            }
        }

        #[test]
        fn capture_pcm_handoff_reuses_buffers_and_accounts_for_loss() {
            const QUEUE_CAPACITY: usize = 2;
            const PACKET_SAMPLES: usize = 4;
            const FIRST: [f32; PACKET_SAMPLES] = [1.0; PACKET_SAMPLES];
            const SECOND: [f32; PACKET_SAMPLES] = [2.0; PACKET_SAMPLES];
            const THIRD: [f32; PACKET_SAMPLES] = [3.0; PACKET_SAMPLES];
            const OVERSIZED_SAMPLES: usize = PACKET_SAMPLES + 1;
            const OVERSIZED: [f32; OVERSIZED_SAMPLES] = [1.0; OVERSIZED_SAMPLES];

            let (mut sender, receiver) = new_pcm_handoff(QUEUE_CAPACITY, PACKET_SAMPLES).unwrap();
            sender.set_wake_thread(std::thread::current()).unwrap();
            assert_no_allocations(|| {
                sender.submit(&FIRST);
                sender.submit(&SECOND);
                sender.submit(&THIRD);
            });

            let loss = receiver.take_loss();
            assert_eq!(loss.dropped, 1);
            assert_eq!(loss.oversized, 0);
            assert_eq!(loss.recycle_failures, 0);
            let second = receiver.pop().unwrap();
            let third = receiver.pop().unwrap();
            assert_eq!(second, SECOND);
            assert_eq!(third, THIRD);
            receiver.recycle(second);
            receiver.recycle(third);
            assert!(receiver.is_empty());

            assert_no_allocations(|| sender.submit(&OVERSIZED));
            let loss = receiver.take_loss();
            assert_eq!(loss.dropped, 0);
            assert_eq!(loss.oversized, 1);
            assert_eq!(loss.recycle_failures, 0);
            assert!(receiver.is_empty());
        }

        #[test]
        fn capture_pcm_handoff_rejects_invalid_layouts() {
            assert!(new_pcm_handoff(0, 1).is_err());
            assert!(new_pcm_handoff(1, 0).is_err());
        }

        fn assert_capture_processor_does_not_allocate(config: CaptureFrameProcessorConfig) {
            const INPUT_LEVEL: f32 = 0.25;
            const TEST_SERVICE_NAME: &str = "audio-allocation-test";

            let service = EmptyExtraFieldService::new(TEST_SERVICE_NAME.to_owned(), true).sp;
            let encode_channel = if config.encode_channel == MONO_CHANNELS {
                Mono
            } else {
                Stereo
            };
            let encoder_config = CaptureEncoderConfig {
                sample_rate: config.output_rate,
                encode_channel,
                max_packet_samples: config.output_rate as usize / super::AUDIO_PACKETS_PER_SECOND
                    * config.device_channel.max(config.encode_channel) as usize,
            };
            let (sender, worker) = start_capture_encoder(encoder_config, service).unwrap();
            let mut processor = CaptureFrameProcessor::new(config, sender).unwrap();
            let errors = super::CaptureErrorHandler::default();
            let input = vec![
                INPUT_LEVEL;
                config.input_rate as usize / super::AUDIO_PACKETS_PER_SECOND
                    * config.device_channel as usize
            ];
            let mut frame_buffer =
                super::audio_capture::CaptureFrameBuffer::new(input.len()).unwrap();

            frame_buffer.process(convert_input_samples(&input), |frame| {
                errors.process_frame(|| processor.process(frame));
            });
            assert_no_allocations(|| {
                frame_buffer.process(convert_input_samples(&input), |frame| {
                    errors.process_frame(|| processor.process(frame));
                });
            });
            assert!(!errors.needs_restart());
            drop(processor);
            drop(worker);
        }
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

// Use a per-encoder counter for the Noise(Zero) Gate Attack Time.
// every audio data length is set to 480
// MAX_AUDIO_ZERO_COUNT=800 is similar as Gate Attack Time 3~5s(Linux) || 6~8s(Windows)
const MAX_AUDIO_ZERO_COUNT: u16 = 800;

struct AudioEncoder {
    encoder: Encoder,
    zero_count: u16,
}

impl AudioEncoder {
    fn new(encoder: Encoder) -> Self {
        Self {
            encoder,
            zero_count: 0,
        }
    }

    fn should_encode(&mut self, data: &[f32]) -> bool {
        if data.iter().filter(|x| **x != 0.).next().is_some() {
            self.zero_count = 0;
        } else if self.zero_count > MAX_AUDIO_ZERO_COUNT {
            if self.zero_count == MAX_AUDIO_ZERO_COUNT + 1 {
                log::debug!("Audio Zero Gate Attack");
                self.zero_count += 1;
            }
            return false;
        } else {
            self.zero_count += 1;
        }
        true
    }
}

fn send_f32(data: &[f32], encoder: &mut AudioEncoder, sp: &GenericService) {
    if !encoder.should_encode(data) {
        return;
    }
    #[cfg(target_os = "android")]
    {
        // the permitted opus data size are 120, 240, 480, 960, 1920, and 2880
        // if data size is bigger than BATCH_SIZE, AND is an integer multiple of BATCH_SIZE
        // then upload in batches
        const BATCH_SIZE: usize = 960;
        let input_size = data.len();
        if input_size > BATCH_SIZE && input_size % BATCH_SIZE == 0 {
            let n = input_size / BATCH_SIZE;
            for i in 0..n {
                match encoder
                    .encoder
                    .encode_vec_float(&data[i * BATCH_SIZE..(i + 1) * BATCH_SIZE], BATCH_SIZE)
                {
                    Ok(data) => {
                        let mut msg_out = Message::new();
                        msg_out.set_audio_frame(AudioFrame {
                            data: data.into(),
                            ..Default::default()
                        });
                        sp.send(msg_out);
                    }
                    Err(error) => log::warn!("Failed to encode audio frame: {error:?}"),
                }
            }
        } else {
            log::debug!("invalid audio data size:{} ", input_size);
            return;
        }
    }

    #[cfg(not(target_os = "android"))]
    match encoder.encoder.encode_vec_float(data, data.len() * 6) {
        Ok(data) => {
            let mut msg_out = Message::new();
            msg_out.set_audio_frame(AudioFrame {
                data: data.into(),
                ..Default::default()
            });
            sp.send(msg_out);
        }
        Err(error) => log::warn!("Failed to encode audio frame: {error:?}"),
    }
}
