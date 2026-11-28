use super::{handles::Handle, pipe, security::Descriptor};
use hbb_common::{anyhow::anyhow, bail, ResultType};
use windows::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, HANDLE},
    Security::{
        CreateRestrictedToken, CreateWellKnownSid, ImpersonateLoggedOnUser, RevertToSelf,
        WinWorldSid, DISABLE_MAX_PRIVILEGE, PSID, SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES,
        TOKEN_DUPLICATE, TOKEN_QUERY,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

pub fn verify_namespace(instance_id: &str) -> ResultType<()> {
    let protected = super::port_name(instance_id)?;
    std::thread::spawn(move || restricted_probe(&protected))
        .join()
        .map_err(|_| anyhow!("The protected pipe namespace probe thread failed"))?
}

fn restricted_probe(protected: &str) -> ResultType<()> {
    let token = restricted_token()?;
    unsafe { ImpersonateLoggedOnUser(token.0)? };
    let result = check_creation(protected);
    // This dedicated thread exits even if reverting fails; it never performs other service work.
    unsafe { RevertToSelf()? };
    result
}

fn check_creation(protected: &str) -> ResultType<()> {
    let descriptor = Descriptor::new("D:P(A;;GA;;;WD)")?;
    let identifier = protected
        .strip_prefix(super::PIPE_PREFIX)
        .ok_or_else(|| anyhow!("Invalid protected pipe test name"))?;
    let control = pipe::create(
        &format!(r"\\.\pipe\RustDesk-PdfNamespaceProbe-{identifier}"),
        &descriptor,
    )?;
    drop(control);
    match pipe::create(protected, &descriptor) {
        Err(error)
            if error
                .downcast_ref::<windows::core::Error>()
                .is_some_and(|error| error.code() == ERROR_ACCESS_DENIED.to_hresult()) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
        Ok(_) => {
            bail!("Windows did not protect the PDF pipe namespace from an unprivileged creator")
        }
    }
}

fn restricted_token() -> ResultType<Handle> {
    let mut original = HANDLE::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY | TOKEN_DUPLICATE,
            &mut original,
        )?
    };
    let original = Handle(original);
    let mut sid = [0u8; SECURITY_MAX_SID_SIZE as usize];
    let mut length = sid.len() as u32;
    let sid = PSID(sid.as_mut_ptr().cast());
    unsafe { CreateWellKnownSid(WinWorldSid, None, Some(sid), &mut length)? };
    let restriction = SID_AND_ATTRIBUTES {
        Sid: sid,
        Attributes: 0,
    };
    let mut token = HANDLE::default();
    unsafe {
        CreateRestrictedToken(
            original.0,
            DISABLE_MAX_PRIVILEGE,
            None,
            None,
            Some(&[restriction]),
            &mut token,
        )?
    };
    Ok(Handle(token))
}

#[cfg(test)]
mod tests {
    #[test]
    fn protected_namespace_rejects_an_unprivileged_creator() {
        let instance = format!("{:032x}", hbb_common::rand::random::<u128>());
        super::verify_namespace(&instance).unwrap();
    }
}
