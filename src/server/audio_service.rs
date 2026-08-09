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
use magnum_opus::{Application::*, Channels::*, Encoder};
use std::sync::atomic::{AtomicBool, Ordering};

pub const NAME: &'static str = "audio";
pub const AUDIO_DATA_SIZE_U8: usize = 960 * 4; // 10ms in 48000 stereo
pub const ASIO_INPUT_PREFIX: &str = "[ASIO] ";
static RESTARTING: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq, Eq)]
struct AsioInputChoice {
    display: String,
    channels: Vec<usize>,
}

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

fn asio_input_choices(device_name: &str, channel_count: u16) -> Vec<AsioInputChoice> {
    let channel_count = channel_count as usize;
    let mut choices = Vec::with_capacity((channel_count + 1) / 2);
    for first in (0..channel_count).step_by(2) {
        let channels = if first + 1 < channel_count {
            vec![first, first + 1]
        } else {
            vec![first]
        };
        let display = if channels.len() == 2 {
            format!(
                "{ASIO_INPUT_PREFIX}{device_name} - Inputs {}-{}",
                first + 1,
                first + 2
            )
        } else {
            format!("{ASIO_INPUT_PREFIX}{device_name} - Input {}", first + 1)
        };
        choices.push(AsioInputChoice { display, channels });
    }
    choices
}

fn select_interleaved_channels(
    data: &[f32],
    source_channels: usize,
    selected_channels: &[usize],
) -> Option<Vec<f32>> {
    if source_channels == 0
        || selected_channels.is_empty()
        || selected_channels
            .iter()
            .any(|channel| *channel >= source_channels)
    {
        return None;
    }
    let frame_count = data.len() / source_channels;
    let mut out = Vec::with_capacity(frame_count * selected_channels.len());
    for frame in data.chunks_exact(source_channels) {
        for channel in selected_channels {
            out.push(frame[*channel]);
        }
    }
    Some(out)
}

#[cfg(all(windows, feature = "asio"))]
pub fn get_asio_input_devices() -> Vec<String> {
    cpal_impl::get_asio_input_devices()
}

#[cfg(not(all(windows, feature = "asio")))]
pub fn get_asio_input_devices() -> Vec<String> {
    Vec::new()
}

