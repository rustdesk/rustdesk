use std::{
    io::{self, IsTerminal, Write},
    mem::MaybeUninit,
    sync::Arc,
};

use hbb_common::libc;

pub(crate) const DETACH_BYTE: u8 = 0x1d;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TtySize {
    pub(crate) rows: u16,
    pub(crate) cols: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum InputChunk {
    Data(Vec<u8>),
    Detach,
}

pub(crate) fn split_input(bytes: &[u8]) -> Vec<InputChunk> {
    let Some(detach_index) = bytes.iter().position(|byte| *byte == DETACH_BYTE) else {
        return if bytes.is_empty() {
            Vec::new()
        } else {
            vec![InputChunk::Data(bytes.to_vec())]
        };
    };

    let mut chunks = Vec::with_capacity(2);
    if detach_index > 0 {
        chunks.push(InputChunk::Data(bytes[..detach_index].to_vec()));
    }
    chunks.push(InputChunk::Detach);
    chunks
}

pub(crate) trait TtyBackend: Send + Sync + 'static {
    type Snapshot: Send + 'static;

    fn stdin_is_tty(&self) -> bool;
    fn stdout_is_tty(&self) -> bool;
    fn capture(&self) -> io::Result<Self::Snapshot>;
    fn enter_raw(&self, snapshot: &Self::Snapshot) -> io::Result<()>;
    fn restore(&self, snapshot: &Self::Snapshot) -> io::Result<()>;
    fn size(&self) -> io::Result<TtySize>;
}

pub(crate) struct SystemTtyBackend;

impl SystemTtyBackend {
    fn get_attributes() -> io::Result<libc::termios> {
        let mut attributes = MaybeUninit::<libc::termios>::uninit();
        // SAFETY: `attributes` points to writable storage for a termios value, and
        // `STDIN_FILENO` remains owned by the process for the duration of the call.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, attributes.as_mut_ptr()) } == -1 {
            return Err(last_tty_error("failed to capture stdin TTY attributes"));
        }

        // SAFETY: tcgetattr returned success and initialized the termios value.
        Ok(unsafe { attributes.assume_init() })
    }

    fn set_attributes(attributes: &libc::termios) -> io::Result<()> {
        // SAFETY: `attributes` is a valid termios value and remains borrowed for
        // the duration of the tcsetattr call.
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, attributes) } == -1 {
            Err(last_tty_error("failed to update stdin TTY attributes"))
        } else {
            Ok(())
        }
    }

    fn disable_echo(snapshot: &libc::termios) -> io::Result<()> {
        let mut attributes = *snapshot;
        attributes.c_lflag &= !libc::ECHO;
        Self::set_attributes(&attributes)
    }
}

impl TtyBackend for SystemTtyBackend {
    type Snapshot = libc::termios;

    fn stdin_is_tty(&self) -> bool {
        io::stdin().is_terminal()
    }

    fn stdout_is_tty(&self) -> bool {
        io::stdout().is_terminal()
    }

    fn capture(&self) -> io::Result<Self::Snapshot> {
        Self::get_attributes()
    }

    fn enter_raw(&self, snapshot: &Self::Snapshot) -> io::Result<()> {
        let mut attributes = *snapshot;
        // SAFETY: `attributes` is an initialized termios value owned by this
        // function, so cfmakeraw may mutate it in place.
        unsafe { libc::cfmakeraw(&mut attributes) };
        Self::set_attributes(&attributes)
    }

    fn restore(&self, snapshot: &Self::Snapshot) -> io::Result<()> {
        Self::set_attributes(snapshot)
    }

    fn size(&self) -> io::Result<TtySize> {
        let mut window_size = MaybeUninit::<libc::winsize>::uninit();
        // SAFETY: `window_size` points to writable storage for a winsize value,
        // and stdout remains owned by the process for the duration of the call.
        if unsafe {
            libc::ioctl(
                libc::STDOUT_FILENO,
                libc::TIOCGWINSZ,
                window_size.as_mut_ptr(),
            )
        } == -1
        {
            return Err(last_tty_error("failed to read local TTY size"));
        }

        // SAFETY: ioctl returned success and initialized the winsize value.
        let window_size = unsafe { window_size.assume_init() };
        if window_size.ws_row == 0 || window_size.ws_col == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "local TTY reported zero rows or columns",
            ));
        }

        Ok(TtySize {
            rows: window_size.ws_row,
            cols: window_size.ws_col,
        })
    }
}

fn last_tty_error(context: &str) -> io::Error {
    let error = io::Error::last_os_error();
    io::Error::new(error.kind(), format!("{context}: {error}"))
}

pub(crate) struct LocalTtyGuard<B: TtyBackend> {
    backend: Arc<B>,
    snapshot: Option<B::Snapshot>,
}

