use hbb_common::{bail, ResultType};
use std::{mem::size_of, path::Path};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{Foundation::HWND, Security::WinTrust},
};

pub(super) fn verify_authenticode_signature(path: &Path) -> ResultType<()> {
    let wide_path = HSTRING::from(path);
    let mut file_info = WinTrust::WINTRUST_FILE_INFO {
        cbStruct: size_of::<WinTrust::WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wide_path.as_ptr()),
        ..Default::default()
    };
    let mut trust_data = WinTrust::WINTRUST_DATA {
        cbStruct: size_of::<WinTrust::WINTRUST_DATA>() as u32,
        dwUIChoice: WinTrust::WTD_UI_NONE,
        fdwRevocationChecks: WinTrust::WTD_REVOKE_NONE,
        dwUnionChoice: WinTrust::WTD_CHOICE_FILE,
        Anonymous: WinTrust::WINTRUST_DATA_0 {
            pFile: &mut file_info,
        },
        dwStateAction: WinTrust::WTD_STATEACTION_IGNORE,
        dwProvFlags: WinTrust::WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    };
    let mut action = WinTrust::WINTRUST_ACTION_GENERIC_VERIFY_V2;
    let trust_data_ptr = (&mut trust_data as *mut WinTrust::WINTRUST_DATA).cast();
    let status = unsafe { WinTrust::WinVerifyTrust(HWND::default(), &mut action, trust_data_ptr) };
    if status != 0 {
        bail!(
            "Update file Authenticode verification failed for {}: 0x{:08X}",
            path.display(),
            status as u32
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsigned_authenticode() {
        assert!(verify_authenticode_signature(&std::env::current_exe().unwrap()).is_err());
    }
}
