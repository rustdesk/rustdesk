use hbb_common::{
    async_recursion::async_recursion,
    bail,
    config::{Config, Socks5Server},
    log::{self, info},
    proxy::{fallback, Proxy, ProxyScheme},
    tls::{
        get_cached_tls_accept_invalid_cert, get_cached_tls_type, is_plain, upsert_tls_cache,
        TlsType,
    },
    ResultType,
};
use reqwest::{blocking::Client as SyncClient, Client as AsyncClient};

macro_rules! configure_http_client {
    ($builder:expr, $tls_type:expr, $danger_accept_invalid_cert:expr, $Client: ty) => {{
        configure_http_client!(
            $builder,
            $tls_type,
            $danger_accept_invalid_cert,
            $Client,
            Config::get_socks()
        )
    }};
    ($builder:expr, $tls_type:expr, $danger_accept_invalid_cert:expr, $Client: ty, $proxy_conf:expr) => {{
        // https://github.com/rustdesk/rustdesk/issues/11569
        // https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.no_proxy
        let mut builder = $builder.no_proxy();

        match $tls_type {
            TlsType::Plain => {}
            TlsType::NativeTls => {
                builder = builder.use_native_tls();
                if $danger_accept_invalid_cert {
                    builder = builder.danger_accept_invalid_certs(true);
                }
            }
            TlsType::Rustls => {
                #[cfg(any(target_os = "android", target_os = "ios"))]
                match hbb_common::verifier::client_config($danger_accept_invalid_cert) {
                    Ok(client_config) => {
                        builder = builder.use_preconfigured_tls(client_config);
                    }
                    Err(e) => {
                        hbb_common::log::error!("Failed to get client config: {}", e);
                    }
                }
                #[cfg(not(any(target_os = "android", target_os = "ios")))]
                {
                    builder = builder.use_rustls_tls();
                    if $danger_accept_invalid_cert {
                        builder = builder.danger_accept_invalid_certs(true);
                    }
                }
            }
        }

        let client = if let Some(conf) = $proxy_conf {
            let proxy_result = Proxy::from_conf(&conf, None);

            match proxy_result {
                Ok(proxy) => {
                    let proxy_setup = match &proxy.intercept {
                        ProxyScheme::Http { host, .. } => {
                            reqwest::Proxy::all(format!("http://{}", host))
                        }
                        ProxyScheme::Https { host, .. } => {
                            reqwest::Proxy::all(format!("https://{}", host))
                        }
                        ProxyScheme::Socks5 { addr, .. } => {
                            reqwest::Proxy::all(&format!("socks5://{}", addr))
                        }
                    };

                    match proxy_setup {
                        Ok(mut p) => {
                            if let Some(auth) = proxy.intercept.maybe_auth() {
                                if !auth.username().is_empty() && !auth.password().is_empty() {
                                    p = p.basic_auth(auth.username(), auth.password());
                                }
                            }
                            builder = builder.proxy(p);
                            builder.build().unwrap_or_else(|e| {
                                info!("Failed to create a proxied client: {}", e);
                                <$Client>::new()
                            })
                        }
                        Err(e) => {
                            info!("Failed to set up proxy: {}", e);
                            <$Client>::new()
                        }
                    }
                }
                Err(e) => {
                    info!("Failed to configure proxy: {}", e);
                    <$Client>::new()
                }
            }
        } else {
            builder.build().unwrap_or_else(|e| {
                info!("Failed to create a client: {}", e);
                <$Client>::new()
            })
        };

        client
    }};
}

pub fn create_http_client(tls_type: TlsType, danger_accept_invalid_cert: bool) -> SyncClient {
    if fallback::is_fallback_enabled() {
        return configure_http_client!(
            SyncClient::builder(),
            tls_type,
            danger_accept_invalid_cert,
            SyncClient,
            fallback::get_socks_sync()
        );
    }
    let builder = SyncClient::builder();
    configure_http_client!(builder, tls_type, danger_accept_invalid_cert, SyncClient)
}

pub fn create_http_client_async(
    tls_type: TlsType,
    danger_accept_invalid_cert: bool,
) -> AsyncClient {
    let builder = AsyncClient::builder();
    configure_http_client!(builder, tls_type, danger_accept_invalid_cert, AsyncClient)
}

