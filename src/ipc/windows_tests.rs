use base::platform::windows::RAIIHandle;
use hbb_common::tokio::{
    self,
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::ServerOptions,
    time::{timeout, Duration},
};
use parity_tokio_ipc::{Endpoint, Incoming, SecurityAttributes};
use windows::Win32::{
    Foundation::{ERROR_ACCESS_DENIED, HANDLE},
    Security::{
        CreateRestrictedToken, CreateWellKnownSid, ImpersonateLoggedOnUser, RevertToSelf,
        WinWorldSid, DISABLE_MAX_PRIVILEGE, PSID, SECURITY_MAX_SID_SIZE, SID_AND_ATTRIBUTES,
        TOKEN_DUPLICATE, TOKEN_QUERY,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

const TEST_TIMEOUT: Duration = Duration::from_secs(2);
const MESSAGE: &[u8] = b"ipc-regression";

fn listener(attributes: SecurityAttributes) -> (String, Incoming) {
    let path = parity_tokio_ipc::dummy_endpoint();
    let mut endpoint = Endpoint::new(path.clone());
    endpoint.set_security_attributes(attributes);
    (path, endpoint.incoming().unwrap())
}

fn as_untrusted_client<T>(operation: impl FnOnce() -> T) -> T {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY,
            &mut token,
        )
        .unwrap();
        let _token = RAIIHandle(token.0.cast());
        let mut sid = [0u8; SECURITY_MAX_SID_SIZE as usize];
        let mut size = sid.len() as u32;
        let sid = PSID(sid.as_mut_ptr().cast());
        CreateWellKnownSid(WinWorldSid, None, Some(sid), &mut size).unwrap();
        let mut restricted = HANDLE::default();
        let restrictions = [SID_AND_ATTRIBUTES {
            Sid: sid,
            Attributes: 0,
        }];
        CreateRestrictedToken(
            token,
            DISABLE_MAX_PRIVILEGE,
            None,
            None,
            Some(&restrictions),
            &mut restricted,
        )
        .unwrap();
        let _restricted = RAIIHandle(restricted.0.cast());
        ImpersonateLoggedOnUser(restricted).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation));
        RevertToSelf().unwrap();
        match result {
            Ok(result) => result,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
}

#[tokio::test]
async fn windows_ipc_rejects_additional_untrusted_instances() {
    let (path, _incoming) = listener(super::listener_security_attributes().unwrap());
    let result = as_untrusted_client(|| ServerOptions::new().create(&path));
    assert_eq!(
        result
            .expect_err("untrusted clients must not create server instances")
            .raw_os_error(),
        Some(ERROR_ACCESS_DENIED.0 as i32),
    );
}

#[tokio::test]
async fn windows_ipc_rejects_untrusted_server_before_sending() {
    timeout(TEST_TIMEOUT, async {
        {
            let (path, _incoming) = listener(super::listener_security_attributes().unwrap());
            let result = super::connect(&path, crate::POSTFIX_SERVICE).await;
            assert_eq!(
                result.is_ok(),
                crate::platform::windows::is_root(),
                "restrictive permissions alone must not authenticate a non-SYSTEM owner"
            );
        }
        for postfix in [crate::POSTFIX_SERVICE, ""] {
            let owner = crate::platform::windows::current_process_user_sid_string().unwrap();
            let attributes =
                SecurityAttributes::from_sddl(&format!("O:{owner}D:P(A;;GA;;;WD)")).unwrap();
            let (path, mut incoming) = listener(attributes);
            let result = super::connect(&path, postfix).await;
            let error =
                result.expect_err("a rogue server must not receive a usable client connection");
            if postfix.is_empty() {
                assert_eq!(
                    error.to_string(),
                    "IPC server grants untrusted users more than client access"
                );
            }
            let mut server = incoming.next().await.unwrap().unwrap();
            let mut received = [0u8; 1];
            match server.read(&mut received).await {
                Ok(length) => {
                    assert_eq!(length, 0, "authentication must not send application data")
                }
                Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe),
            }
        }
    })
    .await
    .expect("Windows IPC server rejection test timed out");
}

#[tokio::test]
async fn windows_ipc_allows_client_communication() {
    timeout(TEST_TIMEOUT, async {
        let (path, mut incoming) = listener(super::listener_security_attributes().unwrap());
        let mut client = as_untrusted_client(|| super::open_pipe(&path)).unwrap();
        let mut server = incoming.next().await.unwrap().unwrap();
        client.write_all(MESSAGE).await.unwrap();
        let mut received = vec![0u8; MESSAGE.len()];
        server.read_exact(&mut received).await.unwrap();
        server.write_all(&received).await.unwrap();
        let mut reply = vec![0u8; MESSAGE.len()];
        client.read_exact(&mut reply).await.unwrap();
        assert_eq!(reply, MESSAGE);
        assert!(super::connect(&path, "").await.is_ok());
    })
    .await
    .expect("Windows IPC client communication test timed out");
}
