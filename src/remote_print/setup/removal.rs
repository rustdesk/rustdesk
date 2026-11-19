use hbb_common::ResultType;
use winreg::{
    enums::{HKEY_LOCAL_MACHINE, KEY_SET_VALUE},
    RegKey,
};

const REMOVAL_PORT: &str = "PDFPRINTER_REMOVAL_PORT";

pub(super) fn uninstall() -> ResultType<()> {
    let app = crate::get_app_name();
    let root = RegKey::predef(HKEY_LOCAL_MACHINE);
    let path = super::marker_key(&app);
    let pending = pending_port(&root, &path)?;
    let queue = remote_printer::pdf::installed(&app)?;
    if let Some(port) = &pending {
        if queue.as_ref().map(|queue| queue.port.as_str()) != Some(port.as_str()) {
            remote_printer::pdf::remove_unused_port(port)?;
        }
    }
    if let Some(queue) = queue {
        // DeletePrinter can remove the queue before its port is released.
        let (key, _) = root.create_subkey(&path)?;
        key.set_value(REMOVAL_PORT, &queue.port)?;
    }
    remote_printer::pdf::uninstall(&app)?;
    let result = root
        .open_subkey_with_flags(&path, KEY_SET_VALUE)
        .and_then(|key| key.delete_value(REMOVAL_PORT));
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn pending_port(root: &RegKey, path: &str) -> ResultType<Option<String>> {
    match root
        .open_subkey(path)
        .and_then(|key| key.get_value(REMOVAL_PORT))
    {
        Ok(port) => Ok(Some(port)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
