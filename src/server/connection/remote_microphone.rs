use super::*;

impl Connection {
    pub(super) async fn handle_remote_microphone(&mut self, mic: RemoteMicrophone) -> bool {
        if !self.authorized || !self.is_remote() {
            return true;
        }
        let mut response = RemoteMicrophone {
            epoch: mic.epoch,
            ..Default::default()
        };
        #[cfg(all(target_os = "linux", feature = "remote-microphone"))]
        {
            match mic.action.as_str() {
                "query" => response.action = "available".into(),
                "start" | "resume" if !self.voice_calling => {
                    if self
                        .remote_microphone
                        .as_ref()
                        .map(|receiver| !receiver.healthy())
                        .unwrap_or(false)
                    {
                        self.remote_microphone.take();
                    }
                    if self.remote_microphone.is_none() {
                        let peer_id = self.lr.my_id.clone();
                        match tokio::task::spawn_blocking(move || {
                            crate::remote_microphone::Receiver::acquire(&peer_id)
                        })
                        .await
                        {
                            Ok(Ok(receiver)) => self.remote_microphone = Some(receiver),
                            Ok(Err(error)) => {
                                response.error =
                                    format!("Cannot create virtual microphone: {error:#}")
                            }
                            Err(error) => response.error = error.to_string(),
                        }
                    }
                    if let Some(receiver) = &self.remote_microphone {
                        match receiver.activate(mic.epoch) {
                            Ok(()) => response.action = "active".into(),
                            Err(error) => {
                                response.action = "error".into();
                                response.error = error.to_string();
                            }
                        }
                    } else {
                        response.action = "error".into();
                    }
                }
                "mute" => {
                    if let Some(receiver) = &self.remote_microphone {
                        receiver.silence();
                    }
                    response.action = "silenced".into();
                }
                "stop" => {
                    self.remote_microphone.take();
                    response.action = "stopped".into();
                }
                "frame" => {
                    if let Some(receiver) = &self.remote_microphone {
                        if receiver.healthy() {
                            receiver.push(mic.epoch, &mic.data);
                            return true;
                        }
                        response.action = "error".into();
                        response.error = "PipeWire microphone disconnected".into();
                        self.remote_microphone.take();
                    } else {
                        return true;
                    }
                }
                _ => {
                    response.action = "error".into();
                    response.error = "Stop the voice call first".into();
                }
            }
        }
        #[cfg(not(all(target_os = "linux", feature = "remote-microphone")))]
        {
            response.action = "unavailable".into();
        }
        let mut msg = Message::new();
        msg.set_remote_microphone(response);
        self.send(msg).await;

        true
    }
}
