use super::{handles::Handle, queues, security::Descriptor, Queue, PIPE_PREFIX};
use hbb_common::{bail, ResultType};
use std::time::Duration;
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{
            ERROR_BROKEN_PIPE, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, HANDLE, WAIT_OBJECT_0,
            WAIT_TIMEOUT,
        },
        Storage::FileSystem::{
            ReadFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
        },
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe,
                GetNamedPipeClientProcessId, PIPE_REJECT_REMOTE_CLIENTS,
            },
            Threading::{CreateEventW, WaitForSingleObject},
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
        },
    },
};

pub(crate) const BUFFER_SIZE: usize = 64 * 1024;
const STOP_POLL_MS: u32 = 200;
const SINGLE_INSTANCE: u32 = 1;

pub struct Pipe(Handle);

impl Pipe {
    pub fn new(port: &str) -> ResultType<Self> {
        let identifier = port.strip_prefix(PIPE_PREFIX).ok_or_else(|| {
            hbb_common::anyhow::anyhow!("The PDF port is outside the protected namespace")
        })?;
        super::port_name(identifier)?;
        let owner = super::identity::controlled_user_sid()?;
        // Local Monitor opens the port while impersonating the submitting user and
        // requires full access. Namespace protection and the Spooler PID check
        // still prevent that user from replacing the receiver or injecting output.
        let descriptor = Descriptor::new(&format!(
            "D:P(D;;GA;;;NU)(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{owner})"
        ))?;
        let handle = create(port, &descriptor)?;
        Ok(Self(handle))
    }

    pub fn connect_queue(&self, queue: &Queue, active: &impl Fn() -> bool) -> ResultType<()> {
        self.connect_checked(&|| {
            if !active() {
                return Ok(false);
            }
            if !queues::get(&queue.name)?.is_some_and(|current| current.port == queue.port) {
                bail!("PDF printer queue changed; reopen the capture port");
            }
            Ok(true)
        })
    }

    fn connect_checked(&self, active: &impl Fn() -> ResultType<bool>) -> ResultType<()> {
        if !active()? {
            bail!("PDF capture stopped before IO completed");
        }
        let mut operation = Operation::new(self.0 .0)?;
        let result = unsafe { ConnectNamedPipe(self.0 .0, Some(&mut operation.overlapped)) };
        if result
            .as_ref()
            .err()
            .is_some_and(|error| error.code() == ERROR_PIPE_CONNECTED.to_hresult())
        {
            return Ok(());
        }
        operation.finish(result, active)?;
        Ok(())
    }

    pub fn client_pid(&self) -> ResultType<u32> {
        let mut pid = 0;
        unsafe { GetNamedPipeClientProcessId(self.0 .0, &mut pid)? };
        Ok(pid)
    }

    pub fn read(&self, buffer: &mut [u8], active: &impl Fn() -> bool) -> ResultType<usize> {
        let mut operation = Operation::new(self.0 .0)?;
        let result = unsafe {
            ReadFile(
                self.0 .0,
                Some(buffer),
                None,
                Some(&mut operation.overlapped),
            )
        };
        match operation.finish(result, &|| Ok(active())) {
            Ok(count) => Ok(count as usize),
            Err(error)
                if error
                    .downcast_ref::<windows::core::Error>()
                    .is_some_and(|error| error.code() == ERROR_BROKEN_PIPE.to_hresult()) =>
            {
                Ok(0)
            }
            Err(error) => Err(error),
        }
    }

    pub fn disconnect(&self) -> ResultType<()> {
        unsafe { DisconnectNamedPipe(self.0 .0)? };
        Ok(())
    }
}

pub(crate) fn create(port: &str, descriptor: &Descriptor) -> ResultType<Handle> {
    let name = HSTRING::from(port);
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
            PIPE_REJECT_REMOTE_CLIENTS,
            SINGLE_INSTANCE,
            BUFFER_SIZE as u32,
            BUFFER_SIZE as u32,
            0,
            Some(&descriptor.attributes()),
        )
    };
    if handle.is_invalid() {
        return Err(windows::core::Error::from_win32().into());
    }
    Ok(Handle(handle))
}

struct Operation {
    pipe: HANDLE,
    event: Handle,
    overlapped: OVERLAPPED,
    pending: bool,
}

impl Operation {
    fn new(pipe: HANDLE) -> ResultType<Self> {
        let event = Handle(unsafe { CreateEventW(None, true, false, PCWSTR::null())? });
        let overlapped = OVERLAPPED {
            hEvent: event.0,
            ..Default::default()
        };
        Ok(Self {
            pipe,
            event,
            overlapped,
            pending: false,
        })
    }

    fn finish(
        &mut self,
        result: windows::core::Result<()>,
        active: &impl Fn() -> ResultType<bool>,
    ) -> ResultType<u32> {
        if let Err(error) = result {
            if error.code() != ERROR_IO_PENDING.to_hresult() {
                return Err(error.into());
            }
            self.pending = true;
            self.wait(active)?;
        }
        let mut count = 0;
        let result = unsafe { GetOverlappedResult(self.pipe, &self.overlapped, &mut count, false) };
        self.pending = false;
        result?;
        Ok(count)
    }

    fn wait(&self, active: &impl Fn() -> ResultType<bool>) -> ResultType<()> {
        loop {
            if !active()? {
                bail!("PDF capture stopped before IO completed");
            }
            match unsafe { WaitForSingleObject(self.event.0, STOP_POLL_MS) } {
                WAIT_OBJECT_0 => return Ok(()),
                WAIT_TIMEOUT => continue,
                _ => return Err(windows::core::Error::from_win32().into()),
            }
        }
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        if !self.pending {
            return;
        }
        // The OVERLAPPED and read buffer must remain alive until cancellation completes.
        if let Err(error) = unsafe { CancelIoEx(self.pipe, Some(&self.overlapped)) } {
            hbb_common::throttled_log!(
                Duration::from_secs(60),
                warn,
                "PDF IO cancellation: {error}"
            );
        }
        let mut count = 0;
        let result = unsafe { GetOverlappedResult(self.pipe, &self.overlapped, &mut count, true) };
        if let Err(error) = result {
            hbb_common::log::trace!("PDF IO stopped: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    const TEST_WAIT: Duration = Duration::from_secs(2);

    #[test]
    fn pdf_capture_releases_a_removed_queue_without_a_client() {
        let identifier = format!("{:032x}", hbb_common::rand::random::<u128>());
        let queue = Queue {
            name: format!("RustDesk PDF removed queue test {identifier}"),
            port: format!(r"\\.\pipe\RustDesk-PdfRebindTest-{identifier}"),
        };
        let descriptor = Descriptor::new("D:P(A;;GA;;;WD)").unwrap();
        let pipe = Pipe(create(&queue.port, &descriptor).unwrap());
        let started = Instant::now();
        let error = pipe
            .connect_queue(&queue, &|| started.elapsed() < TEST_WAIT)
            .unwrap_err();
        assert!(
            error.to_string().contains("PDF printer queue changed"),
            "Capture waited for a client on a removed queue: {error:#}"
        );
    }
}
