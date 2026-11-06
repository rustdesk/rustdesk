use super::super::wide_string;
use hbb_common::{bail, log, ResultType};
use std::io;
use windows::{
    core::{Owned, PCWSTR},
    Win32::{
        Foundation::{
            ERROR_IO_PENDING, ERROR_NOT_FOUND, ERROR_PIPE_CONNECTED, HANDLE, WAIT_OBJECT_0,
        },
        Storage::FileSystem::{
            WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_OUTBOUND,
        },
        System::{
            Pipes::{
                ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId,
                PIPE_REJECT_REMOTE_CLIENTS,
            },
            Threading::{CreateEventW, GetProcessId, WaitForMultipleObjects, INFINITE},
            IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
        },
    },
};

const READY: &[u8] = b"1\r\n";
const PIPE_INSTANCES: u32 = 1;

pub(super) struct InstallGate {
    pipe: Owned<HANDLE>,
    name: String,
}

impl InstallGate {
    pub(super) fn new() -> ResultType<Self> {
        let name = format!(
            r"\\.\pipe\rustdesk-install-{}",
            uuid::Uuid::new_v4().simple()
        );
        let wide_name = wide_string(&name);
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(wide_name.as_ptr()),
                PIPE_ACCESS_OUTBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_INSTANCES,
                READY.len() as u32,
                0,
                0,
                None,
            )
        };
        if pipe.is_invalid() {
            return Err(io::Error::last_os_error().into());
        }
        Ok(Self {
            pipe: unsafe { Owned::new(pipe) },
            name,
        })
    }

    pub(super) fn name(&self) -> &str {
        &self.name
    }

    pub(super) fn wait_for_client(&self, process: HANDLE) -> ResultType<()> {
        let event = unsafe { Owned::new(CreateEventW(None, true, false, None)?) };
        let mut overlapped = OVERLAPPED {
            hEvent: *event,
            ..Default::default()
        };
        if let Err(error) = unsafe { ConnectNamedPipe(*self.pipe, Some(&mut overlapped)) } {
            if error.code() == ERROR_IO_PENDING.to_hresult() {
                self.wait_for_io(&overlapped, process)?;
            } else if error.code() != ERROR_PIPE_CONNECTED.to_hresult() {
                return Err(error.into());
            }
        }
        let mut client = 0;
        unsafe { GetNamedPipeClientProcessId(*self.pipe, &mut client)? };
        if client != unsafe { GetProcessId(process) } {
            bail!("Unexpected installer handoff pipe client");
        }
        Ok(())
    }

    // cmd's SET /P must read a line successfully before it opens the BAT. Closing
    // the pipe without this signal fails the command instead of running the BAT.
    pub(super) fn release(&self, process: HANDLE) -> ResultType<()> {
        let event = unsafe { Owned::new(CreateEventW(None, true, false, None)?) };
        let mut overlapped = OVERLAPPED {
            hEvent: *event,
            ..Default::default()
        };
        // Windows 7 requires a byte-count pointer even for overlapped writes.
        let mut written = 0;
        if let Err(error) = unsafe {
            WriteFile(
                *self.pipe,
                Some(READY),
                Some(&mut written),
                Some(&mut overlapped),
            )
        } {
            if error.code() != ERROR_IO_PENDING.to_hresult() {
                return Err(error.into());
            }
            self.wait_for_io(&overlapped, process)?;
        }
        // Keep the server handle alive until cmd exits so the signal stays buffered.
        Ok(())
    }

    fn wait_for_io(&self, overlapped: &OVERLAPPED, process: HANDLE) -> ResultType<()> {
        let wait =
            unsafe { WaitForMultipleObjects(&[overlapped.hEvent, process], false, INFINITE) };
        let mut transferred = 0;
        if wait == WAIT_OBJECT_0 {
            unsafe { GetOverlappedResult(*self.pipe, overlapped, &mut transferred, false)? };
            return Ok(());
        }
        let error = io::Error::last_os_error();
        // Retire pending I/O before the stack-allocated OVERLAPPED and event go away.
        if let Err(error) = unsafe { CancelIoEx(*self.pipe, Some(overlapped)) } {
            if error.code() != ERROR_NOT_FOUND.to_hresult() {
                log::warn!("Failed to cancel installer handoff I/O: {error}");
            }
        }
        // Cancellation normally completes with ERROR_OPERATION_ABORTED; preserve
        // the original wait failure after observing that completion.
        let _ = unsafe { GetOverlappedResult(*self.pipe, overlapped, &mut transferred, true) };
        if wait.0 == WAIT_OBJECT_0.0 + 1 {
            bail!("Elevated installer exited before the file handoff completed");
        }
        bail!("Installer handoff wait failed ({}): {error}", wait.0)
    }
}
