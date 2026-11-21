use super::{
    handles::{Buffer, Printer},
    identity, queues, Queue,
};
use hbb_common::{anyhow::Context, bail, ResultType};
use std::{thread, time::Duration};
use windows::Win32::{
    Foundation::SYSTEMTIME,
    Graphics::Printing::{
        EnumJobsW, GetJobW, SetJobW, JOB_CONTROL_DELETE, JOB_INFO_1W, JOB_STATUS_DELETED,
        JOB_STATUS_DELETING, JOB_STATUS_ERROR, JOB_STATUS_PRINTED, JOB_STATUS_PRINTING,
        PRINTER_ALL_ACCESS,
    },
};

const JOB_INFO_LEVEL: u32 = 1;
const ALL_JOBS: u32 = u32::MAX;
const FAILED: u32 = JOB_STATUS_ERROR | JOB_STATUS_DELETING | JOB_STATUS_DELETED;
const COMPLETION_POLL: Duration = Duration::from_millis(25);

pub struct Job {
    printer: Printer,
    id: u32,
    submitted: SYSTEMTIME,
    owner: String,
}

impl Job {
    pub fn bind(queue: &Queue, owner: &str) -> ResultType<Option<Self>> {
        let printer = Printer::open(&queue.name, PRINTER_ALL_ACCESS)?;
        let Some((buffer, count)) = Self::list(&printer)? else {
            return Ok(None);
        };
        let mut printing = Vec::new();
        for index in 0..count {
            let info = unsafe { buffer.item::<JOB_INFO_1W>(index as usize)? };
            // Retained completed jobs cannot identify the current pipe connection.
            if info.Status & JOB_STATUS_PRINTING != 0 {
                printing.push(info);
            }
        }
        let info = match printing.as_slice() {
            [] => return Ok(None),
            [info] => *info,
            _ => bail!("More than one PDF job is printing; output ownership is ambiguous"),
        };
        if info.Status & FAILED != 0 {
            bail!("The PDF job was cancelled or failed before capture");
        }
        if identity::account_sid(&queues::text(info.pUserName)?)? != owner {
            bail!("The PDF job belongs to another Windows user");
        }
        Ok(Some(Self {
            printer,
            id: info.JobId,
            submitted: info.Submitted,
            owner: owner.to_owned(),
        }))
    }

    fn list(printer: &Printer) -> ResultType<Option<(Buffer, u32)>> {
        let mut count = 0;
        let buffer = Buffer::query(|buffer, needed| unsafe {
            EnumJobsW(
                printer.0,
                0,
                ALL_JOBS,
                JOB_INFO_LEVEL,
                buffer,
                needed,
                &mut count,
            )
        })?;
        Ok(buffer.map(|buffer| (buffer, count)))
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn check(&self) -> ResultType<bool> {
        let (buffer, info) = self.read()?;
        self.validate(&info)?;
        if info.Status & FAILED != 0 {
            bail!("The PDF job was cancelled or failed during capture");
        }
        drop(buffer);
        Ok(info.Status & JOB_STATUS_PRINTED != 0)
    }

    pub fn wait_completed(&self, active: &impl Fn() -> bool) -> ResultType<()> {
        loop {
            if !active() {
                bail!("PDF capture stopped before Spooler completion");
            }
            if self.check()? {
                return Ok(());
            }
            thread::sleep(COMPLETION_POLL);
        }
    }

    pub fn remove(&self) -> ResultType<()> {
        let (_buffer, info) = self.read()?;
        self.validate(&info)?;
        unsafe { SetJobW(self.printer.0, self.id, 0, None, JOB_CONTROL_DELETE).ok()? };
        Ok(())
    }

    fn validate(&self, info: &JOB_INFO_1W) -> ResultType<()> {
        if info.JobId != self.id
            || info.Submitted != self.submitted
            || identity::account_sid(&queues::text(info.pUserName)?)? != self.owner
        {
            bail!("The bound PDF job changed identity");
        }
        Ok(())
    }

    fn read(&self) -> ResultType<(Buffer, JOB_INFO_1W)> {
        let buffer = Buffer::query(|buffer, needed| unsafe {
            GetJobW(self.printer.0, self.id, JOB_INFO_LEVEL, buffer, needed).ok()
        })?
        .context("The print API returned an empty buffer size")?;
        let info = unsafe { buffer.item::<JOB_INFO_1W>(0)? };
        Ok((buffer, info))
    }
}
