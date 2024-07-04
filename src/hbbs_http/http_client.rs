use hbb_common::config::Config;
use hbb_common::log::info;
use hbb_common::proxy::{Proxy, ProxyScheme};
use reqwest::blocking::Client as SyncClient;
use reqwest::Client as AsyncClient;

macro_rules! configure_http_client {
    ($builder:expr, $Client: ty) => {{
        let mut builder = $builder;
        let client = if let Some(conf) = Config::get_socks() {
            let proxy_result = Proxy::from_conf(&conf, None);

            match proxy_result {
                Ok(proxy) => {
                    let proxy_setup = match &proxy.intercept {
                        ProxyScheme::Http { host, .. } =>{ reqwest::Proxy::http(format!("http://{}", host))},
                        ProxyScheme::Https { host, .. } => {reqwest::Proxy::https(format!("https://{}", host))},
                        ProxyScheme::Socks5 { addr, .. } => { reqwest::Proxy::all(&format!("socks5://{}", addr)) }
                    };

                    match proxy_setup {
                        Ok(p) => {
                            builder = builder.proxy(p);
                            if let Some(auth) = proxy.intercept.maybe_auth() {
                                let basic_auth =
                                    format!("Basic {}", auth.get_basic_authorization());
                                if let Ok(auth) = basic_auth.parse() {
                                    builder = builder.default_headers(
                                        vec![(
                                            reqwest::header::PROXY_AUTHORIZATION,
                                            auth,
                                        )]
                                        .into_iter()
                                        .collect(),
                                    );
                                }
                            }
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

pub fn create_http_client() -> SyncClient {
    let builder = SyncClient::builder();
    configure_http_client!(builder, SyncClient)
}

pub fn create_http_client_async() -> AsyncClient {
    let builder = AsyncClient::builder();
    configure_http_client!(builder, AsyncClient)
}
