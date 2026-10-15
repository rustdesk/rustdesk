use super::{installer_shell::run_elevated_and_wait, ResultType};
use hbb_common::{
    anyhow::Context,
    bail,
    config::{self, Config},
    log,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use windows_service::{
    service::{ServiceAccess, ServiceState},
    service_manager::{ServiceManager, ServiceManagerAccess},
};

mod import;
mod operations;

const CONTROL_ARGUMENT: &str = "--native-service-control";
const SERVICE_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn run(install: bool) -> ResultType<()> {
    let (executable, parameters) = command(install)?;
    let exit_code = run_elevated_and_wait(&executable, &parameters, false)?;
    if exit_code != 0 {
        bail!("Native service control failed with exit code {exit_code}");
    }
    Ok(())
}

pub(super) fn restore_stop_service(install: bool) {
    let stopped = if config::is_outgoing_only() {
        Ok(install)
    } else {
        is_stopped()
    };
    match stopped {
        Ok(stopped) => {
            Config::set_option("stop-service".into(), if stopped { "Y" } else { "" }.into())
        }
        Err(error) => log::error!("Failed to synchronize service state after an error: {error:#}"),
    }
}

fn is_stopped() -> ResultType<bool> {
    let manager = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)?;
    match manager.open_service(crate::get_app_name(), ServiceAccess::QUERY_STATUS) {
        Ok(service) => Ok(matches!(
            service.query_status()?.current_state,
            ServiceState::Stopped | ServiceState::StopPending
        )),
        Err(windows_service::Error::Winapi(error))
            if matches!(error.raw_os_error(), Some(code) if
                code == winapi::shared::winerror::ERROR_SERVICE_DOES_NOT_EXIST as i32 ||
                code == winapi::shared::winerror::ERROR_SERVICE_MARKED_FOR_DELETE as i32) =>
        {
            Ok(true)
        }
        Err(error) => Err(error.into()),
    }
}

fn command(install: bool) -> ResultType<(PathBuf, String)> {
    super::validate_install_app_name(&crate::get_app_name())?;
    let executable = std::env::current_exe()?;
    let action = if install { "install" } else { "uninstall" };
    let parameters = parameters(action, std::process::id(), &Config::file())?;
    Ok((executable, parameters))
}

fn system_import_command() -> ResultType<Command> {
    let executable = PathBuf::from(super::get_install_info().3);
    let directory = executable
        .parent()
        .context("Missing installation directory")?;
    let mut command = Command::new(&executable);
    command.arg("--import-config").current_dir(directory);
    Ok(command)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_import_uses_the_installed_legacy_command() {
        let command = system_import_command().unwrap();
        let installed = PathBuf::from(super::super::get_install_info().3);
        assert_eq!(command.get_program(), installed.as_os_str());
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["--import-config"]);
    }

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
