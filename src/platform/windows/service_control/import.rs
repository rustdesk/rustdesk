use super::{ResultType, SERVICE_WAIT_TIMEOUT};
use hbb_common::{anyhow::Context, bail, log};
use std::{
    ffi::OsStr,
    fmt,
    os::windows::ffi::OsStrExt,
    path::Path,
    process::Command,
    thread,
    time::{Duration, Instant},
};
use windows::{
    core::{Interface, BSTR},
    Win32::{
        Foundation::RPC_E_CHANGED_MODE,
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            TaskScheduler::{
                IExecAction, IRegisteredTask, ITaskDefinition, ITaskService, TaskScheduler,
                TASK_ACTION_EXEC, TASK_CREATE, TASK_LOGON_SERVICE_ACCOUNT, TASK_STATE_QUEUED,
                TASK_STATE_READY, TASK_STATE_RUNNING,
            },
            Variant::VARIANT,
        },
    },
};

const IMPORT_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug)]
pub(super) struct ImportExitUnconfirmed;

impl fmt::Display for ImportExitUnconfirmed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Configuration importer exit is unconfirmed; service remains stopped")
    }
}

impl std::error::Error for ImportExitUnconfirmed {}

pub(super) fn run(config_path: &Path) -> ResultType<()> {
    let mut command = super::system_import_command()?;
    command.arg(config_path);
    let arguments = command
        .get_args()
        .map(quoted)
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    let _com = initialize_com()?;
    let scheduler: ITaskService =
        unsafe { CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER) }?;
    let empty = VARIANT::default();
    unsafe { scheduler.Connect(&empty, &empty, &empty, &empty) }?;
    let definition = task_definition(&scheduler, &command)?;
    let folder = unsafe { scheduler.GetFolder(&BSTR::from("\\")) }?;
    let name = BSTR::from(format!(
        "{}-ConfigImport-{}",
        crate::get_app_name(),
        uuid::Uuid::new_v4().simple()
    ));
    let task = unsafe {
        folder.RegisterTaskDefinition(
            &name,
            &definition,
            TASK_CREATE.0,
            &VARIANT::from("SYSTEM"),
            &empty,
            TASK_LOGON_SERVICE_ACCOUNT,
            &VARIANT::from("O:BAG:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)"),
        )
    }?;
    let result = run_task(&task, &arguments);
    if let Err(error) = unsafe { folder.DeleteTask(&name, 0) } {
        log::error!("Failed to remove configuration import task {name}: {error}");
        if result.is_ok() {
            return Err(error.into());
        }
    }
    result
}

fn run_task(task: &IRegisteredTask, arguments: &str) -> ResultType<()> {
    let _running = unsafe { task.Run(&VARIANT::from(arguments)) }?;
    if let Err(error) = wait_for_exit(task) {
        unsafe { task.Stop(0) }.context(ImportExitUnconfirmed)?;
        wait_for_exit(task).context(ImportExitUnconfirmed)?;
        return Err(error);
    }
    let exit_code = unsafe { task.LastTaskResult() }?;
    if exit_code != 0 {
        bail!("Configuration importer failed with exit code {exit_code:#010x}");
    }
    Ok(())
}

fn wait_for_exit(task: &IRegisteredTask) -> ResultType<()> {
    let started = Instant::now();
    loop {
        match unsafe { task.State() }? {
            TASK_STATE_READY => return Ok(()),
            TASK_STATE_RUNNING | TASK_STATE_QUEUED => {}
            state => bail!("Unexpected configuration import task state: {state:?}"),
        }
        if started.elapsed() >= SERVICE_WAIT_TIMEOUT {
            bail!("Configuration import timed out");
        }
        thread::sleep(IMPORT_POLL_INTERVAL);
    }
}

fn task_definition(scheduler: &ITaskService, command: &Command) -> ResultType<ITaskDefinition> {
    let executable = bstr(command.get_program());
    let directory = bstr(
        command
            .get_current_dir()
            .context("Missing import directory")?
            .as_os_str(),
    );
    let definition = unsafe { scheduler.NewTask(0) }?;
    unsafe {
        let principal = definition.Principal()?;
        principal.SetUserId(&BSTR::from("SYSTEM"))?;
        principal.SetLogonType(TASK_LOGON_SERVICE_ACCOUNT)?;
        let settings = definition.Settings()?;
        settings.SetAllowDemandStart(true.into())?;
        settings.SetDisallowStartIfOnBatteries(false.into())?;
        settings.SetStopIfGoingOnBatteries(false.into())?;
        settings.SetAllowHardTerminate(true.into())?;
        let action: IExecAction = definition.Actions()?.Create(TASK_ACTION_EXEC)?.cast()?;
        action.SetPath(&executable)?;
        // Supply arguments at Run time so user paths are not stored in the task definition.
        action.SetArguments(&BSTR::from("$(Arg0)"))?;
        action.SetWorkingDirectory(&directory)?;
    }
    Ok(definition)
}

struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn initialize_com() -> ResultType<Option<ComGuard>> {
    let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if result == RPC_E_CHANGED_MODE {
        return Ok(None);
    }
    result.ok()?;
    Ok(Some(ComGuard))
}

fn quoted(value: &OsStr) -> ResultType<String> {
    let value = value
        .to_str()
        .context("Import argument is not valid Unicode")?;
    if value.contains(['\0', '"']) || value.ends_with('\\') {
        bail!("Invalid configuration import argument");
    }
    Ok(format!("\"{value}\""))
}

fn bstr(value: &OsStr) -> BSTR {
    BSTR::from_wide(&value.encode_wide().collect::<Vec<_>>())
}
