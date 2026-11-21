use super::handles::Handle;
use hbb_common::ResultType;
use std::{mem::size_of, time::Duration};
use windows::{
    core::{HSTRING, PCWSTR},
    Win32::{
        Foundation::{LocalFree, HANDLE, HLOCAL},
        Security::{
            Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            GetTokenInformation, TokenElevation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES,
            TOKEN_ELEVATION, TOKEN_QUERY,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

pub(crate) struct Descriptor(PSECURITY_DESCRIPTOR);

impl Descriptor {
    pub(crate) fn new(sddl: &str) -> ResultType<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let sddl = HSTRING::from(sddl);
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )?;
        }
        Ok(Self(descriptor))
    }

    pub(crate) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.0 .0,
            bInheritHandle: false.into(),
        }
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        if !unsafe { LocalFree(Some(HLOCAL(self.0 .0))) }.is_invalid() {
            hbb_common::throttled_log!(
                Duration::from_secs(60),
                warn,
                "PDF security descriptor cleanup failed"
            );
        }
    }
}

pub fn is_elevated() -> ResultType<bool> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)? };
    let token = Handle(token);
    let mut elevation = TOKEN_ELEVATION::default();
    let mut length = 0;
    unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        )?;
    }
    Ok(elevation.TokenIsElevated != 0)
}
