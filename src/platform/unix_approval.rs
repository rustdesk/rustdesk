use hbb_common::{
    anyhow::{anyhow, ensure},
    ResultType,
};
use std::process::Command;

pub(crate) fn request(command: &str, target: u32, digest: &[u8; 32]) -> ResultType<()> {
    let exe = std::env::current_exe()?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow!("Invalid executable path"))?;
    let target = target.to_string();
    let digest = hex::encode(digest);
    #[cfg(target_os = "linux")]
    let status = Command::new(exe)
        .args(["-gtk-sudo", "credential", exe, command, &target, &digest])
        .status()?;
    #[cfg(target_os = "macos")]
    const SCRIPT: &str = r#"on run argv
        set cmd to quoted form of (item 1 of argv)
        repeat with arg in rest of argv
            set cmd to cmd & " " & quoted form of (contents of arg)
        end repeat
        do shell script cmd with administrator privileges
    end run"#;
    #[cfg(target_os = "macos")]
    let status = Command::new("/usr/bin/osascript")
        .args(["-e", SCRIPT, exe, command, &target, &digest])
        .status()?;
    ensure!(status.success(), "Authentication Required");
    Ok(())
}