pub fn get_url_for_tls<'a>(url: &'a str, proxy_conf: &'a Option<Socks5Server>) -> &'a str {
    if is_plain(url) {
        if let Some(conf) = proxy_conf {
            if conf.proxy.starts_with("https://") {
                return &conf.proxy;
            }
        }
    }
    url
}

pub async fn create_http_client_async_with_proxy_fallback(
    tls_type: TlsType,
    danger_accept_invalid_cert: bool,
) -> AsyncClient {
    if !fallback::is_fallback_enabled() {
        return create_http_client_async(tls_type, danger_accept_invalid_cert);
    }
    let proxy_conf = fallback::get_socks().await;
    configure_http_client!(
        AsyncClient::builder(),
        tls_type,
        danger_accept_invalid_cert,
        AsyncClient,
        proxy_conf
    )
}

pub fn create_http_client_with_url(url: &str) -> SyncClient {
    let proxy_conf = fallback::get_socks_sync();
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let tls_type = get_cached_tls_type(tls_url);
    let is_tls_type_cached = tls_type.is_some();
    let tls_type = tls_type.unwrap_or(TlsType::Rustls);
    let tls_danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    create_http_client_with_url_(
        url,
        tls_url,
        tls_type,
        is_tls_type_cached,
        tls_danger_accept_invalid_cert,
        tls_danger_accept_invalid_cert,
    )
}

pub fn create_http_client_with_url_strict(url: &str) -> ResultType<SyncClient> {
    let parsed_url = url::Url::parse(url)?;
    if parsed_url.scheme() != "https" {
        bail!("Strict HTTP client requires HTTPS: {}", url);
    }
    let proxy_conf = fallback::get_socks_sync();
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let cached_tls_type = get_cached_tls_type(tls_url);
    let cached_danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    let can_reuse_cached_probe =
        cached_tls_type.is_some() && cached_danger_accept_invalid_cert == Some(false);
    let tls_type = if can_reuse_cached_probe {
        cached_tls_type.unwrap_or(TlsType::Rustls)
    } else {
        TlsType::Rustls
    };
    Ok(create_http_client_with_url_(
        url,
        tls_url,
        tls_type,
        can_reuse_cached_probe,
        Some(false),
        Some(false),
    ))
}

fn create_http_client_with_url_(
    url: &str,
    tls_url: &str,
    tls_type: TlsType,
    is_tls_type_cached: bool,
    danger_accept_invalid_cert: Option<bool>,
    original_danger_accept_invalid_cert: Option<bool>,
) -> SyncClient {
    let mut client = create_http_client(tls_type, danger_accept_invalid_cert.unwrap_or(false));
    if is_tls_type_cached && original_danger_accept_invalid_cert.is_some() {
        return client;
    }
    if let Err(e) = client.head(url).send() {
        if e.is_request() {
            match (tls_type, is_tls_type_cached, danger_accept_invalid_cert) {
                (TlsType::Rustls, _, None) => {
                    log::warn!(
                        "Failed to connect to server {} with rustls-tls: {:?}, trying accept invalid cert",
                        tls_url,
                        e
                    );
                    client = create_http_client_with_url_(
                        url,
                        tls_url,
                        tls_type,
                        is_tls_type_cached,
                        Some(true),
                        original_danger_accept_invalid_cert,
                    );
                }
                (TlsType::Rustls, false, Some(_)) => {
                    log::warn!(
                        "Failed to connect to server {} with rustls-tls: {:?}, trying native-tls",
                        tls_url,
                        e
                    );
                    client = create_http_client_with_url_(
                        url,
                        tls_url,
                        TlsType::NativeTls,
                        is_tls_type_cached,
                        original_danger_accept_invalid_cert,
                        original_danger_accept_invalid_cert,
                    );
                }
                (TlsType::NativeTls, _, None) => {
                    log::warn!(
                        "Failed to connect to server {} with native-tls: {:?}, trying accept invalid cert",
                        tls_url,
                        e
                    );
                    client = create_http_client_with_url_(
                        url,
                        tls_url,
                        tls_type,
                        is_tls_type_cached,
                        Some(true),
                        original_danger_accept_invalid_cert,
                    );
                }
                _ => {
                    log::error!(
                        "Failed to connect to server {} with {:?}, err: {:?}.",
                        tls_url,
                        tls_type,
                        e
                    );
                }
            }
        } else {
            log::warn!(
                "Failed to connect to server {} with {:?}, err: {}.",
                tls_url,
                tls_type,
                e
            );
        }
    } else {
        log::info!(
            "Successfully connected to server {} with {:?}",
            tls_url,
            tls_type
        );
        upsert_tls_cache(
            tls_url,
            tls_type,
            danger_accept_invalid_cert.unwrap_or(false),
        );
    }
    client
}

