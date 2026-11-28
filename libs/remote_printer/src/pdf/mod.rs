use hbb_common::{bail, ResultType};

mod handles;
mod identity;
mod install;
mod jobs;
mod namespace;
mod pipe;
mod ports;
mod printing;
mod queues;
mod raster;
mod security;

pub use identity::{controlled_user_sid, create_private_directory};
pub use install::{install, uninstall};
pub use jobs::Job;
pub use namespace::verify_namespace;
pub use pipe::Pipe;
pub use printing::{available, print_file, printer_names};
pub use queues::{installed, is_installed, is_remote_queue};
pub use security::is_elevated;

pub const DRIVER_NAME: &str = "Microsoft Print To PDF";
pub const PIPE_PREFIX: &str = r"\\.\pipe\ProtectedPrefix\Administrators\RustDesk-PdfPrinter-";
pub(crate) const OWNER_PREFIX: &str = "RustDesk PDF printer v1:";
const INSTANCE_ID_LENGTH: usize = 32;

#[derive(Clone, Debug)]
pub struct Queue {
    pub name: String,
    pub port: String,
}

pub fn remove_unused_port(port: &str) -> ResultType<()> {
    let Some(identifier) = port.strip_prefix(PIPE_PREFIX) else {
        bail!("The saved PDF port is outside the protected printer namespace");
    };
    port_name(identifier)?;
    ports::remove(port)
}

pub(crate) fn display_name(app_name: &str) -> ResultType<String> {
    validate_app_name(app_name)?;
    Ok(format!("{app_name} Printer"))
}

pub(crate) fn owner_marker(app_name: &str) -> ResultType<String> {
    validate_app_name(app_name)?;
    Ok(format!("{OWNER_PREFIX}{app_name}"))
}

pub(crate) fn port_name(instance_id: &str) -> ResultType<String> {
    if instance_id.len() != INSTANCE_ID_LENGTH
        || !instance_id.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("Invalid PDF printer instance identifier");
    }
    Ok(format!("{PIPE_PREFIX}{instance_id}"))
}

fn validate_app_name(app_name: &str) -> ResultType<()> {
    if app_name.is_empty() || app_name.contains('\0') {
        bail!("Invalid PDF printer application name");
    }
    Ok(())
}
