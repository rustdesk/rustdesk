use super::{connect, ConnectionTmpl, Data};
use hbb_common::{
    bail,
    config::Config,
    tokio::{
        self,
        io::{AsyncRead, AsyncWrite},
    },
    ResultType,
};
use std::collections::HashMap;

#[tokio::main(flavor = "current_thread")]
pub async fn set_option_from_cli(key: &str, value: &str) -> ResultType<()> {
    let _nat = crate::common::CheckTestNatType::new();
    let mut connection = connect(1_000, "").await?;
    let options = write_option(&mut connection, key, value).await?;
    #[cfg(target_os = "windows")]
    if crate::trusted_devices::config::changes_authentication(&options) {
        super::sync_security_config_from_server(&mut connection).await?;
    }
    Config::set_options(options);
    Ok(())
}

async fn write_option<T>(
    connection: &mut ConnectionTmpl<T>,
    key: &str,
    value: &str,
) -> ResultType<HashMap<String, String>>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    connection.send(&Data::Options(None)).await?;
    let mut options = match connection.next_timeout(1_000).await? {
        Some(Data::Options(Some(options))) => options,
        _ => bail!("The host did not return its settings; no update was sent"),
    };
    if value.is_empty() {
        options.remove(key);
    } else {
        options.insert(key.to_owned(), value.to_owned());
    }
    connection
        .send(&Data::Options(Some(options.clone())))
        .await?;
    match connection.next_timeout(1_000).await? {
        Some(Data::Options(None)) => Ok(options),
        Some(Data::SecurityConfigError(error)) => bail!(error),
        _ => bail!("The host did not confirm the configuration update"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for value in ["new", ""] {
            let (client, service) = tokio::io::duplex(4_096);
            let mut client = ConnectionTmpl::new(client);
            let mut service = ConnectionTmpl::new(service);
            let initial = HashMap::from([
                ("relay-server".to_owned(), "relay.example".to_owned()),
                ("test".to_owned(), "old".to_owned()),
            ]);
            let (result, ()) = tokio::join!(write_option(&mut client, "test", value), async {
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
                    updated.get("test").map(String::as_str),
                    if value.is_empty() { None } else { Some(value) }
                );
                service.send(&Data::Options(None)).await.unwrap();
            });
            assert!(result.is_ok());
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
}
