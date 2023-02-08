use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use hbb_common::{
    log,
    message_proto::{video_frame, VideoFrame, Message, VoiceCallRequest, VoiceCallResponse}, get_time,
};

const MAX_LATENCY: i64 = 500;
const MIN_LATENCY: i64 = 100;

/// Latency controller for syncing audio with the video stream.
/// Only sync the audio to video, not the other way around.
#[derive(Debug)]
pub struct LatencyController {
    last_video_remote_ts: i64, // generated on remote device
    update_time: Instant,
    allow_audio: bool,
    audio_only: bool
}

impl Default for LatencyController {
    fn default() -> Self {
        Self {
            last_video_remote_ts: Default::default(),
            update_time: Instant::now(),
            allow_audio: Default::default(),
            audio_only: false
        }
    }
}

impl LatencyController {
    /// Create a new latency controller.
    pub fn new() -> Arc<Mutex<LatencyController>> {
        Arc::new(Mutex::new(LatencyController::default()))
    }

    /// Set whether this [LatencyController] should be working in audio only mode.
    pub fn set_audio_only(&mut self, only: bool) {
        self.audio_only = only;
    }

    /// Update the latency controller with the latest video timestamp.
    pub fn update_video(&mut self, timestamp: i64) {
        self.last_video_remote_ts = timestamp;
        self.update_time = Instant::now();
    }

    /// Check if the audio should be played based on the current latency.
    pub fn check_audio(&mut self, timestamp: i64) -> bool {
        // Compute audio latency.
        let expected = self.update_time.elapsed().as_millis() as i64 + self.last_video_remote_ts;
        let latency = if self.audio_only {
            expected
        } else {
            expected - timestamp
        };
        // Set MAX and MIN, avoid fixing too frequently.
        if self.allow_audio {
            if latency.abs() > MAX_LATENCY {
                log::debug!("LATENCY > {}ms cut off, latency:{}", MAX_LATENCY, latency);
                self.allow_audio = false;
            }
        } else {
            if latency.abs() < MIN_LATENCY {
                log::debug!("LATENCY < {}ms resume, latency:{}", MIN_LATENCY, latency);
                self.allow_audio = true;
            }
        }
        // No video frame here, which means the update time is not up to date.
        // We manually update the time here.
        self.update_time = Instant::now();
        self.allow_audio
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum CodecFormat {
    VP9,
    H264,
    H265,
    Unknown,
}

impl From<&VideoFrame> for CodecFormat {
    fn from(it: &VideoFrame) -> Self {
        match it.union {
            Some(video_frame::Union::Vp9s(_)) => CodecFormat::VP9,
            Some(video_frame::Union::H264s(_)) => CodecFormat::H264,
            Some(video_frame::Union::H265s(_)) => CodecFormat::H265,
            _ => CodecFormat::Unknown,
        }
    }
}

impl ToString for CodecFormat {
    fn to_string(&self) -> String {
        match self {
            CodecFormat::VP9 => "VP9".into(),
            CodecFormat::H264 => "H264".into(),
            CodecFormat::H265 => "H265".into(),
            CodecFormat::Unknown => "Unknow".into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct QualityStatus {
    pub speed: Option<String>,
    pub fps: Option<i32>,
    pub delay: Option<i32>,
    pub target_bitrate: Option<i32>,
    pub codec_format: Option<CodecFormat>,
}

#[inline]
pub fn new_voice_call_request(is_connect: bool) -> Message {
    let mut req = VoiceCallRequest::new();
    req.is_connect = is_connect;
    req.req_timestamp = get_time();
    let mut msg = Message::new();
    msg.set_voice_call_request(req);
    msg
}

#[inline]
pub fn new_voice_call_response(request_timestamp: i64, accepted: bool) -> Message {
    let mut resp = VoiceCallResponse::new();
    resp.accepted = accepted;
    resp.req_timestamp = request_timestamp;
    resp.ack_timestamp = get_time();
    let mut msg = Message::new();
    msg.set_voice_call_response(resp);
    msg
}