//! Shell and command-path helpers.
//!
//! These live here rather than with the rest of the platform code because
//! `config::patch()` resolves a home directory through them.

use crate::ResultType;
use std::process::Command;

// to-do: There seems to be some runtime issue that causes the audit logs to be generated.
// We may need to fix this and remove this workaround in the future.
//
// We use the pre-search method to find the command path to avoid the audit logs on some systems.
// No idea why the audit logs happen.
// Though the audit logs may disappear after rebooting.
//
// See https://github.com/rustdesk/rustdesk/discussions/11959
//
// `ausearch -x /usr/share/rustdesk/rustdesk` will return
// ...
// time->Tue Jun 24 10:40:43 2025
// type=PROCTITLE msg=audit(1750776043.446:192757): proctitle=2F7573722F62696E2F727573746465736B002D2D73657276696365
// type=PATH msg=audit(1750776043.446:192757): item=0 name="/usr/local/bin/sh" nametype=UNKNOWN cap_fp=0 cap_fi=0 cap_fe=0 cap_fver=0 cap_frootid=0
// type=CWD msg=audit(1750776043.446:192757): cwd="/"
// type=SYSCALL msg=audit(1750776043.446:192757): arch=c000003e syscall=59 success=no exit=-2 a0=7fb7dbd22da0 a1=1d65f2c0 a2=7ffc25193360 a3=7ffc25194ec0 items=1 ppid=172208 pid=267565 auid=4294967295 uid=0 gid=0 euid=0 suid=0 fsuid=0 egid=0 sgid=0 fsgid=0 tty=(none) ses=4294967295 comm="rustdesk" exe="/usr/share/rustdesk/rustdesk" subj=unconfined key="processos_criados"
// ----
// time->Tue Jun 24 10:40:43 2025
// type=PROCTITLE msg=audit(1750776043.446:192758): proctitle=2F7573722F62696E2F727573746465736B002D2D73657276696365
// type=PATH msg=audit(1750776043.446:192758): item=0 name="/usr/sbin/sh" nametype=UNKNOWN cap_fp=0 cap_fi=0 cap_fe=0 cap_fver=0 cap_frootid=0
// ...
lazy_static::lazy_static! {
    pub static ref CMD_LOGINCTL: String = find_cmd_path("loginctl");
    pub static ref CMD_PS: String = find_cmd_path("ps");
    pub static ref CMD_SH: String = find_cmd_path("sh");
}

fn find_cmd_path(cmd: &'static str) -> String {
    let test_cmd = format!("/bin/{}", cmd);
    if std::path::Path::new(&test_cmd).exists() {
        return test_cmd;
    }
    let test_cmd = format!("/usr/bin/{}", cmd);
    if std::path::Path::new(&test_cmd).exists() {
        return test_cmd;
    }
    if let Ok(output) = Command::new("which").arg(cmd).output() {
        if output.status.success() {
            return String::from_utf8_lossy(&output.stdout).trim().to_string();
        }
    }
    cmd.to_string()
}

pub fn run_cmds_trim_newline(cmds: &str) -> ResultType<String> {
    let output = std::process::Command::new(CMD_SH.as_str())
        .args(vec!["-c", cmds])
        .output()?;
    let out = String::from_utf8_lossy(&output.stdout);
    Ok(if out.ends_with('\n') {
        out[..out.len() - 1].to_string()
    } else {
        out.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // `config::patch()` resolves the root home directory through this, so the
    // trailing-newline strip has to keep working in this crate on its own.
    #[test]
    fn trims_one_trailing_newline() {
        assert_eq!(run_cmds_trim_newline("printf 123").unwrap(), "123");
        assert_eq!(run_cmds_trim_newline("echo 123").unwrap(), "123");
        assert_eq!(run_cmds_trim_newline("printf '1\\n\\n'").unwrap(), "1\n");
        assert_eq!(run_cmds_trim_newline("printf ''").unwrap(), "");
    }
}
