use super::{
    display_name,
    handles::{Buffer, Printer},
    owner_marker, Queue, DRIVER_NAME, OWNER_PREFIX, PIPE_PREFIX,
};
use hbb_common::{anyhow::Context, bail, ResultType};
use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::ERROR_INVALID_PRINTER_NAME,
        Graphics::Printing::{
            EnumPrintersW, GetPrinterW, PRINTER_ACCESS_USE, PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS,
            PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_2W,
        },
    },
};

const QUEUE_INFO_LEVEL: u32 = 2;
const LEGACY_DRIVER_NAME: &str = "RustDesk v4 Printer Driver";

pub(crate) struct Info {
    pub name: String,
    pub port: String,
    pub driver: String,
    pub comment: String,
    pub attributes: u32,
    pub jobs: u32,
}

impl Info {
    pub(crate) fn check_migration(&self, app_name: &str) -> ResultType<()> {
        if !self.is_legacy(app_name) {
            bail!("The existing printer does not match this application's legacy queue");
        }
        if self.jobs != 0 {
            bail!("Finish or cancel pending print jobs before migrating the printer");
        }
        Ok(())
    }

    pub(crate) fn is_legacy(&self, app_name: &str) -> bool {
        let name = format!("{app_name} Printer");
        self.name.eq_ignore_ascii_case(&name)
            && self.port.eq_ignore_ascii_case(&name)
            && self.driver.eq_ignore_ascii_case(LEGACY_DRIVER_NAME)
    }
}

pub fn is_installed(app_name: &str) -> ResultType<bool> {
    Ok(installed(app_name)?.is_some()
        || get(&display_name(app_name)?)?.is_some_and(|info| info.is_legacy(app_name)))
}

pub fn installed(app_name: &str) -> ResultType<Option<Queue>> {
    let marker = owner_marker(app_name)?;
    let mut owned = all()?.into_iter().filter(|queue| queue.comment == marker);
    let Some(info) = owned.next() else {
        return Ok(None);
    };
    if owned.next().is_some() {
        bail!("More than one PDF queue belongs to this application");
    }
    validate(&info)?;
    Ok(Some(Queue {
        name: info.name,
        port: info.port,
    }))
}

pub(crate) fn validate(info: &Info) -> ResultType<()> {
    if !info.driver.eq_ignore_ascii_case(DRIVER_NAME)
        || !info.comment.starts_with(OWNER_PREFIX)
        || !info.port.starts_with(PIPE_PREFIX)
    {
        bail!("The PDF queue does not match its owned driver and protected port");
    }
    let identifier = &info.port[PIPE_PREFIX.len()..];
    super::port_name(identifier)?;
    if info.attributes & PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS == 0 {
        bail!("The PDF queue must retain jobs until capture is verified");
    }
    Ok(())
}

pub fn is_remote_queue(name: &str) -> ResultType<bool> {
    let info = get(name)?
        .ok_or_else(|| hbb_common::anyhow::anyhow!("The selected printer no longer exists"))?;
    Ok(info.comment.starts_with(OWNER_PREFIX)
        || info.driver.eq_ignore_ascii_case(LEGACY_DRIVER_NAME))
}

pub(crate) fn printer_names(printers: Vec<Info>) -> Vec<String> {
    printers
        .into_iter()
        .filter(|info| {
            !info.comment.starts_with(OWNER_PREFIX)
                && !info.driver.eq_ignore_ascii_case(LEGACY_DRIVER_NAME)
        })
        .map(|info| info.name)
        .collect()
}

pub(crate) fn get(name: &str) -> ResultType<Option<Info>> {
    let printer = match Printer::open(name, PRINTER_ACCESS_USE) {
        Ok(printer) => printer,
        Err(error) => {
            if error
                .downcast_ref::<windows::core::Error>()
                .is_some_and(|error| error.code() == ERROR_INVALID_PRINTER_NAME.to_hresult())
            {
                return Ok(None);
            }
            return Err(error);
        }
    };
    let buffer = read(&printer)?;
    info_from(unsafe { buffer.item::<PRINTER_INFO_2W>(0)? }).map(Some)
}

