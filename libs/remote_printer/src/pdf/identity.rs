use super::{
    handles::{check_size, Buffer, Handle},
    queues,
    security::Descriptor,
};
use hbb_common::{anyhow::Context, bail, ResultType};
use std::path::Path;
use windows::{
    core::{HSTRING, PCWSTR, PWSTR},
    Win32::{
        Foundation::{LocalFree, HANDLE, HLOCAL},
        Security::{
            Authorization::ConvertSidToStringSidW, GetTokenInformation, LookupAccountNameW,
            TokenUser, PSID, SID_NAME_USE, TOKEN_QUERY, TOKEN_USER,
        },
        Storage::FileSystem::CreateDirectoryW,
        System::{
            RemoteDesktop::{
                ProcessIdToSessionId, WTSDomainName, WTSFreeMemory, WTSQuerySessionInformationW,
                WTSUserName, WTS_INFO_CLASS,
            },
            Threading::{GetCurrentProcess, GetCurrentProcessId, OpenProcessToken},
        },
    },
};

pub fn controlled_user_sid() -> ResultType<String> {
    let mut session = 0;
    // The privileged server runs in the selected desktop session, including RDP.
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session)? };
    let user = session_text(session, WTSUserName)?;
    let domain = session_text(session, WTSDomainName)?;
    if user.is_empty() || domain.is_empty() {
        bail!("The controlled Windows desktop has no identifiable signed-in user");
    }
    account_sid(&format!(r"{domain}\{user}"))
}

fn session_text(session: u32, class: WTS_INFO_CLASS) -> ResultType<String> {
    let mut buffer = PWSTR::null();
    let mut length = 0;
    unsafe { WTSQuerySessionInformationW(None, session, class, &mut buffer, &mut length)? };
    let result = queues::text(buffer);
    unsafe { WTSFreeMemory(buffer.0.cast()) };
    result
}

pub(crate) fn account_sid(account: &str) -> ResultType<String> {
    if account.is_empty() || account.contains('\0') {
        bail!("Invalid print job account");
    }
    let account = HSTRING::from(account);
    let mut sid_length = 0;
    let mut domain_length = 0;
    let mut kind = SID_NAME_USE::default();
    check_size(unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            PCWSTR(account.as_ptr()),
            None,
            &mut sid_length,
            None,
            &mut domain_length,
            &mut kind,
        )
    })?;
    let mut sid = Buffer::new(sid_length)?;
    let mut domain = vec![0u16; domain_length as usize];
    let sid = PSID(sid.bytes().as_mut_ptr().cast());
    unsafe {
        LookupAccountNameW(
            PCWSTR::null(),
            PCWSTR(account.as_ptr()),
            Some(sid),
            &mut sid_length,
            Some(PWSTR(domain.as_mut_ptr())),
            &mut domain_length,
            &mut kind,
        )?
    };
    sid_text(sid)
}

fn current_user_sid() -> ResultType<String> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
    let token = Handle(token);
    let buffer = Buffer::query(|buffer, length| unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.map(|bytes| bytes.as_mut_ptr().cast()),
            *length,
            length,
        )
    })?
    .context("The print API returned an empty buffer size")?;
    sid_text(unsafe { buffer.item::<TOKEN_USER>(0)? }.User.Sid)
}

pub(crate) fn require_printing_user() -> ResultType<()> {
    const LOCAL_SYSTEM_SID: &str = "S-1-5-18";
    if current_user_sid()? == LOCAL_SYSTEM_SID {
        bail!("Received PDFs must be rendered by the controller user, not LocalSystem");
    }
    Ok(())
}

fn sid_text(sid: PSID) -> ResultType<String> {
    let mut value = PWSTR::null();
    unsafe { ConvertSidToStringSidW(sid, &mut value)? };
    let result = queues::text(value);
    let remaining = unsafe { LocalFree(Some(HLOCAL(value.0.cast()))) };
    if !remaining.is_invalid() {
        bail!("Failed to release the Windows SID string");
    }
    result
}

pub fn create_private_directory(path: &Path) -> ResultType<()> {
    let sid = current_user_sid()?;
    let descriptor = Descriptor::new(&format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})"))?;
    let path = HSTRING::from(path.as_os_str());
    unsafe { CreateDirectoryW(PCWSTR(path.as_ptr()), Some(&descriptor.attributes()))? };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Run as the signed-in desktop user; use an RDP session for non-console regression coverage"]
    fn controlled_user_matches_session_user() {
        assert_eq!(controlled_user_sid().unwrap(), current_user_sid().unwrap());
    }
}
