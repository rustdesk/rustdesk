use super::*;
pub use crate::clipboard::{
    check_clipboard, ClipboardContext, ClipboardSide, CLIPBOARD_INTERVAL as INTERVAL,
    CLIPBOARD_NAME as NAME,
};
#[cfg(windows)]
use crate::ipc::{ClipboardFile, ClipboardNonFile, Data};
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
            match Self::read_clipboard_from_cm_ipc() {
                Err(e) => {
                    log::error!("Failed to read clipboard from cm: {}", e);
                }
                Ok(data) => {
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
        check_clipboard(&mut self.ctx, ClipboardSide::Host, false)
    }

    #[inline]
    #[cfg(windows)]
    #[tokio::main(flavor = "current_thread")]
    async fn read_clipboard_from_cm_ipc() -> ResultType<Vec<ClipboardNonFile>> {
        // It's ok to use 1000ms timeout here, because
        // 1. the clipboard is not used frequently.
        // 2. the clipboard handle is sync and will not block the main thread.
        let mut stream = crate::ipc::connect(100, "_cm").await?;
        timeout(100, stream.send(&Data::ClipboardNonFile(None))).await??;
        loop {
            match stream.next_timeout(800).await? {
                Some(Data::ClipboardNonFile(Some((err, contents)))) => {
                    if !err.is_empty() {
                        bail!("{}", err);
                    } else {
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
}
