use hbb_common::{anyhow::Context, bail, ResultType};
use remote_printer::pdf;
use std::io;
use winreg::{
    enums::{HKEY_LOCAL_MACHINE, KEY_QUERY_VALUE, KEY_SET_VALUE},
    types::FromRegValue,
    RegKey, RegValue,
};

const OLD_PORT_VALUE: &str = "PDFPRINTER_REMOVAL_PORT";

pub(super) fn disable() -> ResultType<()> {
    let app = crate::get_app_name();
    let mut record = None;
    pdf::uninstall(&app, |port| {
        // DeletePrinter can succeed while the port is busy. A reinstall gets a different port.
        let (key, _) = RegKey::predef(HKEY_LOCAL_MACHINE).create_subkey(super::marker_key(&app))?;
        key.set_value(port, &1u32)?;
        record = Some((key, port.to_owned()));
        Ok(())
    })?;
    super::write_marker(false)?;
    if let Some((key, port)) = record {
        forget(&key, &port)?;
    }
    Ok(())
}

pub(super) fn uninstall() -> ResultType<()> {
    let queue = disable();
    let ports = retry_ports();
    match (queue, ports) {
        (Err(queue), Err(ports)) => bail!("{queue:#}; {ports:#}"),
        (queue, ports) => queue.and(ports),
    }
}

fn retry_ports() -> ResultType<()> {
    let key = match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
        super::marker_key(&crate::get_app_name()),
        KEY_QUERY_VALUE | KEY_SET_VALUE,
    ) {
        Ok(key) => key,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    // Deleting values during enumeration can skip the next entry.
    let records = key.enum_values().collect::<io::Result<Vec<_>>>()?;
    let mut errors = Vec::new();
    for (name, value) in records {
        if let Err(error) = retry_port(&key, &name, &value) {
            errors.push(format!("{error:#}"));
        }
    }
    if !errors.is_empty() {
        bail!(
            "Pending PDF printer port cleanup failed: {}",
            errors.join("; ")
        );
    }
    Ok(())
}

fn retry_port(key: &RegKey, name: &str, value: &RegValue) -> ResultType<()> {
    let port = if name.eq_ignore_ascii_case(OLD_PORT_VALUE) {
        String::from_reg_value(value).context("Invalid saved PDF printer port")?
    } else if name.starts_with(pdf::PIPE_PREFIX) {
        name.to_owned()
    } else {
        return Ok(());
    };
    pdf::remove_unused_port(&port).with_context(|| format!("Removing {port}"))?;
    forget(key, name)
}

fn forget(key: &RegKey, name: &str) -> ResultType<()> {
    match key.delete_value(name) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => Ok(result?),
    }
}
