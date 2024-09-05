use super::*;
pub use crate::clipboard::{
    check_clipboard, ClipboardContext, ClipboardSide, CLIPBOARD_INTERVAL as INTERVAL,
    CLIPBOARD_NAME as NAME,
};
#[cfg(windows)]
use crate::ipc::{self, ClipboardFile, ClipboardNonFile, Data};
use clipboard_master::{CallbackResult, ClipboardHandler};
use std::{
    io,
    sync::mpsc::{channel, RecvTimeoutError, Sender},
    time::Duration,
};

struct Handler {
    sp: EmptyExtraFieldService,
    ctx: Option<ClipboardContext>,
    tx_cb_result: Sender<CallbackResult>,
    #[cfg(target_os = "windows")]
    stream: Option<ipc::ConnectionTmpl<parity_tokio_ipc::ConnectionClient>>,
}

pub fn new() -> GenericService {
    let svc = EmptyExtraFieldService::new(NAME.to_owned(), true);
    GenericService::run(&svc.clone(), run);
    svc.sp
}

fn run(sp: EmptyExtraFieldService) -> ResultType<()> {
    let (tx_cb_result, rx_cb_result) = channel();
    let handler = Handler {
        sp: sp.clone(),
        ctx: Some(ClipboardContext::new()?),
        tx_cb_result,
        #[cfg(target_os = "windows")]
        stream: None,
    };

    let (tx_start_res, rx_start_res) = channel();
    let h = crate::clipboard::start_clipbard_master_thread(handler, tx_start_res);
    let shutdown = match rx_start_res.recv() {
        Ok((Some(s), _)) => s,
        Ok((None, err)) => {
            bail!(err);
        }
        Err(e) => {
            bail!("Failed to create clipboard listener: {}", e);
        }
    };

    while sp.ok() {
        match rx_cb_result.recv_timeout(Duration::from_millis(INTERVAL)) {
            Ok(CallbackResult::Stop) => {
                log::debug!("Clipboard listener stopped");
                break;
            }
            Ok(CallbackResult::StopWithError(err)) => {
                bail!("Clipboard listener stopped with error: {}", err);
            }
            Err(RecvTimeoutError::Timeout) => {}
            _ => {}
        }
    }
    shutdown.signal();
    h.join().ok();

    Ok(())
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        self.sp.snapshot(|_sps| Ok(())).ok();
        if self.sp.ok() {
            if let Some(msg) = self.get_clipboard_msg() {
                self.sp.send(msg);
            }
        }
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: io::Error) -> CallbackResult {
        self.tx_cb_result
            .send(CallbackResult::StopWithError(error))
            .ok();
        CallbackResult::Next
    }
}

impl Handler {
    fn get_clipboard_msg(&mut self) -> Option<Message> {
        #[cfg(target_os = "windows")]
        if crate::common::is_server() && crate::platform::is_root() {
            match self.read_clipboard_from_cm_ipc() {
                Err(e) => {
                    log::error!("Failed to read clipboard from cm: {}", e);
                }
                Ok(data) => {
                    // Skip sending empty clipboard data.
                    // Maybe there's something wrong reading the clipboard data in cm, but no error msg is returned.
                    // The clipboard data should not be empty, the last line will try again to get the clipboard data.
                    if !data.is_empty() {
                        let mut msg = Message::new();
                        let multi_clipboards = MultiClipboards {
                            clipboards: data
                                .into_iter()
                                .map(|c| Clipboard {
                                    compress: c.compress,
                                    content: c.content,
                                    width: c.width,
                                    height: c.height,
                                    format: ClipboardFormat::from_i32(c.format)
                                        .unwrap_or(ClipboardFormat::Text)
                                        .into(),
                                    ..Default::default()
                                })
                                .collect(),
                            ..Default::default()
                        };
                        msg.set_multi_clipboards(multi_clipboards);
                        return Some(msg);
                    }
                }
            }
        }
        check_clipboard(&mut self.ctx, ClipboardSide::Host, false)
    }

    // It's ok to do async operation in the clipboard service because:
    // 1. the clipboard is not used frequently.
    // 2. the clipboard handle is sync and will not block the main thread.
    #[cfg(windows)]
    #[tokio::main(flavor = "current_thread")]
    async fn read_clipboard_from_cm_ipc(&mut self) -> ResultType<Vec<ClipboardNonFile>> {
        let mut is_sent = false;
        if let Some(stream) = &mut self.stream {
            // If previous stream is still alive, reuse it.
            // If the previous stream is dead, `is_sent` will trigger reconnect.
            is_sent = stream.send(&Data::ClipboardNonFile(None)).await.is_ok();
        }
        if !is_sent {
            let mut stream = crate::ipc::connect(100, "_cm").await?;
            stream.send(&Data::ClipboardNonFile(None)).await?;
            self.stream = Some(stream);
        }

        if let Some(stream) = &mut self.stream {
            loop {
                match stream.next_timeout(800).await? {
                    Some(Data::ClipboardNonFile(Some((err, mut contents)))) => {
                        if !err.is_empty() {
                            bail!("{}", err);
                        } else {
                            if contents.iter().any(|c| c.next_raw) {
                                match timeout(1000, stream.next_raw()).await {
                                    Ok(Ok(mut data)) => {
                                        for c in &mut contents {
                                            if c.next_raw {
                                                // No need to check the length because sum(content_len) == data.len().
                                                c.content = data.split_to(c.content_len).into();
                                            }
                                        }
                                    }
                                    Ok(Err(e)) => {
                                        // reset by peer
                                        self.stream = None;
                                        bail!("failed to get raw clipboard data: {}", e);
                                    }
                                    Err(e) => {
                                        // Reconnect to avoid the next raw data remaining in the buffer.
                                        self.stream = None;
                                        log::debug!("failed to get raw clipboard data: {}", e);
                                    }
                                }
                            }
                            return Ok(contents);
                        }
                    }
                    Some(Data::ClipboardFile(ClipboardFile::MonitorReady)) => {
                        // ClipboardFile::MonitorReady is the first message sent by cm.
                    }
                    _ => {
                        bail!("failed to get clipboard data from cm");
                    }
                }
            }
        }
        // unreachable!
        bail!("failed to get clipboard data from cm");
    }
}
