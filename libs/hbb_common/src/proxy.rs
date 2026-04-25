use std::{
    io::Error as IoError,
    net::{SocketAddr, ToSocketAddrs},
};

use anyhow::bail;
use async_recursion::async_recursion;
use base64::{engine::general_purpose, Engine};
use httparse::{Error as HttpParseError, Response, EMPTY_HEADER};
use thiserror::Error as ThisError;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufStream};
use tokio_native_tls::{native_tls, TlsConnector, TlsStream};
use tokio_rustls::{client::TlsStream as RustlsTlsStream, TlsConnector as RustlsTlsConnector};
use tokio_socks::{tcp::Socks5Stream, IntoTargetAddr, TargetAddr};
use tokio_util::codec::Framed;
use url::Url;

use crate::{
    bytes_codec::BytesCodec,
    config::Socks5Server,
    tcp::{DynTcpStream, FramedStream},
    tls::{get_cached_tls_accept_invalid_cert, get_cached_tls_type, upsert_tls_cache, TlsType},
    ResultType,
};

#[derive(Debug, ThisError)]
pub enum ProxyError {
    #[error("IO Error: {0}")]
    IoError(#[from] IoError),
    #[error("Target parse error: {0}")]
    TargetParseError(String),
    #[error("HTTP parse error: {0}")]
    HttpParseError(#[from] HttpParseError),
    #[error("The maximum response header length is exceeded: {0}")]
    MaximumResponseHeaderLengthExceeded(usize),
    #[error("The end of file is reached")]
    EndOfFile,
    #[error("The url is error: {0}")]
    UrlBadScheme(String),
    #[error("The url parse error: {0}")]
    UrlParseScheme(#[from] url::ParseError),
    #[error("No HTTP code was found in the response")]
    NoHttpCode,
    #[error("The HTTP code is not equal 200: {0}")]
    HttpCode200(u16),
    #[error("The proxy address resolution failed: {0}")]
    AddressResolutionFailed(String),
    #[error("The native tls error: {0}")]
    NativeTlsError(#[from] tokio_native_tls::native_tls::Error),
}

const MAXIMUM_RESPONSE_HEADER_LENGTH: usize = 4096;
/// The maximum HTTP Headers, which can be parsed.
const MAXIMUM_RESPONSE_HEADERS: usize = 16;
const DEFINE_TIME_OUT: u64 = 600;

pub trait IntoUrl {
    // Besides parsing as a valid `Url`, the `Url` must be a valid
    // `http::Uri`, in that it makes sense to use in a network request.
    fn into_url(self) -> Result<Url, ProxyError>;

    fn as_str(&self) -> &str;
}

impl IntoUrl for Url {
    fn into_url(self) -> Result<Url, ProxyError> {
        if self.has_host() {
            Ok(self)
        } else {
            Err(ProxyError::UrlBadScheme(self.to_string()))
        }
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl<'a> IntoUrl for &'a str {
    fn into_url(self) -> Result<Url, ProxyError> {
        Url::parse(self)
            .map_err(ProxyError::UrlParseScheme)?
            .into_url()
    }

    fn as_str(&self) -> &str {
        self
    }
}

impl<'a> IntoUrl for &'a String {
    fn into_url(self) -> Result<Url, ProxyError> {
        (&**self).into_url()
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

impl<'a> IntoUrl for String {
    fn into_url(self) -> Result<Url, ProxyError> {
        (&*self).into_url()
    }

    fn as_str(&self) -> &str {
        self.as_ref()
    }
}

#[derive(Clone)]
pub struct Auth {
    user_name: String,
    password: String,
}

impl Auth {
    fn get_proxy_authorization(&self) -> String {
        format!(
            "Proxy-Authorization: Basic {}\r\n",
            self.get_basic_authorization()
        )
    }

    pub fn get_basic_authorization(&self) -> String {
        let authorization = format!("{}:{}", &self.user_name, &self.password);
        general_purpose::STANDARD.encode(authorization.as_bytes())
    }

    pub fn username(&self) -> &str {
        &self.user_name
    }

    pub fn password(&self) -> &str {
        &self.password
    }
}

#[derive(Clone)]
pub enum ProxyScheme {
    Http {
        auth: Option<Auth>,
        host: String,
    },
    Https {
        auth: Option<Auth>,
        host: String,
    },
    Socks5 {
        addr: SocketAddr,
        auth: Option<Auth>,
        remote_dns: bool,
    },
}

impl ProxyScheme {
    pub fn maybe_auth(&self) -> Option<&Auth> {
        match self {
            ProxyScheme::Http { auth, .. }
            | ProxyScheme::Https { auth, .. }
            | ProxyScheme::Socks5 { auth, .. } => auth.as_ref(),
        }
    }

    fn socks5(addr: SocketAddr) -> Result<Self, ProxyError> {
        Ok(ProxyScheme::Socks5 {
            addr,
            auth: None,
            remote_dns: false,
        })
    }

    fn http(host: &str) -> Result<Self, ProxyError> {
        Ok(ProxyScheme::Http {
            auth: None,
            host: host.to_string(),
        })
    }
    fn https(host: &str) -> Result<Self, ProxyError> {
        Ok(ProxyScheme::Https {
            auth: None,
            host: host.to_string(),
        })
    }

    fn set_basic_auth<T: Into<String>, U: Into<String>>(&mut self, username: T, password: U) {
        let auth = Auth {
            user_name: username.into(),
            password: password.into(),
        };
        match self {
            ProxyScheme::Http { auth: a, .. } => *a = Some(auth),
            ProxyScheme::Https { auth: a, .. } => *a = Some(auth),
            ProxyScheme::Socks5 { auth: a, .. } => *a = Some(auth),
        }
    }

    fn parse(url: Url) -> Result<Self, ProxyError> {
        use url::Position;

        // Resolve URL to a host and port
        let to_addr = || {
            let addrs = url.socket_addrs(|| match url.scheme() {
                "socks5" => Some(1080),
                _ => None,
            })?;
            addrs
                .into_iter()
                .next()
                .ok_or_else(|| ProxyError::UrlParseScheme(url::ParseError::EmptyHost))
        };

        let mut scheme: Self = match url.scheme() {
            "http" => Self::http(&url[Position::BeforeHost..Position::AfterPort])?,
            "https" => Self::https(&url[Position::BeforeHost..Position::AfterPort])?,
            "socks5" => Self::socks5(to_addr()?)?,
            e => return Err(ProxyError::UrlBadScheme(e.to_string())),
        };

        if let Some(pwd) = url.password() {
            let username = url.username();
            scheme.set_basic_auth(username, pwd);
        }

        Ok(scheme)
    }
    pub async fn socket_addrs(&self) -> Result<SocketAddr, ProxyError> {
        log::trace!("Resolving socket address");
        match self {
            ProxyScheme::Http { host, .. } => self.resolve_host(host, 80).await,
            ProxyScheme::Https { host, .. } => self.resolve_host(host, 443).await,
            ProxyScheme::Socks5 { addr, .. } => Ok(addr.clone()),
        }
    }

    async fn resolve_host(&self, host: &str, default_port: u16) -> Result<SocketAddr, ProxyError> {
        let (host_str, port) = match host.split_once(':') {
            Some((h, p)) => (h, p.parse::<u16>().ok()),
            None => (host, None),
        };
        let addr = (host_str, port.unwrap_or(default_port))
            .to_socket_addrs()?
            .next()
            .ok_or_else(|| ProxyError::AddressResolutionFailed(host.to_string()))?;
        Ok(addr)
    }

    pub fn get_domain(&self) -> Result<String, ProxyError> {
        match self {
            ProxyScheme::Http { host, .. } | ProxyScheme::Https { host, .. } => {
                let domain = host
                    .split(':')
                    .next()
                    .ok_or_else(|| ProxyError::AddressResolutionFailed(host.clone()))?;
                Ok(domain.to_string())
            }
            ProxyScheme::Socks5 { addr, .. } => match addr {
                SocketAddr::V4(addr_v4) => Ok(addr_v4.ip().to_string()),
                SocketAddr::V6(addr_v6) => Ok(addr_v6.ip().to_string()),
            },
        }
    }
    pub fn get_host_and_port(&self) -> Result<String, ProxyError> {
        match self {
            ProxyScheme::Http { host, .. } => Ok(self.append_default_port(host, 80)),
            ProxyScheme::Https { host, .. } => Ok(self.append_default_port(host, 443)),
            ProxyScheme::Socks5 { addr, .. } => Ok(format!("{}", addr)),
        }
    }
    fn append_default_port(&self, host: &str, default_port: u16) -> String {
        if host.contains(':') {
            host.to_string()
        } else {
            format!("{}:{}", host, default_port)
        }
    }
}

pub trait IntoProxyScheme {
    fn into_proxy_scheme(self) -> Result<ProxyScheme, ProxyError>;
}

impl<S: IntoUrl> IntoProxyScheme for S {
    fn into_proxy_scheme(self) -> Result<ProxyScheme, ProxyError> {
        // validate the URL
        let url = match self.as_str().into_url() {
            Ok(ok) => ok,
            Err(e) => {
                match e {
                    // If the string does not contain protocol headers, try to parse it using the socks5 protocol
                    ProxyError::UrlParseScheme(_source) => {
                        let try_this = format!("socks5://{}", self.as_str());
                        try_this.into_url()?
                    }
                    _ => {
                        return Err(e);
                    }
                }
            }
        };
        ProxyScheme::parse(url)
    }
}

impl IntoProxyScheme for ProxyScheme {
    fn into_proxy_scheme(self) -> Result<ProxyScheme, ProxyError> {
        Ok(self)
    }
}

#[derive(Clone)]
pub struct Proxy {
    pub intercept: ProxyScheme,
    ms_timeout: u64,
}

impl Proxy {
    pub fn new<U: IntoProxyScheme>(proxy_scheme: U, ms_timeout: u64) -> Result<Self, ProxyError> {
        Ok(Self {
            intercept: proxy_scheme.into_proxy_scheme()?,
            ms_timeout,
        })
    }

    pub fn is_http_or_https(&self) -> bool {
        return match self.intercept {
            ProxyScheme::Socks5 { .. } => false,
            _ => true,
        };
    }

    pub fn from_conf(conf: &Socks5Server, ms_timeout: Option<u64>) -> Result<Self, ProxyError> {
        let mut proxy;
        match ms_timeout {
            None => {
                proxy = Self::new(&conf.proxy, DEFINE_TIME_OUT)?;
            }
            Some(time_out) => {
                proxy = Self::new(&conf.proxy, time_out)?;
            }
        }

        if !conf.password.is_empty() && !conf.username.is_empty() {
            proxy = proxy.basic_auth(&conf.username, &conf.password);
        }
        Ok(proxy)
    }

    pub async fn proxy_addrs(&self) -> Result<SocketAddr, ProxyError> {
        self.intercept.socket_addrs().await
    }

    fn basic_auth(mut self, username: &str, password: &str) -> Proxy {
        self.intercept.set_basic_auth(username, password);
        self
    }

    async fn new_stream(
        &self,
        local: SocketAddr,
        proxy: SocketAddr,
    ) -> ResultType<tokio::net::TcpStream> {
        let stream = super::timeout(
            self.ms_timeout,
            crate::tcp::new_socket(local, true)?.connect(proxy),
        )
        .await??;
        stream.set_nodelay(true).ok();
        Ok(stream)
    }

    pub async fn connect<'t, T>(
        &self,
        target: T,
        local_addr: Option<SocketAddr>,
    ) -> ResultType<FramedStream>
    where
        T: IntoTargetAddr<'t>,
    {
        log::trace!("Connect to proxy server");
        let proxy = self.proxy_addrs().await?;

        let target_addr = target
            .into_target_addr()
            .map_err(|e| ProxyError::TargetParseError(e.to_string()))?;

        let local = if let Some(addr) = local_addr {
            addr
        } else {
            crate::config::Config::get_any_listen_addr(proxy.is_ipv4())
        };

        let stream = self.new_stream(local, proxy).await?;
        let addr = stream.local_addr()?;

        return match self.intercept {
            ProxyScheme::Http { .. } => {
                log::trace!("Connect to remote http proxy server: {}", proxy);
                let stream =
                    super::timeout(self.ms_timeout, self.http_connect(stream, &target_addr))
                        .await??;
                Ok(FramedStream(
                    Framed::new(DynTcpStream(Box::new(stream)), BytesCodec::new()),
                    addr,
                    None,
                    0,
                ))
            }
            ProxyScheme::Https { .. } => {
                log::trace!("Connect to remote https proxy server: {}", proxy);
                let url = format!("https://{}", self.intercept.get_host_and_port()?);
                let tls_type = get_cached_tls_type(&url);
                let danger_accept_invalid_cert = get_cached_tls_accept_invalid_cert(&url);
                let stream = match tls_type.unwrap_or(TlsType::Rustls) {
                    TlsType::Rustls => {
                        self.https_connect_rustls_wrap_danger(
                            &url,
                            local,
                            proxy,
                            Some(stream),
                            &target_addr,
                            tls_type.is_some(),
                            danger_accept_invalid_cert,
                            danger_accept_invalid_cert,
                        )
                        .await?
                    }
                    TlsType::NativeTls => {
                        self.https_connect_nativetls_wrap_danger(
                            &url,
                            local,
                            proxy,
                            &target_addr,
                            danger_accept_invalid_cert,
                        )
                        .await?
                    }
                    _ => {
                        // Unreachable
                        crate::bail!("Unreachable, TlsType::Plain in HTTPS proxy");
                    }
                };
                Ok(FramedStream(
                    Framed::new(stream, BytesCodec::new()),
                    addr,
                    None,
                    0,
                ))
            }
            ProxyScheme::Socks5 { .. } => {
                log::trace!("Connect to remote socket5 proxy server: {}", proxy);
                let stream = if let Some(auth) = self.intercept.maybe_auth() {
                    super::timeout(
                        self.ms_timeout,
                        Socks5Stream::connect_with_password_and_socket(
                            stream,
                            target_addr,
                            &auth.user_name,
                            &auth.password,
                        ),
                    )
                    .await??
                } else {
                    super::timeout(
                        self.ms_timeout,
                        Socks5Stream::connect_with_socket(stream, target_addr),
                    )
                    .await??
                };
                Ok(FramedStream(
                    Framed::new(DynTcpStream(Box::new(stream)), BytesCodec::new()),
                    addr,
                    None,
                    0,
                ))
            }
        };
    }

    async fn https_connect_nativetls_wrap_danger<'a>(
        &self,
        url: &str,
        local: SocketAddr,
        proxy: SocketAddr,
        target_addr: &TargetAddr<'a>,
        danger_accept_invalid_cert: Option<bool>,
    ) -> ResultType<DynTcpStream> {
        let stream = self.new_stream(local, proxy).await?;
        let s = super::timeout(
            self.ms_timeout,
            self.https_connect_nativetls(
                stream,
                &target_addr,
                danger_accept_invalid_cert.unwrap_or(false),
            ),
        )
        .await??;
        upsert_tls_cache(
            url,
            TlsType::NativeTls,
            danger_accept_invalid_cert.unwrap_or(false),
        );
        Ok(DynTcpStream(Box::new(s)))
    }

    pub async fn https_connect_nativetls<'a, Input>(
        &self,
        io: Input,
        target_addr: &TargetAddr<'a>,
        danger_accept_invalid_cert: bool,
    ) -> Result<BufStream<TlsStream<Input>>, ProxyError>
    where
        Input: AsyncRead + AsyncWrite + Unpin,
    {
        let mut tls_connector_builder = native_tls::TlsConnector::builder();
        if danger_accept_invalid_cert {
            tls_connector_builder.danger_accept_invalid_certs(true);
        }
        let tls_connector = TlsConnector::from(tls_connector_builder.build()?);
        let stream = tls_connector
            .connect(&self.intercept.get_domain()?, io)
            .await?;
        self.http_connect(stream, target_addr).await
    }

    #[async_recursion]
    async fn https_connect_rustls_wrap_danger<'a>(
        &self,
        url: &str,
        local: SocketAddr,
        proxy: SocketAddr,
        stream: Option<tokio::net::TcpStream>,
        target_addr: &TargetAddr<'a>,
        is_tls_type_cached: bool,
        danger_accept_invalid_cert: Option<bool>,
        origin_danger_accept_invalid_cert: Option<bool>,
    ) -> ResultType<DynTcpStream> {
        let stream = stream.unwrap_or(self.new_stream(local, proxy).await?);
        match super::timeout(
            self.ms_timeout,
            self.https_connect_rustls(
                stream,
                target_addr,
                danger_accept_invalid_cert.unwrap_or(false),
            ),
        )
        .await?
        {
            Ok(s) => {
                upsert_tls_cache(
                    &url,
                    TlsType::Rustls,
                    danger_accept_invalid_cert.unwrap_or(false),
                );
                Ok(DynTcpStream(Box::new(s)))
            }
            Err(e) => {
                // NOTE: Maybe it's better to check if the error is related to TLS here. (ProxyError::IoError(e), or ProxyError::NativeTlsError(e))
                // But we can only get the error when the TLS protocol is TLSv1.1.
                // The error message of the following is unclear:
                // https://github.com/rustdesk/rustdesk-server-pro/issues/189#issuecomment-1895701480
                // So we just try to fallback unconditionally here.
                //
                // If the protocol is TLS 1.1, the error is:
                // 1. "IO Error: received fatal alert: ProtocolVersion"
                // 2. "IO Error: An existing connection was forcibly closed by the remote host. (os error 10054)" on Windows sometimes.
                //
                // If the cert verification fails, the error is:
                // "IO Error: invalid peer certificate: UnknownIssuer"

                let s = if danger_accept_invalid_cert.is_none() {
                    log::warn!(
                        "Falling back to rustls-tls (accept invalid cert) for HTTPS proxy server."
                    );
                    self.https_connect_rustls_wrap_danger(
                        &url,
                        local,
                        proxy,
                        None,
                        target_addr,
                        is_tls_type_cached,
                        Some(true),
                        origin_danger_accept_invalid_cert,
                    )
                    .await?
                } else if !is_tls_type_cached {
                    log::warn!("Falling back to native-tls for HTTPS proxy server.");
                    self.https_connect_nativetls_wrap_danger(
                        &url,
                        local,
                        proxy,
                        &target_addr,
                        origin_danger_accept_invalid_cert,
                    )
                    .await?
                } else {
                    log::error!(
                        "Failed to connect to HTTPS proxy server with native-tls: {:?}.",
                        e
                    );
                    bail!(e)
                };
                Ok(s)
            }
        }
    }

    pub async fn https_connect_rustls<'a, Input>(
        &self,
        io: Input,
        target_addr: &TargetAddr<'a>,
        danger_accept_invalid_cert: bool,
    ) -> Result<BufStream<RustlsTlsStream<Input>>, ProxyError>
    where
        Input: AsyncRead + AsyncWrite + Unpin,
    {
        use std::convert::TryFrom;

        let url_domain = self.intercept.get_domain()?;
        let domain = rustls_pki_types::ServerName::try_from(url_domain.as_str())
            .map_err(|e| ProxyError::AddressResolutionFailed(e.to_string()))?
            .to_owned();
        let client_config = crate::verifier::client_config(danger_accept_invalid_cert)
            .map_err(|e| ProxyError::IoError(std::io::Error::other(e)))?;
        let tls_connector = RustlsTlsConnector::from(std::sync::Arc::new(client_config));
        let stream = tls_connector.connect(domain, io).await?;
        self.http_connect(stream, target_addr).await
    }

    pub async fn http_connect<'a, Input>(
        &self,
        io: Input,
        target_addr: &TargetAddr<'a>,
    ) -> Result<BufStream<Input>, ProxyError>
    where
        Input: AsyncRead + AsyncWrite + Unpin,
    {
        let mut stream = BufStream::new(io);
        let (domain, port) = get_domain_and_port(target_addr)?;

        let request = self.make_request(&domain, port);
        stream.write_all(request.as_bytes()).await?;
        stream.flush().await?;
        recv_and_check_response(&mut stream).await?;
        Ok(stream)
    }

    fn make_request(&self, host: &str, port: u16) -> String {
        let mut request = format!(
            "CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n",
            host = host,
            port = port
        );

        if let Some(auth) = self.intercept.maybe_auth() {
            request = format!("{}{}", request, auth.get_proxy_authorization());
        }

        request.push_str("\r\n");
        request
    }
}

fn get_domain_and_port<'a>(target_addr: &TargetAddr<'a>) -> Result<(String, u16), ProxyError> {
    match target_addr {
        tokio_socks::TargetAddr::Ip(addr) => Ok((addr.ip().to_string(), addr.port())),
        tokio_socks::TargetAddr::Domain(name, port) => Ok((name.to_string(), *port)),
    }
}

async fn get_response<IO>(stream: &mut BufStream<IO>) -> Result<String, ProxyError>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    let mut response = String::new();

    loop {
        if stream.read_line(&mut response).await? == 0 {
            return Err(ProxyError::EndOfFile);
        }

        if MAXIMUM_RESPONSE_HEADER_LENGTH < response.len() {
            return Err(ProxyError::MaximumResponseHeaderLengthExceeded(
                response.len(),
            ));
        }

        if response.ends_with("\r\n\r\n") {
            return Ok(response);
        }
    }
}

async fn recv_and_check_response<IO>(stream: &mut BufStream<IO>) -> Result<(), ProxyError>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    let response_string = get_response(stream).await?;

    let mut response_headers = [EMPTY_HEADER; MAXIMUM_RESPONSE_HEADERS];
    let mut response = Response::new(&mut response_headers);
    let response_bytes = response_string.into_bytes();
    response.parse(&response_bytes)?;

    return match response.code {
        Some(code) => {
            if code == 200 {
                Ok(())
            } else {
                Err(ProxyError::HttpCode200(code))
            }
        }
        None => Err(ProxyError::NoHttpCode),
    };
}