pub async fn create_http_client_async_with_url(url: &str) -> AsyncClient {
    let proxy_conf = fallback::get_socks().await;
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let tls_type = get_cached_tls_type(tls_url);
    let is_tls_type_cached = tls_type.is_some();
    let tls_type = tls_type.unwrap_or(TlsType::Rustls);
    let danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    create_http_client_async_with_url_(
        url,
        tls_url,
        tls_type,
        is_tls_type_cached,
        danger_accept_invalid_cert,
        danger_accept_invalid_cert,
    )
    .await
}

pub async fn create_http_client_async_with_url_strict(url: &str) -> ResultType<AsyncClient> {
    let parsed_url = url::Url::parse(url)?;
    if parsed_url.scheme() != "https" {
        bail!("Strict HTTP client requires HTTPS: {}", url);
    }
    let proxy_conf = fallback::get_socks().await;
    let tls_url = get_url_for_tls(url, &proxy_conf);
    let cached_tls_type = get_cached_tls_type(tls_url);
    let cached_danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(tls_url);
    let can_reuse_cached_probe =
        cached_tls_type.is_some() && cached_danger_accept_invalid_cert == Some(false);
    let tls_type = if can_reuse_cached_probe {
        cached_tls_type.unwrap_or(TlsType::Rustls)
    } else {
        TlsType::Rustls
    };
    Ok(create_http_client_async_with_url_(
        url,
        tls_url,
        tls_type,
        can_reuse_cached_probe,
        Some(false),
        Some(false),
    )
    .await)
}

#[async_recursion]
async fn create_http_client_async_with_url_(
    url: &str,
    tls_url: &str,
    tls_type: TlsType,
    is_tls_type_cached: bool,
    danger_accept_invalid_cert: Option<bool>,
    original_danger_accept_invalid_cert: Option<bool>,
) -> AsyncClient {
    let mut client = create_http_client_async_with_proxy_fallback(
        tls_type,
        danger_accept_invalid_cert.unwrap_or(false),
    )
    .await;
    if is_tls_type_cached && original_danger_accept_invalid_cert.is_some() {
        return client;
    }
    if let Err(e) = client.head(url).send().await {
        match (tls_type, is_tls_type_cached, danger_accept_invalid_cert) {
            (TlsType::Rustls, _, None) => {
                log::warn!(
                    "Failed to connect to server {} with rustls-tls: {:?}, trying accept invalid cert",
                    tls_url,
                    e
                );
                client = create_http_client_async_with_url_(
                    url,
                    tls_url,
                    tls_type,
                    is_tls_type_cached,
                    Some(true),
                    original_danger_accept_invalid_cert,
                )
                .await;
            }
            (TlsType::Rustls, false, Some(_)) => {
                log::warn!(
                    "Failed to connect to server {} with rustls-tls: {:?}, trying native-tls",
                    tls_url,
                    e
                );
                client = create_http_client_async_with_url_(
                    url,
                    tls_url,
                    TlsType::NativeTls,
                    is_tls_type_cached,
                    original_danger_accept_invalid_cert,
                    original_danger_accept_invalid_cert,
                )
                .await;
            }
            (TlsType::NativeTls, _, None) => {
                log::warn!(
                    "Failed to connect to server {} with native-tls: {:?}, trying accept invalid cert",
                    tls_url,
                    e
                );
                client = create_http_client_async_with_url_(
                    url,
                    tls_url,
                    tls_type,
                    is_tls_type_cached,
                    Some(true),
                    original_danger_accept_invalid_cert,
                )
                .await;
            }
            _ => {
                log::error!(
                    "Failed to connect to server {} with {:?}, err: {:?}.",
                    tls_url,
                    tls_type,
                    e
                );
            }
        }
    } else {
        log::info!(
            "Successfully connected to server {} with {:?}",
            tls_url,
            tls_type
        );
        upsert_tls_cache(
            tls_url,
            tls_type,
            danger_accept_invalid_cert.unwrap_or(false),
        );
    }
    client
}

