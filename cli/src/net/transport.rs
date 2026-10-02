use std::time::Duration;

use crate::app_info;

/// One GET request.
#[derive(Debug, Clone, Default)]
pub struct HttpRequest {
    pub url: String,
    pub headers: Vec<(String, String)>,
}

impl HttpRequest {
    pub fn new(url: impl Into<String>) -> Self {
        HttpRequest { url: url.into(), headers: Vec::new() }
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        find_header(&self.headers, name)
    }

    pub fn set_header(&mut self, name: &str, value: impl Into<String>) {
        self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case(name));
        self.headers.push((name.to_string(), value.into()));
    }
}

#[derive(Debug, Clone, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        find_header(&self.headers, name)
    }

    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

fn find_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

/// Sends one HTTP request. Any HTTP status is a response; only "the server never answered" is an
/// error.
pub trait Transport: Send + Sync {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError>;
}

/// No connection, a timeout or a TLS failure: the server never answered.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Couldn't reach {host}. Check your internet connection and try again.")]
pub struct NetError {
    pub host: String,
    /// The underlying error, for `--verbose`-style detail.
    pub detail: String,
}

/// A non-success HTTP status that wasn't mapped to a domain error.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{host} returned {status} {}.", reason_phrase(*status))]
pub struct HttpStatusError {
    pub status: u16,
    pub host: String,
}

/// The status's reason in sentence case ("Internal server error"), as the apps word it.
pub fn reason_phrase(status: u16) -> &'static str {
    match status {
        400 => "Bad request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not found",
        408 => "Request timed out",
        409 => "Conflict",
        410 => "No longer exists",
        429 => "Too many requests",
        500 => "Internal server error",
        502 => "Bad gateway",
        503 => "Service unavailable",
        504 => "Gateway timed out",
        _ if (400..500).contains(&status) => "Client error",
        _ if (500..600).contains(&status) => "Server error",
        _ => "Unexpected status",
    }
}

/// The real network, through ureq. Its own status-as-error handling is off: the cache and the
/// GitHub client decide what a status means.
pub struct UreqTransport {
    agent: ureq::Agent,
}

impl UreqTransport {
    /// For API calls and small files: the whole request must finish within `timeout`.
    pub fn new(timeout: Duration) -> Self {
        UreqTransport { agent: agent(Some(timeout)) }
    }
}

/// An agent with the shared User-Agent and the platform's TLS. `global_timeout` None suits
/// downloads of large files (connecting and the response headers still time out).
pub(crate) fn agent(global_timeout: Option<Duration>) -> ureq::Agent {
    let builder = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .user_agent(app_info::user_agent())
        .timeout_connect(Some(Duration::from_secs(30)))
        .timeout_recv_response(Some(Duration::from_secs(60)))
        .timeout_global(global_timeout);
    #[cfg(any(target_os = "macos", windows))]
    let builder = builder.tls_config(
        ureq::tls::TlsConfig::builder()
            .provider(ureq::tls::TlsProvider::NativeTls)
            .root_certs(ureq::tls::RootCerts::PlatformVerifier)
            .build(),
    );
    builder.build().new_agent()
}

pub(crate) fn net_error(url: &str, error: ureq::Error) -> NetError {
    NetError { host: super::host_of(url), detail: error.to_string() }
}

impl Transport for UreqTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError> {
        let mut builder = self.agent.get(&request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let mut response = builder.call().map_err(|e| net_error(&request.url, e))?;
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(k, v)| Some((k.as_str().to_string(), v.to_str().ok()?.to_string())))
            .collect();
        let body = response
            .body_mut()
            .with_config()
            .limit(200 * 1024 * 1024)
            .read_to_vec()
            .map_err(|e| net_error(&request.url, e))?;
        Ok(HttpResponse { status, headers, body })
    }
}
