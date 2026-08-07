use std::{future::Future, io, pin::Pin};

use super::TransferSignal;
use hbb_common::tokio;

trait SignalSource: Send + 'static {
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Option<TransferSignal>> + Send + '_>>;
}

struct TokioSignalSource {
    interrupt: tokio::signal::unix::Signal,
    terminate: tokio::signal::unix::Signal,
}

impl TokioSignalSource {
    fn new() -> io::Result<Self> {
        use tokio::signal::unix::{signal, SignalKind};

        Ok(Self {
            interrupt: signal(SignalKind::interrupt())?,
            terminate: signal(SignalKind::terminate())?,
        })
    }
}

impl SignalSource for TokioSignalSource {
    fn recv(&mut self) -> Pin<Box<dyn Future<Output = Option<TransferSignal>> + Send + '_>> {
        Box::pin(async {
            tokio::select! {
                result = self.interrupt.recv() => result.map(|()| TransferSignal::Interrupt),
                result = self.terminate.recv() => result.map(|()| TransferSignal::Terminate),
            }
        })
    }
}

pub(crate) struct SignalForwarder {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn spawn_signal_forwarder_with_factory<S, F, G>(
    source_factory: G,
    emit: F,
) -> io::Result<SignalForwarder>
where
    S: SignalSource,
    F: FnMut(TransferSignal) + Send + 'static,
    G: FnOnce() -> io::Result<S>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let runtime_guard = runtime.enter();
    let mut source = source_factory()?;
    drop(runtime_guard);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let thread = std::thread::spawn(move || {
        runtime.block_on(async move {
            let mut emit = emit;
            tokio::select! {
                biased;
                _ = shutdown_rx => {}
                signal = source.recv() => {
                    if let Some(signal) = signal {
                        emit(signal);
                    }
                }
            }
        });
    });
    Ok(SignalForwarder {
        shutdown_tx: Some(shutdown_tx),
        thread: Some(thread),
    })
}

pub(crate) fn spawn_signal_forwarder<F>(emit: F) -> io::Result<SignalForwarder>
where
    F: FnMut(TransferSignal) + Send + 'static,
{
    spawn_signal_forwarder_with_factory(TokioSignalSource::new, emit)
}

impl Drop for SignalForwarder {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("RDH headless file-transfer signal thread panicked");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{mpsc, Arc, Mutex},
        time::Duration,
    };

    use hbb_common::tokio;

    use super::*;

    struct FakeSignalSource {
        event_rx: tokio::sync::mpsc::UnboundedReceiver<TransferSignal>,
    }

    impl SignalSource for FakeSignalSource {
        fn recv(&mut self) -> Pin<Box<dyn Future<Output = Option<TransferSignal>> + Send + '_>> {
            Box::pin(self.event_rx.recv())
        }
    }

    fn source() -> (
        tokio::sync::mpsc::UnboundedSender<TransferSignal>,
        FakeSignalSource,
    ) {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        (event_tx, FakeSignalSource { event_rx })
    }

    #[test]
    fn forwards_interrupt_and_terminate_as_distinct_events() {
        for expected in [TransferSignal::Interrupt, TransferSignal::Terminate] {
            let (source_tx, source) = source();
            let (emitted_tx, emitted_rx) = mpsc::channel();
            let forwarder = spawn_signal_forwarder_with_factory(
                move || Ok(source),
                move |signal| {
                    let _ = emitted_tx.send(signal);
                },
            )
            .unwrap();

            source_tx.send(expected).unwrap();
            assert_eq!(
                emitted_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
                expected
            );
            drop(forwarder);
        }
    }

    #[test]
    fn dropping_forwarder_stops_without_emitting_another_event() {
        let (source_tx, source) = source();
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let emitted_for_thread = emitted.clone();
        let forwarder = spawn_signal_forwarder_with_factory(
            move || Ok(source),
            move |signal| {
                emitted_for_thread.lock().unwrap().push(signal);
            },
        )
        .unwrap();

        drop(forwarder);
        assert!(source_tx.send(TransferSignal::Interrupt).is_err());
        assert!(emitted.lock().unwrap().is_empty());
    }
}
