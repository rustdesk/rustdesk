use super::{super::installer_shell::get_system_executable, ResultType, SERVICE_WAIT_TIMEOUT};
use hbb_common::{
    anyhow::Context,
    bail, config, log,
    sysinfo::{Process, System},
};
use std::{
    fs, io,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{Error, Owned},
    Win32::{
        Foundation::{ERROR_INVALID_PARAMETER, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::{
            Com::CoTaskMemFree,
            Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
        },
        UI::Shell::{FOLDERID_CommonStartup, SHGetKnownFolderPath, KF_FLAG_DEFAULT},
    },
};
use windows_service::{
    service::{
        Service, ServiceAccess, ServiceControlAccept, ServiceErrorControl, ServiceInfo,
        ServiceStartType, ServiceState, ServiceStatus,
    },
    service_manager::{ServiceManager, ServiceManagerAccess},
};

const SERVICE_POLL_INTERVAL: Duration = Duration::from_millis(100);

pub(super) fn execute(install: bool, caller_pid: u32, config_path: &Path) -> ResultType<()> {
    let (_, directory, _, executable) = super::super::get_install_info();
    if install && config::is_outgoing_only() {
        terminate_processes(caller_pid, false)?;
        return create_tray_shortcut(&directory, &executable);
    }
    let access = if install {
        ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE
    } else {
        ServiceManagerAccess::CONNECT
    };
    let manager = ServiceManager::local_computer(None::<&str>, access)?;
    if install {
        if let Err(error) = create_tray_shortcut(&directory, &executable) {
            log::warn!("Service tray shortcut creation failed: {error:#}");
        }
        let service = open_or_create_service(&manager, Path::new(&executable))?;
        stop_service(&service)?;
        if let Err(error) = terminate_processes(caller_pid, false) {
            log::warn!("Service process cleanup failed: {error:#}");
        }
        let imported = super::import::run(config_path);
        if let Err(error) = &imported {
            log::error!("Service configuration import failed: {error:#}");
            if error.is::<super::import::ImportExitUnconfirmed>() {
                return imported;
            }
        }
        service.start::<&str>(&[])?;
        wait_for_state(&service, ServiceState::Running)?;
        imported?;
    } else {
        remove_service(&manager)?;
        if let Err(error) = remove_tray_shortcut() {
            log::warn!("Service tray shortcut removal failed: {error:#}");
        }
        if let Err(error) = terminate_processes(caller_pid, true) {
            log::warn!("Service process cleanup failed: {error:#}");
        }
    }
    Ok(())
}

fn open_or_create_service(manager: &ServiceManager, executable: &Path) -> ResultType<Service> {
    let name = crate::get_app_name();
    let info = ServiceInfo {
        name: name.clone().into(),
        display_name: format!("{name} Service").into(),
        service_type: super::super::SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: executable.to_owned(),
        launch_arguments: vec!["--service".into()],
        dependencies: Vec::new(),
        account_name: Some("LocalSystem".into()),
        account_password: None,
    };
    let access = ServiceAccess::START
        | ServiceAccess::STOP
        | ServiceAccess::QUERY_STATUS
        | ServiceAccess::CHANGE_CONFIG;
    match manager.open_service(&name, access) {
        Ok(service) => {
            service.change_config(&info)?;
            Ok(service)
        }
        Err(windows_service::Error::Winapi(error))
            if error.raw_os_error()
                == Some(winapi::shared::winerror::ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
        {
            Ok(manager.create_service(&info, access)?)
        }
        Err(error) => Err(error.into()),
    }
}

fn remove_service(manager: &ServiceManager) -> ResultType<()> {
    let service = match manager.open_service(
        crate::get_app_name(),
        ServiceAccess::STOP | ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE,
    ) {
        Ok(service) => service,
        Err(windows_service::Error::Winapi(error))
            if error.raw_os_error()
                == Some(winapi::shared::winerror::ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
        {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    stop_service(&service)?;
    Ok(service.delete()?)
}

fn stop_service(service: &Service) -> ResultType<()> {
    let started = Instant::now();
    loop {
        let status = service.query_status()?;
        match status.current_state {
            ServiceState::Stopped => return Ok(()),
            ServiceState::Running | ServiceState::Paused => {
                if !status
                    .controls_accepted
                    .contains(ServiceControlAccept::STOP)
                {
                    bail!(
                        "Service does not accept STOP controls: {:?}",
                        status.current_state
                    );
                }
                service.stop()?;
                wait_for_state(service, ServiceState::Stopped)?;
                return Ok(());
            }
            _ => {}
        }
        if started.elapsed() >= SERVICE_WAIT_TIMEOUT {
            bail!("Service is still transitioning: {:?}", status.current_state);
        }
        thread::sleep(SERVICE_POLL_INTERVAL);
    }
}

fn wait_for_state(service: &Service, expected: ServiceState) -> ResultType<ServiceStatus> {
    let started = Instant::now();
    loop {
        let status = service.query_status()?;
        if status.current_state == expected {
            return Ok(status);
        }
        if status.current_state == ServiceState::Stopped
            || started.elapsed() >= SERVICE_WAIT_TIMEOUT
        {
            bail!(
                "Service did not reach {expected:?}: state={:?}, exit={:?}",
                status.current_state,
                status.exit_code
            );
        }
        thread::sleep(SERVICE_POLL_INTERVAL);
    }
}

fn tray_shortcut_path() -> ResultType<PathBuf> {
    let folder = unsafe { SHGetKnownFolderPath(&FOLDERID_CommonStartup, KF_FLAG_DEFAULT, None) }?;
    let path = unsafe { folder.to_string() };
    unsafe { CoTaskMemFree(Some(folder.0.cast())) };
    Ok(PathBuf::from(path?).join(format!("{} Tray.lnk", crate::get_app_name())))
}

fn create_tray_shortcut(directory: &str, executable: &str) -> ResultType<()> {
    let icon = super::super::get_custom_icon(directory, executable);
    let shortcut =
        super::super::installer_shell::shortcut_bytes(executable, Some("--tray"), icon.as_deref())?;
    fs::write(tray_shortcut_path()?, shortcut).context("Failed to write the service tray shortcut")
}

fn remove_tray_shortcut() -> ResultType<()> {
    match fs::remove_file(tray_shortcut_path()?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Failed to remove the service tray shortcut"),
    }
}

fn terminate_processes(caller_pid: u32, include_broker: bool) -> ResultType<()> {
    let application = format!("{}.exe", crate::get_app_name());
    let system = System::new_all();
    for (pid, process) in system.processes() {
        if [caller_pid, std::process::id()].contains(&pid.as_u32()) {
            continue;
        }
        let matches = process.name().eq_ignore_ascii_case(&application)
            || (include_broker
                && process
                    .name()
                    .eq_ignore_ascii_case(super::super::WIN_TOPMOST_INJECTED_PROCESS_EXE));
        if matches {
            terminate_process(process)?;
        }
    }
    Ok(())
}

fn terminate_process(process: &Process) -> ResultType<()> {
    let mut command = termination_command(process.pid().as_u32())?;
    if !command.output()?.status.success() && !process_has_exited(process.pid().as_u32())? {
        bail!(
            "Failed to terminate {} (PID {})",
            process.name(),
            process.pid()
        );
    }
    Ok(())
}

fn termination_command(pid: u32) -> ResultType<Command> {
    let mut command = Command::new(get_system_executable("taskkill.exe")?);
    command
        .args(["/PID", &pid.to_string(), "/F"])
        .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW);
    Ok(command)
}

fn process_has_exited(pid: u32) -> ResultType<bool> {
    const DO_NOT_WAIT: u32 = 0;
    let handle = match unsafe { OpenProcess(PROCESS_SYNCHRONIZE, false, pid) } {
        Ok(handle) => unsafe { Owned::new(handle) },
        Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => return Ok(true),
        Err(error) => return Err(error.into()),
    };
    match unsafe { WaitForSingleObject(*handle, DO_NOT_WAIT) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(Error::from_win32().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::sysinfo::Pid;
    use std::{
        os::windows::process::CommandExt,
        process::{Command, Stdio},
    };

    #[test]
    fn cleanup_uses_the_system_taskkill_executable() {
        let command = termination_command(std::process::id()).unwrap();
        let expected = get_system_executable("taskkill.exe").unwrap();
        assert_eq!(command.get_program(), expected.as_os_str());
    }

    #[test]
    fn cleanup_accepts_a_process_that_exited_after_the_snapshot() {
        let command =
            crate::platform::windows::installer_shell::get_system_executable("cmd.exe").unwrap();
        let mut child = Command::new(command)
            .args(["/d", "/q"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(winapi::um::winbase::CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        let pid = Pid::from_u32(child.id());
        let mut system = System::new();
        let captured = system.refresh_process(pid);
        assert!(!process_has_exited(pid.as_u32()).unwrap());
        drop(child.stdin.take());
        assert!(child.wait().unwrap().success());
        assert!(captured);
        terminate_process(system.process(pid).unwrap()).unwrap();
    }
}