impl<B: TtyBackend> LocalTtyGuard<B> {
    pub(crate) fn enter(backend: Arc<B>) -> io::Result<Self> {
        let snapshot = backend.capture()?;
        backend.enter_raw(&snapshot)?;
        Ok(Self {
            backend,
            snapshot: Some(snapshot),
        })
    }

    pub(crate) fn restore(&mut self) -> io::Result<()> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Ok(());
        };
        self.backend.restore(snapshot)?;
        self.snapshot.take();
        Ok(())
    }
}

impl<B: TtyBackend> Drop for LocalTtyGuard<B> {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("RDH headless terminal failed to restore local TTY: {error}");
        }
    }
}

struct EchoGuard {
    snapshot: Option<libc::termios>,
}

impl EchoGuard {
    fn disable() -> io::Result<Self> {
        let snapshot = SystemTtyBackend::get_attributes()?;
        SystemTtyBackend::disable_echo(&snapshot)?;
        Ok(Self {
            snapshot: Some(snapshot),
        })
    }

    fn restore(&mut self) -> io::Result<()> {
        let Some(snapshot) = self.snapshot.as_ref() else {
            return Ok(());
        };
        SystemTtyBackend::set_attributes(snapshot)?;
        self.snapshot.take();
        Ok(())
    }
}

impl Drop for EchoGuard {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("RDH headless terminal failed to restore stdin echo: {error}");
        }
    }
}

fn trim_line_endings(line: String) -> String {
    line.trim_end_matches(['\r', '\n']).to_owned()
}

fn secret_from_line(line: Option<String>) -> Option<String> {
    line.filter(|line| line.as_bytes() != [DETACH_BYTE].as_slice())
}

fn confirmation_from_line(line: Option<&str>) -> Option<bool> {
    line.map(|value| value.eq_ignore_ascii_case("y") || value.eq_ignore_ascii_case("yes"))
}

fn read_prompt_line(prompt: &str) -> io::Result<Option<String>> {
    {
        let mut stderr = io::stderr().lock();
        stderr.write_all(prompt.as_bytes())?;
        stderr.flush()?;
    }

    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        Ok(None)
    } else {
        Ok(Some(trim_line_endings(line)))
    }
}

pub(crate) fn prompt_line(prompt: &str) -> io::Result<Option<String>> {
    read_prompt_line(prompt)
}

pub(crate) fn prompt_secret(prompt: &str) -> io::Result<Option<String>> {
    let mut echo_guard = EchoGuard::disable()?;
    let line_result = read_prompt_line(prompt);
    let restore_result = echo_guard.restore();
    let newline_result = {
        let mut stderr = io::stderr().lock();
        stderr.write_all(b"\n").and_then(|()| stderr.flush())
    };

    let line = line_result?;
    restore_result?;
    newline_result?;
    Ok(secret_from_line(line))
}

