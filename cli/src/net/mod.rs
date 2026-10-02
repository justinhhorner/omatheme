//! HTTP: a small transport trait (ureq for real, a routing fake in tests) and the disk cache that
//! both apps share (`cache/<xx>/<sha256>.body` + `.json`).

mod http_cache;
mod transport;

pub use http_cache::{CacheOptions, CachedResponse, Clock, HttpCache, system_clock};
pub use transport::{HttpRequest, HttpResponse, HttpStatusError, NetError, Transport, UreqTransport, reason_phrase};
pub(crate) use transport::{agent as transport_agent, net_error};

/// The host of `url`, or "The server".
pub fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| "The server".to_string())
}