#[cfg(test)]
mod proxy_fallback_tests {
    use super::*;
    use base::config::keys::{OPTION_ALLOW_PROXY_FALLBACK, OPTION_PROXY_URL};
    use hbb_common::{
        config::OVERWRITE_SETTINGS,
        tokio::{
            self,
            io::{AsyncReadExt, AsyncWriteExt},
            net::{TcpListener, TcpSocket},
        },
    };
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::Duration,
    };

    struct RestoreSettings(Vec<(&'static str, Option<String>)>);

    impl Drop for RestoreSettings {
        fn drop(&mut self) {
            let mut settings = OVERWRITE_SETTINGS.write().unwrap();
            for (key, value) in self.0.drain(..) {
                if let Some(value) = value {
                    settings.insert(key.to_owned(), value);
                } else {
                    settings.remove(key);
                }
            }
        }
    }

    fn set_option(key: &str, value: &str) {
        OVERWRITE_SETTINGS
            .write()
            .unwrap()
            .insert(key.to_owned(), value.to_owned());
    }

    async fn serve(listener: TcpListener, response: &'static [u8], requests: Arc<AtomicUsize>) {
        loop {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            if stream.read(&mut request).await.unwrap() != 0 {
                requests.fetch_add(1, Ordering::SeqCst);
                stream.write_all(response).await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn http_proxy_fallback_sync_and_async() {
        let _restore = RestoreSettings(
            [OPTION_PROXY_URL, OPTION_ALLOW_PROXY_FALLBACK]
                .into_iter()
                .map(|key| (key, OVERWRITE_SETTINGS.read().unwrap().get(key).cloned()))
                .collect(),
        );
        let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/test", target.local_addr().unwrap());
        let requests = Arc::new(AtomicUsize::new(0));
        let target_server = tokio::spawn(serve(
            target,
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
            requests.clone(),
        ));
        let unavailable = TcpSocket::new_v4().unwrap();
        unavailable.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        set_option(
            OPTION_PROXY_URL,
            &format!("http://{}", unavailable.local_addr().unwrap()),
        );
        set_option(OPTION_ALLOW_PROXY_FALLBACK, "");
        let saved = Config::get_socks();
        assert!(
            create_http_client_async_with_proxy_fallback(TlsType::Plain, false)
                .await
                .get(&url)
                .timeout(Duration::from_secs(1))
                .send()
                .await
                .is_err()
        );
        assert_eq!(requests.load(Ordering::SeqCst), 0);

        set_option(OPTION_ALLOW_PROXY_FALLBACK, "Y");
        let response = create_http_client_async_with_proxy_fallback(TlsType::Plain, false)
            .await
            .get(&url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "ok");
        let sync_url = url.clone();
        tokio::task::spawn_blocking(move || {
            let response = create_http_client(TlsType::Plain, false)
                .get(sync_url)
                .timeout(Duration::from_secs(1))
                .send()
                .unwrap();
            assert_eq!(response.text().unwrap(), "ok");
        })
        .await
        .unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        assert_eq!(Config::get_socks(), saved);

        let reachable = TcpListener::bind("127.0.0.1:0").await.unwrap();
        set_option(
            OPTION_PROXY_URL,
            &format!("http://{}", reachable.local_addr().unwrap()),
        );
        let proxy_server = tokio::spawn(serve(
            reachable,
            b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            Arc::new(AtomicUsize::new(0)),
        ));
        let response = create_http_client_async_with_proxy_fallback(TlsType::Plain, false)
            .await
            .get(&url)
            .timeout(Duration::from_secs(1))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 407);
        tokio::task::spawn_blocking(move || {
            let response = create_http_client(TlsType::Plain, false)
                .get(url)
                .timeout(Duration::from_secs(1))
                .send()
                .unwrap();
            assert_eq!(response.status().as_u16(), 407);
        })
        .await
        .unwrap();
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        target_server.abort();
        proxy_server.abort();
    }
}
