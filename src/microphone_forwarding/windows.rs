use hbb_common::{
    anyhow::anyhow,
    base64::{engine::general_purpose::STANDARD, Engine},
    ResultType,
};
use std::process::Command;
fn powershell(script: &str) -> ResultType<String> {
    let utf16: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let result = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-EncodedCommand",
            &STANDARD.encode(utf16),
        ])
        .output()?;
    if !result.status.success() {
        return Err(anyhow!(
            "Install VB-CABLE and AudioDeviceCmdlets: {}",
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    Ok(String::from_utf8(result.stdout)?.trim().to_owned())
}
pub struct Route {
    previous: String,
    previous_communication: String,
    target: String,
    restored: bool,
}
impl Route {
    pub fn open() -> ResultType<Self> {
        let previous = powershell("$ErrorActionPreference='Stop'; Import-Module AudioDeviceCmdlets; (Get-AudioDevice -Recording).ID")?;
        let previous_communication = powershell("$ErrorActionPreference='Stop'; Import-Module AudioDeviceCmdlets; (Get-AudioDevice -RecordingCommunication).ID")?;
        let target = powershell("$ErrorActionPreference='Stop'; Import-Module AudioDeviceCmdlets; $d=@(Get-AudioDevice -List | Where-Object { $_.Type -eq 'Recording' -and $_.Name -like 'CABLE Output*' }); if ($d.Count -ne 1) { throw 'Exactly one VB-CABLE recording endpoint is required' }; $d[0].ID")?;
        powershell(&format!("$ErrorActionPreference='Stop'; Import-Module AudioDeviceCmdlets; Set-AudioDevice -ID '{}' | Out-Null", target.replace('\'', "''")))?;
        Ok(Self {
            previous,
            previous_communication,
            target,
            restored: false,
        })
    }
    pub fn output(&self) -> &str {
        "CABLE Input"
    }
    pub fn restore(&mut self) {
        if self.restored {
            return;
        }
        let script = format!("$ErrorActionPreference='Stop'; Import-Module AudioDeviceCmdlets; if ((Get-AudioDevice -Recording).ID -eq '{}') {{ Set-AudioDevice -ID '{}' -DefaultOnly | Out-Null }}; if ((Get-AudioDevice -RecordingCommunication).ID -eq '{}') {{ Set-AudioDevice -ID '{}' -CommunicationOnly | Out-Null }}", self.target.replace('\'', "''"), self.previous.replace('\'', "''"), self.target.replace('\'', "''"), self.previous_communication.replace('\'', "''"));
        if let Err(error) = powershell(&script) {
            hbb_common::log::warn!("{error}");
        }
        self.restored = true;
    }
}
