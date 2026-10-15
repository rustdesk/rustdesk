use super::{installer_shell::run_elevated_and_wait, ResultType};
use hbb_common::{
    anyhow::{anyhow, Context},
    bail,
    config::Config,
    log,
};
use std::{
    ffi::OsString,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use windows_service::{
    define_windows_service,
    service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus},
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

mod operations;

const CONTROL_ARGUMENT: &str = "--native-service-control";
const IMPORT_ARGUMENT: &str = "--native-service-import";

pub(super) fn run(install: bool) -> ResultType<()> {
    let (executable, parameters) = command(install)?;
    let exit_code = run_elevated_and_wait(&executable, &parameters, false)?;
    if exit_code != 0 {
        bail!("Native service control failed with exit code {exit_code}");
    }
    Ok(())
}

fn command(install: bool) -> ResultType<(PathBuf, String)> {
    super::validate_install_app_name(&crate::get_app_name())?;
    let executable = std::env::current_exe()?;
    let action = if install { "install" } else { "uninstall" };
    let parameters = parameters(action, std::process::id(), &Config::file())?;
    Ok((executable, parameters))
}

fn parameters(action: &str, caller_pid: u32, config_path: &Path) -> ResultType<String> {
    let config = config_path
        .to_str()
        .context("Configuration path is not valid Unicode")?;
    if !config_path.is_absolute() || config.contains(['\0', '"']) || config.ends_with('\\') {
        bail!("Invalid service configuration path");
    }
    Ok(format!(
        "{CONTROL_ARGUMENT} {action} {caller_pid} \"{config}\""
    ))
}

pub(crate) fn handle_command(args: &[String]) -> Option<ResultType<()>> {
    match args.first().map(String::as_str) {
        Some(CONTROL_ARGUMENT) => Some(execute_command(args)),
        Some(IMPORT_ARGUMENT) if args.len() == 1 => Some(
            service_dispatcher::start(crate::get_app_name(), ffi_import_service)
                .map_err(Into::into),
        ),
        Some(IMPORT_ARGUMENT) => Some(Err(anyhow!("Invalid service import arguments"))),
        _ => None,
    }
}

fn execute_command(args: &[String]) -> ResultType<()> {
    let [_, action, caller_pid, config_path] = args else {
        bail!("Invalid native service control arguments");
    };
    let install = match action.as_str() {
        "install" => true,
        "uninstall" => false,
        _ => bail!("Unknown native service control action"),
    };
    let caller_pid = caller_pid
        .parse::<u32>()
        .context("Invalid caller process ID")?;
    if caller_pid == 0 || !Path::new(config_path).is_absolute() {
        bail!("Invalid native service control caller or configuration path");
    }
    if !super::is_elevated(None)? {
        bail!("Native service control requires administrator privileges");
    }
    super::validate_install_app_name(&crate::get_app_name())?;
    operations::execute(install, caller_pid, Path::new(config_path))
}

define_windows_service!(ffi_import_service, import_service_main);

fn import_service_main(arguments: Vec<OsString>) {
    if let Err(error) = run_import_service(&arguments) {
        log::error!("Service configuration import failed: {error:#}");
    }
}

fn run_import_service(arguments: &[OsString]) -> ResultType<()> {
    let handle = service_control_handler::register(crate::get_app_name(), |event| match event {
        ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
        _ => ServiceControlHandlerResult::NotImplemented,
    })?;
    let status = ServiceStatus {
        service_type: super::SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::ZERO,
        process_id: None,
    };
    handle.set_service_status(status.clone())?;
    let result = import_configuration(arguments);
    let exit_code = match &result {
        Ok(()) => ServiceExitCode::Win32(0),
        Err(_) => ServiceExitCode::Win32(winapi::shared::winerror::ERROR_GEN_FAILURE),
    };
    handle.set_service_status(ServiceStatus {
        current_state: ServiceState::Stopped,
        exit_code,
        ..status
    })?;
    result
}

fn import_configuration(arguments: &[OsString]) -> ResultType<()> {
    let [_, config_path] = arguments else {
        bail!("Configuration import requires one source path");
    };
    if !Path::new(config_path).is_absolute() {
        bail!("Configuration import requires an absolute source path");
    }
    // The SCM starts this process as SYSTEM. Keep the existing importer in a
    // separate process so the subsequently started service loads fresh config.
    let status = Command::new(super::get_install_info().3)
        .arg("--import-config")
        .arg(config_path)
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
        .status()?;
    if !status.success() {
        bail!("Configuration importer failed: {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_control_uses_current_executable_without_batch_tools() {
        for install in [true, false] {
            let (executable, parameters) =
                command(install).expect("service command should be valid");
            assert_eq!(executable, std::env::current_exe().unwrap());
            assert!(parameters.starts_with(CONTROL_ARGUMENT));
            for batch_tool in ["certutil", "findstr", "cmd.exe", ".bat"] {
                assert!(
                    !parameters.contains(batch_tool),
                    "unexpected tool: {batch_tool}"
                );
            }
        }
    }

    #[test]
    fn service_arguments_preserve_the_configuration_path() {
        use windows::{
            core::PCWSTR,
            Win32::{
                Foundation::{LocalFree, HLOCAL},
                UI::Shell::CommandLineToArgvW,
            },
        };

        let path = Path::new(r"C:\Users\用户 & !value! %name%\RustDesk.toml");
        let parameters = parameters("install", 123, path).expect("path should be accepted");
        let command_line = super::super::wide_string(&format!("rustdesk.exe {parameters}"));
        let mut count = 0;
        let argv = unsafe { CommandLineToArgvW(PCWSTR(command_line.as_ptr()), &mut count) };
        assert!(!argv.is_null());
        let arguments: Vec<String> = unsafe { std::slice::from_raw_parts(argv, count as usize) }
            .iter()
            .map(|argument| unsafe { argument.to_string() }.expect("argument should be Unicode"))
            .collect();
        unsafe { LocalFree(Some(HLOCAL(argv.cast()))) };
        assert_eq!(
            arguments,
            [
                "rustdesk.exe",
                CONTROL_ARGUMENT,
                "install",
                "123",
                path.to_str().unwrap()
            ]
        );
    }
}
