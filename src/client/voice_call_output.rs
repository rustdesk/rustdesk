//! Explicit host-side voice-call routing, including virtual microphone cables.
use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct VoiceAudioStop(Arc<AtomicBool>);

impl Drop for VoiceAudioStop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub(super) fn output_device(name: &str) -> ResultType<cpal::Device> {
    let devices = AUDIO_HOST.output_devices()?.collect::<Vec<_>>();
    let names = devices
        .iter()
        .map(|device| device.name().unwrap_or_default())
        .collect::<Vec<_>>();
    let index =
        voice_call_output_name::unique_named_index(&names, name).map_err(|error| anyhow!(error))?;
    devices
        .into_iter()
        .nth(index)
        .with_context(|| "Voice-call output device disappeared")
}

pub(crate) fn start_host_voice_audio(voice_calling: bool) -> (MediaSender, Option<VoiceAudioStop>) {
    let name =
        hbb_common::config::Config::get_option(base::config::keys::OPTION_VOICE_CALL_OUTPUT_DEVICE);
    if !voice_calling || name.is_empty() {
        return (start_audio_thread(), None);
    }
    let (sender, receiver) = mpsc::channel::<MediaData>();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    std::thread::spawn(move || {
        let mut handler = AudioHandler {
            voice_call_output_device: Some(name),
            ..Default::default()
        };
        while let Ok(data) = handler.receive_audio(&receiver) {
            if worker_stop.load(Ordering::Acquire) {
                break;
            }
            match data {
                MediaData::AudioFrame(frame) => handler.handle_frame(*frame),
                MediaData::AudioFormat(format) => handler.handle_format(format),
                _ => {}
            }
        }
    });
    (sender, Some(VoiceAudioStop(stop)))
}
