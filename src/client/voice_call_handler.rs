// Voice Call Handler Integration
// File: src/client/voice_call_handler.rs
// Handles voice call session management and message routing

use hbb_common::{
    anyhow::{anyhow, Result as ResultType},
    log,
};
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(feature = "voice-call")]
use crate::audio::{
    AudioCapture, AudioCodec, AudioNetwork, JitterBuffer, AudioPlayback, VoiceCallConfig,
    VoiceCallState, VoiceCallStats,
};

/// Voice call channel types
#[derive(Debug, Clone)]
pub enum VoiceCallMessage {
    #[cfg(feature = "voice-call")]
    StartCall { peer_id: String },
    #[cfg(feature = "voice-call")]
    StopCall { peer_id: String },
    #[cfg(feature = "voice-call")]
    AudioFrame { data: Vec<u8>, sequence: u32 },
    #[cfg(feature = "voice-call")]
    MuteToggle { is_muted: bool },
    #[cfg(feature = "voice-call")]
    Error { code: u32, message: String },
}

/// Voice call session handler
#[cfg(feature = "voice-call")]
pub struct VoiceCallHandler {
    peer_id: String,
    state: VoiceCallState,
    config: VoiceCallConfig,
    codec: Arc<AudioCodec>,
    jitter_buffer: Arc<JitterBuffer>,
    network: Arc<AudioNetwork>,
    stats: Arc<tokio::sync::Mutex<VoiceCallStats>>,
    tx: mpsc::UnboundedSender<VoiceCallMessage>,
    rx: tokio::sync::Mutex<mpsc::UnboundedReceiver<VoiceCallMessage>>,
}

#[cfg(feature = "voice-call")]
impl VoiceCallHandler {
    pub fn new(
        peer_id: String,
        config: VoiceCallConfig,
    ) -> ResultType<(Self, mpsc::UnboundedSender<VoiceCallMessage>)> {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();

        let codec = Arc::new(AudioCodec::new(&config.codec)?);
        let jitter_buffer = Arc::new(JitterBuffer::new(&config.jitter_buffer));
        let network = Arc::new(AudioNetwork::new());
        let stats = Arc::new(tokio::sync::Mutex::new(VoiceCallStats::new()));

        let handler = Self {
            peer_id: peer_id.clone(),
            state: VoiceCallState::Idle,
            config,
            codec,
            jitter_buffer,
            network,
            stats,
            tx,
            rx: tokio::sync::Mutex::new(rx),
        };

        log::info!("Voice call handler created for peer: {}", peer_id);
        Ok((handler, tx_clone))
    }

    pub fn state(&self) -> VoiceCallState {
        self.state
    }

    pub fn set_state(&mut self, state: VoiceCallState) {
        self.state = state;
        log::debug!("Voice call state changed to: {}", state);
    }

    pub async fn start(&mut self) -> ResultType<()> {
        log::info!("Starting voice call with peer: {}", self.peer_id);
        self.set_state(VoiceCallState::Active);

        let mut stats = self.stats.lock().await;
        stats.update_state(VoiceCallState::Active);
        drop(stats);

        Ok(())
    }

    pub async fn stop(&mut self) -> ResultType<()> {
        log::info!("Stopping voice call with peer: {}", self.peer_id);
        self.set_state(VoiceCallState::Ending);
        Ok(())
    }

    pub async fn handle_audio_frame(&mut self, data: Vec<u8>) -> ResultType<()> {
        if self.state != VoiceCallState::Active {
            return Err(anyhow!("Call not active"));
        }

        // Decode Opus data
        let pcm = self.codec.decode(&data)?;

        // Insert into jitter buffer
        let frame = crate::audio::jitter_buffer::JitterBufferFrame {
            data: pcm,
            sequence_number: 0,
            timestamp_ms: 0,
            sample_rate: self.config.codec.sample_rate,
        };

        self.jitter_buffer.insert(frame)?;

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.bytes_received += data.len() as u64;
        stats.packets_received += 1;

        Ok(())
    }

