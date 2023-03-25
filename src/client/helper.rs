use hbb_common::{
    get_time,
    message_proto::{video_frame, Message, VideoFrame, VoiceCallRequest, VoiceCallResponse},
};

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