pub fn append_asio_input_devices(out: &mut Vec<String>) {
    out.extend(get_asio_input_devices());
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
        unsafe {
            AUDIO_ZERO_COUNT = 0;
        }
        let mut encoder = Encoder::new(crate::platform::PA_SAMPLE_RATE, Stereo, LowDelay)?;
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
mod cpal_impl {
    use self::service::{Reset, ServiceSwap};
    use super::*;
    use cpal::{
        traits::{DeviceTrait, HostTrait, StreamTrait},
        BufferSize, Device, Host, InputCallbackInfo, StreamConfig, SupportedStreamConfig,
    };

    type CaptureDevice = (Device, SupportedStreamConfig, Option<Vec<usize>>);

    lazy_static::lazy_static! {
        static ref HOST: Host = cpal::default_host();
        static ref INPUT_BUFFER: Arc<Mutex<std::collections::VecDeque<f32>>> = Default::default();
    }

    #[cfg(feature = "screencapturekit")]
    lazy_static::lazy_static! {
        static ref HOST_SCREEN_CAPTURE_KIT: Result<Host, cpal::HostUnavailable> = cpal::host_from_id(cpal::HostId::ScreenCaptureKit);
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

    fn run_restart(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
        state.reset();
        sp.snapshot(|_sps: ServiceSwap<_>| Ok(()))?;
        match &state.stream {
            None => {
                state.stream = Some(play(&sp)?);
            }
            _ => {}
        }
        if let Some((_, format)) = &state.stream {
            sp.send_shared(format.clone());
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
            if let Some((_, format)) = &state.stream {
                sps.send_shared(format.clone());
            }
            Ok(())
        })?;
        Ok(())
    }

    pub fn run(sp: EmptyExtraFieldService, state: &mut State) -> ResultType<()> {
        if !RESTARTING.load(Ordering::SeqCst) {
            run_serv_snapshot(sp, state)
        } else {
            run_restart(sp, state)
        }
    }

    fn send(
        data: Vec<f32>,
        sample_rate0: u32,
        sample_rate: u32,
        device_channel: u16,
        encode_channel: u16,
        encoder: &mut Encoder,
        sp: &GenericService,
    ) {
        let mut data = data;
        if sample_rate0 != sample_rate {
            data = crate::common::audio_resample(&data, sample_rate0, sample_rate, device_channel);
        }
        if device_channel != encode_channel {
            data = crate::common::audio_rechannel(
                data,
                sample_rate,
                sample_rate,
                device_channel,
                encode_channel,
            )
        }
        send_f32(&data, encoder, sp);
    }

    #[cfg(feature = "screencapturekit")]
    fn get_device() -> ResultType<CaptureDevice> {
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
        Ok((device, format, None))
    }

    #[cfg(windows)]
    fn get_device() -> ResultType<CaptureDevice> {
        let audio_input = super::get_audio_input();
        if !audio_input.is_empty() {
            #[cfg(feature = "asio")]
            if audio_input.starts_with(super::ASIO_INPUT_PREFIX) {
                return get_asio_audio_input(&audio_input);
            }
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
        Ok((device, format, None))
    }

    #[cfg(all(windows, feature = "asio"))]
    pub(super) fn get_asio_input_devices() -> Vec<String> {
        let mut out = Vec::new();
        let host = match cpal::host_from_id(cpal::HostId::Asio) {
            Ok(host) => host,
            Err(err) => {
                log::debug!("Failed to initialize ASIO host: {:?}", err);
                return out;
            }
        };
        let devices = match host.devices() {
            Ok(devices) => devices,
            Err(err) => {
                log::debug!("Failed to enumerate ASIO devices: {:?}", err);
                return out;
            }
        };
        for device in devices {
            let device_name = match device.name() {
                Ok(name) => name,
                Err(err) => {
                    log::debug!("Failed to get ASIO device name: {:?}", err);
                    continue;
                }
            };
            let format = match device.default_input_config() {
                Ok(format) => format,
                Err(err) => {
                    log::debug!(
                        "Failed to get ASIO input config for {}: {:?}",
                        device_name,
                        err
                    );
                    continue;
                }
            };
            out.extend(
                super::asio_input_choices(&device_name, format.channels())
                    .into_iter()
                    .map(|choice| choice.display),
            );
        }
        out
    }

    #[cfg(all(windows, feature = "asio"))]
    fn get_asio_audio_input(audio_input: &str) -> ResultType<CaptureDevice> {
        let host = cpal::host_from_id(cpal::HostId::Asio)
            .map_err(|e| anyhow!("{:?}", e))
            .with_context(|| "Failed to initialize ASIO host")?;
        for device in host
            .devices()
            .with_context(|| "Failed to enumerate ASIO devices")?
        {
            let device_name = match device.name() {
                Ok(name) => name,
                Err(err) => {
                    log::debug!("Failed to get ASIO device name: {:?}", err);
                    continue;
                }
            };
            let format = match device.default_input_config() {
                Ok(format) => format,
                Err(err) => {
                    log::debug!(
                        "Failed to get ASIO input config for {}: {:?}",
                        device_name,
                        err
                    );
                    continue;
                }
            };
            if let Some(choice) = super::asio_input_choices(&device_name, format.channels())
                .into_iter()
                .find(|choice| choice.display == audio_input)
            {
                log::info!("ASIO input route: {}", choice.display);
                log::info!("ASIO input format: {:?}", format);
                return Ok((device, format, Some(choice.channels)));
            }
        }
        bail!("Failed to get ASIO input route: {}", audio_input);
    }

    #[cfg(not(any(windows, feature = "screencapturekit")))]
    fn get_device() -> ResultType<CaptureDevice> {
        let audio_input = super::get_audio_input();
        get_audio_input(&audio_input)
    }

    fn get_audio_input(audio_input: &str) -> ResultType<CaptureDevice> {
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
        Ok((device, format, None))
    }

    fn play(sp: &GenericService) -> ResultType<(Box<dyn StreamTrait>, Arc<Message>)> {
        use cpal::SampleFormat::*;
        let (device, config, selected_channels) = get_device()?;
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
        let capture_channel_count = selected_channels
            .as_ref()
            .map(|channels| channels.len() as u16)
            .unwrap_or_else(|| config.channels());
        let ch = if capture_channel_count > 1 {
            Stereo
        } else {
            Mono
        };
        let stream = match config.sample_format() {
            I8 => {
                build_input_stream::<i8>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            I16 => {
                build_input_stream::<i16>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            I32 => {
                build_input_stream::<i32>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            I64 => {
                build_input_stream::<i64>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            U8 => {
                build_input_stream::<u8>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            U16 => {
                build_input_stream::<u16>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            U32 => {
                build_input_stream::<u32>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            U64 => {
                build_input_stream::<u64>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            F32 => {
                build_input_stream::<f32>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            F64 => {
                build_input_stream::<f64>(device, &config, sp, sample_rate, ch, selected_channels)?
            }
            f => bail!("unsupported audio format: {:?}", f),
        };
        stream.play()?;
        Ok((
            Box::new(stream),
            Arc::new(create_format_msg(sample_rate, ch as _)),
        ))
    }

    fn build_input_stream<T>(
        device: cpal::Device,
        config: &cpal::SupportedStreamConfig,
        sp: GenericService,
        sample_rate: u32,
        encode_channel: magnum_opus::Channels,
        selected_channels: Option<Vec<usize>>,
    ) -> ResultType<cpal::Stream>
    where
        T: cpal::SizedSample + dasp::sample::ToSample<f32>,
    {
        let err_fn = move |err| {
            // too many UnknownErrno, will improve later
            log::trace!("an error occurred on stream: {}", err);
        };
        let sample_rate_0 = config.sample_rate().0;
        log::debug!("Audio sample rate : {}", sample_rate);
        unsafe {
            AUDIO_ZERO_COUNT = 0;
        }
        let source_channel = config.channels();
        if let Some(channels) = selected_channels.as_ref() {
            if channels.is_empty()
                || channels
                    .iter()
                    .any(|channel| *channel >= source_channel as usize)
            {
                bail!("Invalid ASIO channel selection: {:?}", channels);
            }
        }
        let device_channel = selected_channels
            .as_ref()
            .map(|channels| channels.len() as u16)
            .unwrap_or(source_channel);
        let mut encoder = Encoder::new(sample_rate, encode_channel, LowDelay)?;
        // https://www.opus-codec.org/docs/html_api/group__opusencoder.html#gace941e4ef26ed844879fde342ffbe546
        // https://chromium.googlesource.com/chromium/deps/opus/+/1.1.1/include/opus.h
        // Do not set `frame_size = sample_rate as usize / 100;`
        // Because we find `sample_rate as usize / 100` will cause encoder error in `encoder.encode_vec_float()` sometimes.
        // https://github.com/xiph/opus/blob/2554a89e02c7fc30a980b4f7e635ceae1ecba5d6/src/opus_encoder.c#L725
        let frame_size = sample_rate_0 as usize / 100; // 10 ms
        let encode_len = frame_size * encode_channel as usize;
        let rechannel_len = encode_len * device_channel as usize / encode_channel as usize;
        INPUT_BUFFER.lock().unwrap().clear();
        let timeout = None;
        let stream_config = StreamConfig {
            channels: source_channel,
            sample_rate: config.sample_rate(),
            buffer_size: BufferSize::Default,
        };
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[T], _: &InputCallbackInfo| {
                let source_buffer: Vec<f32> = data.iter().map(|s| T::to_sample(*s)).collect();
                let buffer = if let Some(channels) = selected_channels.as_deref() {
                    super::select_interleaved_channels(
                        &source_buffer,
                        source_channel as usize,
                        channels,
                    )
                    .unwrap_or_default()
                } else {
                    source_buffer
                };
                let mut lock = INPUT_BUFFER.lock().unwrap();
                lock.extend(buffer);
                while lock.len() >= rechannel_len {
                    let frame: Vec<f32> = lock.drain(0..rechannel_len).collect();
                    send(
                        frame,
                        sample_rate_0,
                        sample_rate,
                        device_channel,
                        encode_channel as _,
                        &mut encoder,
                        &sp,
                    );
                }
            },
            err_fn,
            timeout,
        )?;
        Ok(stream)
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
                    Err(_) => {}
                }
            }
        } else {
            log::debug!("invalid audio data size:{} ", input_size);
            return;
        }
    }

    #[cfg(not(target_os = "android"))]
    match encoder.encode_vec_float(data, data.len() * 6) {
        Ok(data) => {
            let mut msg_out = Message::new();
            msg_out.set_audio_frame(AudioFrame {
                data: data.into(),
                ..Default::default()
            });
            sp.send(msg_out);
        }
        Err(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asio_input_choices_are_stereo_pairs_with_a_mono_tail() {
        let choices = asio_input_choices("Example Driver", 5);
        assert_eq!(choices.len(), 3);
        assert_eq!(
            choices[0],
            AsioInputChoice {
                display: "[ASIO] Example Driver - Inputs 1-2".to_owned(),
                channels: vec![0, 1],
            }
        );
        assert_eq!(
            choices[1],
            AsioInputChoice {
                display: "[ASIO] Example Driver - Inputs 3-4".to_owned(),
                channels: vec![2, 3],
            }
        );
        assert_eq!(
            choices[2],
            AsioInputChoice {
                display: "[ASIO] Example Driver - Input 5".to_owned(),
                channels: vec![4],
            }
        );
    }

    #[test]
    fn selects_an_asio_pair_from_interleaved_frames() {
        let source = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert_eq!(
            select_interleaved_channels(&source, 4, &[2, 3]),
            Some(vec![3.0, 4.0, 7.0, 8.0])
        );
    }

    #[test]
    fn rejects_invalid_asio_channel_selection() {
        assert_eq!(select_interleaved_channels(&[1.0, 2.0], 2, &[2]), None);
        assert_eq!(select_interleaved_channels(&[1.0, 2.0], 0, &[0]), None);
        assert_eq!(select_interleaved_channels(&[1.0, 2.0], 2, &[]), None);
    }
}
