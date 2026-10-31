use super::{connect, ConnectionTmpl, Data};
use hbb_common::{
    bail,
    config::Config,
    tokio::io::{AsyncRead, AsyncWrite},
    ResultType,
};
use std::collections::HashMap;

pub async fn set_option_from_cli(key: &str, value: &str) -> ResultType<()> {
    let _nat = crate::common::CheckTestNatType::new();
    let mut connection = connect(1_000, "").await?;
    let (options, _security_changed) = write_option(&mut connection, key, value).await?;
    #[cfg(target_os = "windows")]
    {
        if _security_changed || crate::trusted_devices::config::changes_authentication(&options) {
            sync_security_config(&mut connection).await?;
        }
        if !crate::trusted_devices::config::SECURITY_OPTION_KEYS.contains(&key) {
            Config::set_option(key.to_owned(), value.to_owned());
        }
    }
    #[cfg(not(target_os = "windows"))]
    Config::set_options(options);
    Ok(())
}

#[cfg(any(target_os = "windows", test))]
async fn sync_security_config<T>(connection: &mut ConnectionTmpl<T>) -> ResultType<()>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    use hbb_common::anyhow::Context;

    async {
        connection.send(&Data::SyncConfig(None)).await?;
        match connection.next_timeout(1_000).await? {
            Some(Data::SyncConfig(Some(configs))) => {
                let (config, options) = *configs;
                crate::trusted_devices::config::update(move || {
                    crate::trusted_devices::config::sync_cli_security_config(config, options)
                })
                .await
            }
            Some(Data::SecurityConfigError(error)) => bail!(error),
            _ => bail!("The host did not provide its current security configuration"),
        }
    }
    .await
    .context("Host option update was acknowledged, but local security synchronization failed")
}

async fn write_option<T>(
    connection: &mut ConnectionTmpl<T>,
    key: &str,
    value: &str,
) -> ResultType<(HashMap<String, String>, bool)>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    connection.send(&Data::Options(None)).await?;
    let mut options = match connection.next_timeout(1_000).await? {
        Some(Data::Options(Some(options))) => options,
        _ => bail!("The host did not return its settings; no update was sent"),
    };
    let previous = options.get(key).cloned();
    if value.is_empty() {
        options.remove(key);
    } else {
        options.insert(key.to_owned(), value.to_owned());
    }
    let security_changed = crate::trusted_devices::config::SECURITY_OPTION_KEYS.contains(&key)
        && previous.as_ref() != options.get(key);
    connection
        .send(&Data::Options(Some(options.clone())))
        .await?;
    match connection.next_timeout(1_000).await? {
        Some(Data::Options(None)) => Ok((options, security_changed)),
        Some(Data::SecurityConfigError(error)) => bail!(error),
        _ => bail!("The host did not confirm the configuration update"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hbb_common::tokio;

    #[tokio::test]
    async fn failed_read_sends_no_settings_update() {
        let (client, service) = tokio::io::duplex(4_096);
        let mut client = ConnectionTmpl::new(client);
        let mut service = ConnectionTmpl::new(service);
        let (result, ()) = tokio::join!(write_option(&mut client, "test", "new"), async {
            assert!(matches!(
                service.next().await.unwrap(),
                Some(Data::Options(None))
            ));
            assert!(service.next_timeout(1_100).await.is_err());
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn changing_or_removing_an_option_preserves_service_settings() {
        for (key, value, security_changed) in [
            ("test", "new", false),
            ("test", "", false),
            ("2fa", "new", true),
            ("2fa", "", true),
            ("2fa", "old", false),
        ] {
            let (client, service) = tokio::io::duplex(4_096);
            let mut client = ConnectionTmpl::new(client);
            let mut service = ConnectionTmpl::new(service);
            let initial = HashMap::from([
                ("relay-server".to_owned(), "relay.example".to_owned()),
                ("test".to_owned(), "old".to_owned()),
                ("2fa".to_owned(), "old".to_owned()),
            ]);
            let (result, ()) = tokio::join!(write_option(&mut client, key, value), async {
                assert!(matches!(
                    service.next().await.unwrap(),
                    Some(Data::Options(None))
                ));
                service.send(&Data::Options(Some(initial))).await.unwrap();
                let Some(Data::Options(Some(updated))) = service.next().await.unwrap() else {
                    panic!("Missing settings update");
                };
                assert_eq!(
                    updated.get("relay-server").map(String::as_str),
                    Some("relay.example")
                );
                assert_eq!(
                    updated.get(key).map(String::as_str),
                    if value.is_empty() { None } else { Some(value) }
                );
                service.send(&Data::Options(None)).await.unwrap();
            });
            assert_eq!(result.unwrap().1, security_changed);
        }
    }

    #[tokio::test]
    async fn rejected_write_is_reported() {
        let (client, service) = tokio::io::duplex(4_096);
        let mut client = ConnectionTmpl::new(client);
        let mut service = ConnectionTmpl::new(service);
        let (result, ()) = tokio::join!(write_option(&mut client, "test", "new"), async {
            service.next().await.unwrap();
            service
                .send(&Data::Options(Some(HashMap::new())))
                .await
                .unwrap();
            service.next().await.unwrap();
            service
                .send(&Data::SecurityConfigError("write denied".to_owned()))
                .await
                .unwrap();
        });
        assert_eq!(result.unwrap_err().to_string(), "write denied");
    }

    #[tokio::test]
    async fn failed_security_sync_reports_that_the_service_already_acknowledged_the_update() {
        for response in [
            Data::SyncConfig(None),
            Data::SecurityConfigError("denied".into()),
        ] {
            let (client, service) = tokio::io::duplex(4_096);
            let mut client = ConnectionTmpl::new(client);
            let mut service = ConnectionTmpl::new(service);
            let (result, ()) = tokio::join!(sync_security_config(&mut client), async {
                assert!(matches!(
                    service.next().await.unwrap(),
                    Some(Data::SyncConfig(None))
                ));
                service.send(&response).await.unwrap();
            });
            assert!(result.unwrap_err().to_string().contains("was acknowledged"));
        }
    }
}