    pub async fn encode_and_send(&self, pcm_data: &[f32]) -> ResultType<Vec<u8>> {
        // Encode to Opus
        let opus_data = self.codec.encode(pcm_data)?;

        // Create network frame
        let frame = self.network.create_frame(
            &opus_data,
            self.config.codec.sample_rate,
            self.config.codec.channels,
        );

        // Update stats
        let mut stats = self.stats.lock().await;
        stats.bytes_sent += opus_data.len() as u64;
        stats.packets_sent += 1;

        Ok(opus_data)
    }

    pub async fn get_stats(&self) -> VoiceCallStats {
        self.stats.lock().await.clone()
    }

    pub async fn process_message(&mut self, msg: VoiceCallMessage) -> ResultType<()> {
        match msg {
            VoiceCallMessage::StartCall { peer_id } => {
                log::info!("Starting call with: {}", peer_id);
                self.start().await?;
            }
            VoiceCallMessage::StopCall { peer_id } => {
                log::info!("Stopping call with: {}", peer_id);
                self.stop().await?;
            }
            VoiceCallMessage::AudioFrame { data, sequence } => {
                log::debug!("Received audio frame, sequence: {}", sequence);
                self.handle_audio_frame(data).await?;
            }
            VoiceCallMessage::MuteToggle { is_muted } => {
                let new_state = if is_muted {
                    VoiceCallState::Muted
                } else {
                    VoiceCallState::Active
                };
                self.set_state(new_state);
                log::info!("Mute toggled: {}", is_muted);
            }
            VoiceCallMessage::Error { code, message } => {
                log::error!("Voice call error [{}]: {}", code, message);
                self.set_state(VoiceCallState::Error);
            }
        }
        Ok(())
    }

    pub async fn run(&mut self) -> ResultType<()> {
        log::info!("Voice call handler running for: {}", self.peer_id);

        while let Some(msg) = self.rx.lock().await.recv().await {
            if let Err(e) = self.process_message(msg).await {
                log::error!("Error processing voice call message: {}", e);
                self.set_state(VoiceCallState::Error);
                break;
            }

            if self.state == VoiceCallState::Ending {
                break;
            }
        }

        log::info!("Voice call handler stopped");
        Ok(())
    }
}

// Non-voice-call stub
#[cfg(not(feature = "voice-call"))]
pub struct VoiceCallHandler;

#[cfg(not(feature = "voice-call"))]
impl VoiceCallHandler {
    pub fn new(_peer_id: String, _config: VoiceCallConfig) -> ResultType<(Self, String)> {
        Err(anyhow!("Voice call feature not enabled"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "voice-call")]
    #[tokio::test]
    async fn test_voice_call_handler_creation() {
        let config = VoiceCallConfig::default();
        let (handler, _tx) = VoiceCallHandler::new("peer123".to_string(), config);
        assert!(handler.is_ok());
    }

    #[cfg(feature = "voice-call")]
    #[tokio::test]
    async fn test_voice_call_state_transitions() {
        let config = VoiceCallConfig::default();
        let (mut handler, _tx) = VoiceCallHandler::new("peer123".to_string(), config).unwrap();

        assert_eq!(handler.state(), VoiceCallState::Idle);
        handler.start().await.unwrap();
        assert_eq!(handler.state(), VoiceCallState::Active);
        handler.stop().await.unwrap();
        assert_eq!(handler.state(), VoiceCallState::Ending);
    }

    #[cfg(feature = "voice-call")]
    #[tokio::test]
    async fn test_voice_call_stats() {
        let config = VoiceCallConfig::default();
        let (handler, _tx) = VoiceCallHandler::new("peer123".to_string(), config).unwrap();
        let stats = handler.get_stats().await;
        assert_eq!(stats.packets_sent, 0);
        assert_eq!(stats.packets_received, 0);
    }
}
