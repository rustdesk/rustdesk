use reqwest::blocking::Client;
use hbb_common::config::Config;
use hbb_common::log::info;
use hbb_common::proxy::{Proxy, ProxyScheme};
use hbb_common::ResultType;

pub fn create_client() -> ResultType<Client> {
    let mut builder = Client::builder();
    if let Some(conf) = Config::get_socks() {
        info!("Create an http request client with proxy forwarding");
        let proxy = Proxy::form_conf(&conf, None)?;
        // 根据不同的代理类型设置代理
        match &proxy.intercept {
            ProxyScheme::Http {  host, .. } => {
                let proxy = reqwest::Proxy::http(host)?;
                builder = builder.proxy(proxy);
            }
            ProxyScheme::Https {  host, .. } => {
                let proxy = reqwest::Proxy::https(host)?;
                builder = builder.proxy(proxy);
            }
            ProxyScheme::Socks5 { addr, .. } => {
                // 使用socks5代理
                let proxy = reqwest::Proxy::all(&format!("socks5://{}", addr))?;
                builder = builder.proxy(proxy);
            }
        }

        // 如果有认证信息，添加Basic认证头
        if let Some(auth) = proxy.intercept.maybe_auth() {
            let basic_auth = format!(
                "Basic {}",
                auth.get_basic_authorization()
            );
            builder = builder.default_headers(vec![(
                reqwest::header::PROXY_AUTHORIZATION,
                basic_auth.parse().unwrap(),
            )].into_iter().collect());
        }
    }

    Ok(builder.build()?)
}