pub(crate) fn prompt_confirmation(prompt: &str) -> io::Result<Option<bool>> {
    let line = prompt_line(prompt)?;
    Ok(confirmation_from_line(line.as_deref()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignalEvent {
    Resize,
    Shutdown,
}

pub(crate) struct SignalForwarder {
    shutdown_tx: Option<hbb_common::tokio::sync::oneshot::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

pub(crate) fn spawn_signal_forwarder<F>(emit: F) -> io::Result<SignalForwarder>
where
    F: FnMut(SignalEvent) + Send + 'static,
{
    use hbb_common::tokio;
    use tokio::signal::unix::{signal, SignalKind};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let runtime_guard = runtime.enter();
    let mut resize = signal(SignalKind::window_change())?;
    let mut terminate = signal(SignalKind::terminate())?;
    let mut hangup = signal(SignalKind::hangup())?;
    drop(runtime_guard);

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let thread = std::thread::spawn(move || {
        runtime.block_on(async move {
            let mut emit = emit;
            tokio::select! {
                biased;
                _ = shutdown_rx => {}
                _ = terminate.recv() => emit(SignalEvent::Shutdown),
                _ = hangup.recv() => emit(SignalEvent::Shutdown),
                _ = async {
                    loop {
                        if resize.recv().await.is_none() {
                            break;
                        }
                        emit(SignalEvent::Resize);
                    }
                } => {}
            }
        });
    });
    Ok(SignalForwarder {
        shutdown_tx: Some(shutdown_tx),
        thread: Some(thread),
    })
}

impl Drop for SignalForwarder {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                eprintln!("RDH headless terminal signal thread panicked");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct FakeTtyBackend {
        capture_count: AtomicUsize,
        raw_count: AtomicUsize,
        restore_count: AtomicUsize,
        fail_raw: bool,
    }

    impl FakeTtyBackend {
        fn succeeding() -> Self {
            Self::new(false)
        }

        fn failing_raw() -> Self {
            Self::new(true)
        }

        fn new(fail_raw: bool) -> Self {
            Self {
                capture_count: AtomicUsize::new(0),
                raw_count: AtomicUsize::new(0),
                restore_count: AtomicUsize::new(0),
                fail_raw,
            }
        }

        fn counts(&self) -> (usize, usize, usize) {
            (
                self.capture_count.load(Ordering::SeqCst),
                self.raw_count.load(Ordering::SeqCst),
                self.restore_count.load(Ordering::SeqCst),
            )
        }
    }

    impl TtyBackend for FakeTtyBackend {
        type Snapshot = ();

        fn stdin_is_tty(&self) -> bool {
            true
        }

        fn stdout_is_tty(&self) -> bool {
            true
        }

        fn capture(&self) -> io::Result<Self::Snapshot> {
            self.capture_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn enter_raw(&self, _snapshot: &Self::Snapshot) -> io::Result<()> {
            self.raw_count.fetch_add(1, Ordering::SeqCst);
            if self.fail_raw {
                Err(io::Error::other("injected raw-mode failure"))
            } else {
                Ok(())
            }
        }

        fn restore(&self, _snapshot: &Self::Snapshot) -> io::Result<()> {
            self.restore_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn size(&self) -> io::Result<TtySize> {
            Ok(TtySize { rows: 24, cols: 80 })
        }
    }

    #[test]
    fn split_input_forwards_control_bytes_except_detach() {
        assert_eq!(
            split_input(&[b'a', 0x03, 0x04, b'b']),
            vec![InputChunk::Data(vec![b'a', 0x03, 0x04, b'b'])]
        );
    }

    #[test]
    fn split_input_stops_at_local_detach_byte() {
        assert_eq!(
            split_input(&[b'a', b'b', DETACH_BYTE, b'c']),
            vec![InputChunk::Data(vec![b'a', b'b']), InputChunk::Detach]
        );
    }

    #[test]
    fn split_input_emits_only_detach_when_escape_is_first() {
        assert_eq!(split_input(&[DETACH_BYTE]), vec![InputChunk::Detach]);
    }

    #[test]
    fn tty_guard_restores_exactly_once_on_drop() {
        let backend = Arc::new(FakeTtyBackend::succeeding());
        {
            let _guard = LocalTtyGuard::enter(backend.clone()).unwrap();
        }
        assert_eq!(backend.counts(), (1, 1, 1));
    }

    #[test]
    fn tty_guard_does_not_restore_when_enter_raw_fails() {
        let backend = Arc::new(FakeTtyBackend::failing_raw());
        assert!(LocalTtyGuard::enter(backend.clone()).is_err());
        assert_eq!(backend.counts(), (1, 1, 0));
    }

    #[test]
    fn explicit_restore_makes_drop_idempotent() {
        let backend = Arc::new(FakeTtyBackend::succeeding());
        {
            let mut guard = LocalTtyGuard::enter(backend.clone()).unwrap();
            guard.restore().unwrap();
            assert_eq!(backend.counts(), (1, 1, 1));
        }
        assert_eq!(backend.counts(), (1, 1, 1));
    }

    #[test]
    fn prompt_line_trims_only_trailing_cr_and_lf() {
        assert_eq!(trim_line_endings(" value \t\r\n".to_owned()), " value \t");
    }

    #[test]
    fn confirmation_preserves_explicit_no_and_eof() {
        assert_eq!(confirmation_from_line(Some("y")), Some(true));
        assert_eq!(confirmation_from_line(Some("YES")), Some(true));
        assert_eq!(confirmation_from_line(Some(" yes ")), Some(false));
        assert_eq!(confirmation_from_line(Some("yeah")), Some(false));
        assert_eq!(confirmation_from_line(Some("")), Some(false));
        assert_eq!(confirmation_from_line(None), None);
    }

    #[test]
    fn secret_maps_only_single_detach_byte_to_none() {
        assert_eq!(secret_from_line(Some("\u{1d}".to_owned())), None);
        assert_eq!(
            secret_from_line(Some("\u{1d}secret".to_owned())),
            Some("\u{1d}secret".to_owned())
        );
        assert_eq!(secret_from_line(Some(String::new())), Some(String::new()));
        assert_eq!(secret_from_line(None), None);
    }

    #[test]
    fn signal_forwarder_drop_stops_without_emitting() {
        let emitted = Arc::new(AtomicUsize::new(0));
        {
            let emitted = emitted.clone();
            let _forwarder = spawn_signal_forwarder(move |_| {
                emitted.fetch_add(1, Ordering::SeqCst);
            })
            .unwrap();
        }
        assert_eq!(emitted.load(Ordering::SeqCst), 0);
    }
}
