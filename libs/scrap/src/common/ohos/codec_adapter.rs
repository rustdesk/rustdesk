use super::{
    avcodec::{OhosVideoDecoder, OhosVideoEncoderConfig},
    direct_render::current_direct_render_target,
};
use crate::{
    codec::{Encoder, EncodingUpdate},
    common::GoogleImage,
    CodecFormat, ImageRgb,
};
use hbb_common::{anyhow::anyhow, log, message_proto::EncodedVideoFrames, ResultType};

pub fn encoder_config(
    format: CodecFormat,
    width: u32,
    height: u32,
    quality: f32,
    keyframe_interval: Option<usize>,
) -> Option<OhosVideoEncoderConfig> {
    matches!(format, CodecFormat::H264 | CodecFormat::H265).then_some(OhosVideoEncoderConfig {
        format,
        width,
        height,
        quality,
        keyframe_interval,
    })
}

pub fn retry_encoder_setup<T>(mut setup: impl FnMut() -> ResultType<T>) -> ResultType<T> {
    let mut last_error = None;
    // A failed H.265 initialization quarantines only H.265 and recomputes
    // negotiation, so the next attempt is H.264 before the final VP9 fallback.
    for attempt in 1..=3 {
        match setup() {
            Ok(result) => return Ok(result),
            Err(error) => {
                log::error!("Failed to create OHOS encoder on attempt {attempt}: {error:?}");
                Encoder::update(EncodingUpdate::Check);
                last_error = Some(error);
            }
        }
    }
    Err(last_error.expect("OHOS encoder setup attempted"))
}

#[derive(Default)]
pub(crate) struct OhosDecoderHolder {
    format: Option<CodecFormat>,
    decoder: Option<OhosVideoDecoder>,
    init_error: Option<String>,
}

impl OhosDecoderHolder {
    pub(crate) fn new(format: CodecFormat) -> Self {
        if !matches!(format, CodecFormat::H264 | CodecFormat::H265) {
            return Self::default();
        }

        let mut holder = Self {
            format: Some(format.clone()),
            ..Default::default()
        };
        let target = current_direct_render_target();
        log::info!(
            "OHOS decoder target, format: {format:?}, decode_size: {:?}, surface_id: {:?}",
            target.decode_size.unwrap_or((64, 64)),
            target.surface_id,
        );
        match OhosVideoDecoder::new(format.clone(), target) {
            Ok(decoder) => holder.decoder = Some(decoder),
            Err(error) => {
                log::error!("create {format:?} OHOS decoder failed: {error}");
                holder.init_error = Some(error.to_string());
            }
        }
        holder
    }

    pub(crate) fn valid(&self) -> bool {
        self.decoder.is_some()
    }

    pub(crate) fn handle_video_frame(
        &mut self,
        format: CodecFormat,
        frames: &EncodedVideoFrames,
        rgb: &mut ImageRgb,
        pixelbuffer: &mut bool,
    ) -> ResultType<bool> {
        let format_name = match format {
            CodecFormat::H264 => "h264",
            CodecFormat::H265 => "h265",
            _ => "OHOS decoder format",
        };
        if self.format.as_ref() != Some(&format) {
            return Err(anyhow!("don't support {format_name}").into());
        }
        let init_error = self
            .init_error
            .as_deref()
            .unwrap_or("decoder was not initialized");
        let decoder = self
            .decoder
            .as_mut()
            .ok_or_else(|| anyhow!("don't support {}: {}", format_name, init_error,))?;

        if decoder.is_surface_mode() {
            *pixelbuffer = false;
            for frame in frames.frames.iter() {
                decoder.submit_to_surface(&frame.data, frame.key)?;
            }
            return decoder.take_surface_rendered_frame();
        }

        let mut last_frame = None;
        for frame in frames.frames.iter() {
            for image in decoder.decode(&frame.data, frame.key)? {
                last_frame = Some(image);
            }
        }
        if let Some(last_frame) = last_frame {
            *pixelbuffer = true;
            last_frame.to(rgb);
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