pub(crate) fn read(printer: &Printer) -> ResultType<Buffer> {
    Buffer::query(|buffer, needed| unsafe {
        GetPrinterW(printer.0, QUEUE_INFO_LEVEL, buffer, needed)
    })?
    .context("The print API returned an empty buffer size")
}

pub(crate) fn all() -> ResultType<Vec<Info>> {
    let mut count = 0;
    let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
    let Some(buffer) = Buffer::query(|buffer, needed| unsafe {
        EnumPrintersW(
            flags,
            PCWSTR::null(),
            QUEUE_INFO_LEVEL,
            buffer,
            needed,
            &mut count,
        )
    })?
    else {
        return Ok(Vec::new());
    };
    (0..count)
        .map(|index| info_from(unsafe { buffer.item::<PRINTER_INFO_2W>(index as usize)? }))
        .collect()
}

pub(crate) fn info_from(info: PRINTER_INFO_2W) -> ResultType<Info> {
    Ok(Info {
        name: text(info.pPrinterName)?,
        port: text(info.pPortName)?,
        driver: text(info.pDriverName)?,
        comment: text(info.pComment)?,
        attributes: info.Attributes,
        jobs: info.cJobs,
    })
}

pub(crate) fn text(value: PWSTR) -> ResultType<String> {
    if value.is_null() {
        return Ok(String::new());
    }
    Ok(unsafe { value.to_string()? })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerated_printers_retain_their_identity_when_queried() {
        let printers = all().unwrap();
        for listed in &printers {
            let queried = get(&listed.name)
                .unwrap()
                .expect("Listed printer disappeared");
            assert_eq!(queried.name, listed.name);
            assert_eq!(queried.port, listed.port);
            assert_eq!(queried.driver, listed.driver);
        }
        println!("Verified {} native printer queries", printers.len());
    }

    fn legacy_queue() -> Info {
        Info {
            name: "Branded Client Printer".to_owned(),
            port: "Branded Client Printer".to_owned(),
            driver: LEGACY_DRIVER_NAME.to_owned(),
            comment: String::new(),
            attributes: PRINTER_ATTRIBUTE_KEEPPRINTEDJOBS,
            jobs: 0,
        }
    }

    #[test]
    fn discovery_keeps_unavailable_and_local_printers() {
        let unavailable = Info {
            name: "Unavailable printer regression fixture".to_owned(),
            driver: "Network printer driver".to_owned(),
            ..legacy_queue()
        };
        let local = Info {
            name: "Local PDF printer regression fixture".to_owned(),
            driver: DRIVER_NAME.to_owned(),
            ..legacy_queue()
        };
        let remote = Info {
            driver: DRIVER_NAME.to_owned(),
            comment: owner_marker("Branded Client").unwrap(),
            ..legacy_queue()
        };
        assert_eq!(
            printer_names(vec![unavailable, local, remote, legacy_queue()]),
            [
                "Unavailable printer regression fixture",
                "Local PDF printer regression fixture"
            ]
        );
    }

    #[test]
    fn migration_accepts_only_the_matching_legacy_queue() {
        assert!(legacy_queue().check_migration("Branded Client").is_ok());
        assert!(legacy_queue().check_migration("Another Client").is_err());
        let unrelated = Info {
            port: "PORTPROMPT:".to_owned(),
            ..legacy_queue()
        };
        assert!(unrelated.check_migration("Branded Client").is_err());
    }

    #[test]
    fn migration_refuses_pending_documents() {
        let busy = Info {
            jobs: 1,
            ..legacy_queue()
        };
        assert!(busy
            .check_migration("Branded Client")
            .unwrap_err()
            .to_string()
            .contains("pending print jobs"));
    }
}
