//! Opus codec wrapper for audio compression/decompression.
//!
//! Provides efficient bidirectional audio encoding using the Opus codec,
//! optimized for real-time applications with adaptive bitrate capabilities.

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
};
use magnum_opus::{Application::Voip, Channels::*, Decoder as OpusDecoder, Encoder as OpusEncoder};
use std::sync::Arc;

/// Configuration for audio codec
#[derive(Debug, Clone)]
pub struct CodecConfig {
    /// Sample rate: 8000, 12000, 16000, 24000, or 48000 Hz
    pub sample_rate: u32,
    /// Number of channels: Mono (1) or Stereo (2)
    pub channels: u16,
    /// Bitrate in bits per second (8000-128000)
    pub bitrate: u32,
    /// Use variable bitrate encoding (more efficient)
    pub vbr: bool,
    /// Frame size in milliseconds: 2.5, 5, 10, 20, 40, 60
    pub frame_duration_ms: u32,
}

impl Default for CodecConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bitrate: 32000,
            vbr: true,
            frame_duration_ms: 20,
        }
    }
}

impl CodecConfig {
    /// Calculate samples per frame based on sample rate and frame duration
    pub fn samples_per_frame(&self) -> usize {
        (self.sample_rate as usize * self.frame_duration_ms as usize) / 1000
    }

    /// Validate configuration
    pub fn is_valid(&self) -> bool {
        matches!(
            self.sample_rate,
            8000 | 12000 | 16000 | 24000 | 48000
        ) && matches!(self.channels, 1 | 2)
            && self.bitrate >= 8000
            && self.bitrate <= 128000
            && matches!(
                self.frame_duration_ms,
                2 | 5 | 10 | 20 | 40 | 60
            )
    }

    /// Create config optimized for VoIP (low bandwidth)
    pub fn voip() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bitrate: 32000,
            vbr: true,
            frame_duration_ms: 20,
        }
    }

    /// Create config for high-quality communication
    pub fn quality() -> Self {
        Self {
            sample_rate: 48000,
            channels: 2,
            bitrate: 128000,
            vbr: true,
            frame_duration_ms: 20,
        }
    }
}

/// Opus audio codec wrapper
pub struct AudioCodec {
    encoder: Arc<OpusEncoder>,
    decoder: Arc<OpusDecoder>,
    config: CodecConfig,
    encode_buffer: Vec<u8>,
    decode_buffer: Vec<f32>,
}

impl AudioCodec {
    /// Create a new audio codec with the given configuration
    pub fn new(config: CodecConfig) -> ResultType<Self> {
        if !config.is_valid() {
            return Err(anyhow!(
                "Invalid codec config: sample_rate={}, channels={}",
                config.sample_rate,
                config.channels
            ));
        }

        let channels = if config.channels == 2 { Stereo } else { Mono };

        let encoder = OpusEncoder::new(config.sample_rate, channels, Voip)
            .map_err(|e| anyhow!("Failed to create Opus encoder: {}", e))?;

        encoder
            .set_bitrate(magnum_opus::Bitrate::Bits(config.bitrate as i32))
            .map_err(|e| anyhow!("Failed to set bitrate: {}", e))?;

        if config.vbr {
            encoder
                .set_vbr(true)
                .map_err(|e| anyhow!("Failed to enable VBR: {}", e))?;
        }

        let decoder = OpusDecoder::new(config.sample_rate, channels)
            .map_err(|e| anyhow!("Failed to create Opus decoder: {}", e))?;

        // Pre-allocate buffers
        let samples_per_frame = config.samples_per_frame();
        let encode_buffer = vec![0u8; 4000]; // Max opus frame size
        let decode_buffer = vec![0f32; samples_per_frame * config.channels as usize];

        log::info!(
            "AudioCodec created: sample_rate={}, channels={}, bitrate={}",
            config.sample_rate,
            config.channels,
            config.bitrate
        );

        Ok(Self {
            encoder: Arc::new(encoder),
            decoder: Arc::new(decoder),
            config,
            encode_buffer,
            decode_buffer,
        })
    }

