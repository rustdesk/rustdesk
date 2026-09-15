use super::VRamEncoder;
use hbb_common::{anyhow::anyhow, ResultType};
use hwcodec::vram::encode::EncodeFrame;

impl VRamEncoder {
    pub(super) fn encode_repeat(&mut self, ms: i64) -> ResultType<Vec<EncodeFrame>> {
        match self.encoder.encode_repeat(ms) {
            Ok(frames) => Ok(std::mem::take(frames)),
            Err(code) => Err(anyhow!("VRAM repeat encode failed: {}", code)),
        }
    }
}

#[cfg(test)]
mod tests;