    /// Encode PCM audio to Opus
    ///
    /// # Arguments
    ///
    /// * `pcm` - PCM samples (f32, normalized to [-1.0, 1.0])
    pub fn encode(&self, pcm: &[f32]) -> ResultType<Vec<u8>> {
        let samples_per_frame = self.config.samples_per_frame();

        if pcm.len() != samples_per_frame * self.config.channels as usize {
            return Err(anyhow!(
                "Invalid PCM length: expected {}, got {}",
                samples_per_frame * self.config.channels as usize,
                pcm.len()
            ));
        }

        match self.encoder.encode_float(pcm, &mut self.encode_buffer) {
            Ok(len) => {
                let encoded = self.encode_buffer[..len].to_vec();
                Ok(encoded)
            }
            Err(e) => Err(anyhow!("Opus encoding failed: {}", e)),
        }
    }

    /// Decode Opus to PCM audio
    ///
    /// # Arguments
    ///
    /// * `opus_data` - Opus-encoded audio frame
    pub fn decode(&self, opus_data: &[u8]) -> ResultType<Vec<f32>> {
        match self.decoder.decode_float(
            opus_data,
            &mut self.decode_buffer.clone(),
            false,
        ) {
            Ok(samples_decoded) => {
                let len = samples_decoded as usize * self.config.channels as usize;
                Ok(self.decode_buffer[..len].to_vec())
            }
            Err(e) => Err(anyhow!("Opus decoding failed: {}", e)),
        }
    }

    /// Update the encoder bitrate (for adaptive bitrate)
    pub fn set_bitrate(&self, bitrate: u32) -> ResultType<()> {
        if bitrate < 8000 || bitrate > 128000 {
            return Err(anyhow!("Bitrate out of range: {} (8000-128000)", bitrate));
        }

        self.encoder
            .set_bitrate(magnum_opus::Bitrate::Bits(bitrate as i32))
            .map_err(|e| anyhow!("Failed to set bitrate: {}", e))?;

        log::debug!("Encoder bitrate updated to {} bps", bitrate);
        Ok(())
    }

    /// Get current codec configuration
    pub fn config(&self) -> &CodecConfig {
        &self.config
    }

    /// Get samples per frame based on current configuration
    pub fn samples_per_frame(&self) -> usize {
        self.config.samples_per_frame()
    }
}

impl Clone for AudioCodec {
    fn clone(&self) -> Self {
        Self {
            encoder: self.encoder.clone(),
            decoder: self.decoder.clone(),
            config: self.config.clone(),
            encode_buffer: vec![0u8; 4000],
            decode_buffer: vec![0f32; self.decode_buffer.len()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codec_config_is_valid() {
        let config = CodecConfig::default();
        assert!(config.is_valid());
    }

    #[test]
    fn test_codec_config_invalid_sample_rate() {
        let config = CodecConfig {
            sample_rate: 44100,
            ..Default::default()
        };
        assert!(!config.is_valid());
    }

    #[test]
    fn test_samples_per_frame() {
        let config = CodecConfig {
            sample_rate: 16000,
            frame_duration_ms: 20,
            ..Default::default()
        };
        assert_eq!(config.samples_per_frame(), 320);
    }

    #[test]
    fn test_codec_creation() {
        let codec = AudioCodec::new(CodecConfig::default());
        assert!(codec.is_ok());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let codec = AudioCodec::new(CodecConfig::voip()).unwrap();

        // Create test PCM data (silence)
        let samples_per_frame = codec.samples_per_frame();
        let pcm = vec![0.0; samples_per_frame];

        // Encode
        let encoded = codec.encode(&pcm).unwrap();
        assert!(!encoded.is_empty());

        // Decode
        let decoded = codec.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), samples_per_frame);
    }

    #[test]
    fn test_set_bitrate() {
        let codec = AudioCodec::new(CodecConfig::default()).unwrap();

        // Valid bitrates
        assert!(codec.set_bitrate(32000).is_ok());
        assert!(codec.set_bitrate(64000).is_ok());

        // Invalid bitrates
        assert!(codec.set_bitrate(4000).is_err());
        assert!(codec.set_bitrate(256000).is_err());
    }

    #[test]
    fn test_codec_config_voip() {
        let config = CodecConfig::voip();
        assert!(config.is_valid());
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.channels, 1);
    }

    #[test]
    fn test_codec_config_quality() {
        let config = CodecConfig::quality();
        assert!(config.is_valid());
        assert_eq!(config.sample_rate, 48000);
        assert_eq!(config.channels, 2);
    }
}
