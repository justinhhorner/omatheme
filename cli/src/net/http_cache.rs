use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{HttpRequest, HttpResponse, HttpStatusError, Transport, host_of};
use crate::json;

pub type Clock = Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>;

pub fn system_clock() -> Clock {
    Arc::new(Utc::now)
}

/// Disk-backed HTTP GET cache. Responses are stored with their ETag / Last-Modified and revalidated
/// with conditional requests (a GitHub 304 doesn't count against the rate limit). When the network
/// or server fails and a cached copy exists, the stale copy is returned with the error attached,
/// which keeps the tool usable offline. The files are the ones the apps write (data-format.md).
pub struct HttpCache {
    transport: Arc<dyn Transport>,
    dir: PathBuf,
    now: Clock,
}

pub type MapError = Box<dyn Fn(&HttpResponse) -> Option<anyhow::Error>>;

pub struct CacheOptions {
    /// How long a cached copy is used without any request. Zero always revalidates.
    pub max_age: TimeDelta,
    /// Revalidate even if the cached copy is younger than `max_age`.
    pub force_revalidate: bool,
    /// Return the cached copy (marked stale) when the server responds with an error.
    pub allow_stale_on_error: bool,
    pub headers: Vec<(String, String)>,
    /// Turns a non-success response into a domain error (e.g. GitHub rate limiting).
    pub map_error: Option<MapError>,
}

impl Default for CacheOptions {
    fn default() -> Self {
        CacheOptions {
            max_age: TimeDelta::zero(),
            force_revalidate: false,
            allow_stale_on_error: true,
            headers: Vec::new(),
            map_error: None,
        }
    }
}

#[derive(Clone)]
pub struct CachedResponse {
    pub body: Vec<u8>,
    pub fetched_at: DateTime<Utc>,
    pub is_stale: bool,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    /// Why a stale copy was served instead of a fresh one.
    pub error: Option<Arc<anyhow::Error>>,
}

impl CachedResponse {
    /// The body as text, without a byte-order mark.
    pub fn text(&self) -> String {
        let text = String::from_utf8_lossy(&self.body);
        text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_string()
    }

    fn stale(mut self, error: anyhow::Error) -> Self {
        self.is_stale = true;
        self.error = Some(Arc::new(error));
        self
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CacheMeta {
    url: String,
    #[serde(with = "json::date")]
    fetched_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    etag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_modified: Option<String>,
}

impl HttpCache {
    pub fn new(transport: Arc<dyn Transport>, dir: impl Into<PathBuf>, now: Clock) -> Self {
        HttpCache { transport, dir: dir.into(), now }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn now(&self) -> DateTime<Utc> {
        (self.now)()
    }

    pub fn get(&self, url: &str, options: &CacheOptions) -> Result<CachedResponse> {
        let cached = self.cached_response(url);

        if let Some(cached) = &cached
            && !options.force_revalidate
            && self.now() - cached.fetched_at < options.max_age
        {
            return Ok(cached.clone());
        }

        let mut request = HttpRequest::new(url);
        for (name, value) in &options.headers {
            request.set_header(name, value.clone());
        }
        if let Some(etag) = cached.as_ref().and_then(|c| c.etag.clone()) {
            request.set_header("If-None-Match", etag);
        } else if let Some(last_modified) = cached.as_ref().and_then(|c| c.last_modified.clone()) {
            request.set_header("If-Modified-Since", last_modified);
        }

        let response = match self.transport.send(&request) {
            Ok(response) => response,
            Err(error) => {
                return match cached {
                    Some(cached) => Ok(cached.stale(error.into())),
                    None => Err(error.into()),
                };
            }
        };

        if response.status == 304
            && let Some(mut refreshed) = cached.clone()
        {
            refreshed.fetched_at = self.now();
            let _ = self.write_meta(url, &refreshed);
            return Ok(refreshed);
        }

        if !response.is_success() {
            let error = options
                .map_error
                .as_ref()
                .and_then(|map| map(&response))
                .unwrap_or_else(|| HttpStatusError { status: response.status, host: host_of(url) }.into());
            return match cached {
                Some(cached) if options.allow_stale_on_error => Ok(cached.stale(error)),
                _ => Err(error),
            };
        }

        let fresh = CachedResponse {
            etag: response.header("ETag").map(str::to_string),
            last_modified: response.header("Last-Modified").map(str::to_string),
            body: response.body,
            fetched_at: self.now(),
            is_stale: false,
            error: None,
        };
        // Caching is best-effort: a full disk shouldn't turn a good response into an error.
        let _ = self.store(url, &fresh);
        Ok(fresh)
    }

    /// The cached copy without touching the network, or None.
    pub fn cached_response(&self, url: &str) -> Option<CachedResponse> {
        let (body_path, meta_path) = self.paths(url);
        let meta: CacheMeta = json::read_json(&meta_path)?;
        if meta.url != url {
            return None;
        }
        let body = std::fs::read(body_path).ok()?;
        Some(CachedResponse {
            body,
            fetched_at: meta.fetched_at,
            is_stale: false,
            etag: meta.etag,
            last_modified: meta.last_modified,
            error: None,
        })
    }

    /// Deletes every cached response. Only the cache's own `<xx>/` folders go, so other caches kept
    /// in the same folder (the macOS app's thumbnails in `images/`) are left to their owners.
    pub fn clear(&self) -> Result<()> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() == 2 && name.chars().all(|c| c.is_ascii_hexdigit()) && entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            }
        }
        Ok(())
    }

    /// Total size of the cached responses, in bytes.
    pub fn size(&self) -> u64 {
        fn walk(path: &Path) -> u64 {
            std::fs::read_dir(path)
                .map(|entries| {
                    entries
                        .flatten()
                        .map(|e| match e.metadata() {
                            Ok(m) if m.is_dir() => walk(&e.path()),
                            Ok(m) => m.len(),
                            Err(_) => 0,
                        })
                        .sum()
                })
                .unwrap_or(0)
        }
        walk(&self.dir)
    }

    fn store(&self, url: &str, response: &CachedResponse) -> Result<()> {
        json::write_atomic(&self.paths(url).0, &response.body)?;
        self.write_meta(url, response)
    }

    fn write_meta(&self, url: &str, response: &CachedResponse) -> Result<()> {
        let meta = CacheMeta {
            url: url.to_string(),
            fetched_at: response.fetched_at,
            etag: response.etag.clone(),
            last_modified: response.last_modified.clone(),
        };
        json::write_json(&self.paths(url).1, &meta)
    }

    /// cache/<xx>/<sha256 of the URL>.body and .json.
    pub(crate) fn paths(&self, url: &str) -> (PathBuf, PathBuf) {
        let key: String = Sha256::digest(url.as_bytes()).iter().map(|b| format!("{b:02x}")).collect();
        let base = self.dir.join(&key[..2]).join(&key);
        (base.with_extension("body"), base.with_extension("json"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::NetError;
    use crate::test_support::{FakeTransport, ManualClock, fixture};

    const URL: &str = "https://example.com/data.json";

    struct Harness {
        _dir: tempfile::TempDir,
        http: Arc<FakeTransport>,
        clock: ManualClock,
        cache: HttpCache,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let http = Arc::new(FakeTransport::default());
        let clock = ManualClock::at(1_790_424_000); // 2026-09-26 12:00 UTC
        let cache = HttpCache::new(http.clone(), dir.path(), clock.function());
        Harness { _dir: dir, http, clock, cache }
    }

    fn max_age(seconds: i64) -> CacheOptions {
        CacheOptions { max_age: TimeDelta::seconds(seconds), ..Default::default() }
    }

    #[test]
    fn serves_from_disk_within_max_age_without_network() {
        let h = harness();
        h.http.on_body(URL, "v1", Some("\"a\""));

        let first = h.cache.get(URL, &max_age(3600)).unwrap();
        h.clock.advance(1800);
        let second = h.cache.get(URL, &max_age(3600)).unwrap();

        assert_eq!(first.text(), "v1");
        assert_eq!(second.text(), "v1");
        assert_eq!(h.http.count(URL), 1);
    }

    #[test]
    fn revalidates_with_etag_and_uses_cached_body_on_304() {
        let h = harness();
        h.http.on_body(URL, "v1", Some("\"a\""));
        h.cache.get(URL, &CacheOptions::default()).unwrap();

        h.http.on(URL, |request| {
            assert_eq!(request.header("If-None-Match"), Some("\"a\""));
            Ok(FakeTransport::status(304))
        });
        h.clock.advance(300);
        let revalidated = h.cache.get(URL, &CacheOptions::default()).unwrap();

        assert_eq!(h.http.count(URL), 2);
        assert!(!revalidated.is_stale);
        assert_eq!(revalidated.text(), "v1"); // the 304 has no body: this is the cached one
        assert_eq!(revalidated.fetched_at, h.clock.now());
        assert_eq!(h.cache.cached_response(URL).unwrap().fetched_at, h.clock.now());
    }

    #[test]
    fn revalidates_with_last_modified_when_there_is_no_etag() {
        let h = harness();
        h.http.on(URL, |_| {
            Ok(HttpResponse {
                status: 200,
                headers: vec![("Last-Modified".into(), "Sat, 26 Sep 2026 11:00:00 GMT".into())],
                body: b"v1".to_vec(),
            })
        });
        h.cache.get(URL, &CacheOptions::default()).unwrap();
        h.http.on(URL, |request| {
            assert_eq!(request.header("If-Modified-Since"), Some("Sat, 26 Sep 2026 11:00:00 GMT"));
            assert_eq!(request.header("If-None-Match"), None);
            Ok(FakeTransport::status(304))
        });

        assert_eq!(h.cache.get(URL, &CacheOptions::default()).unwrap().text(), "v1");
    }

    #[test]
    fn replaces_cache_when_content_changes() {
        let h = harness();
        h.http.on_body(URL, "v1", Some("\"a\""));
        h.cache.get(URL, &CacheOptions::default()).unwrap();
        h.http.on_body(URL, "v2", Some("\"b\""));

        let updated = h.cache.get(URL, &CacheOptions::default()).unwrap();

        assert_eq!(updated.text(), "v2");
        assert_eq!(h.cache.cached_response(URL).unwrap().text(), "v2");
    }

    #[test]
    fn returns_stale_copy_when_offline() {
        let h = harness();
        h.http.on_body(URL, "v1", None);
        h.cache.get(URL, &CacheOptions::default()).unwrap();
        h.http.on(URL, |_| Err(FakeTransport::offline()));

        let stale = h.cache.get(URL, &CacheOptions::default()).unwrap();

        assert!(stale.is_stale);
        assert_eq!(stale.text(), "v1");
        assert!(stale.error.unwrap().is::<NetError>());
    }

    #[test]
    fn throws_when_offline_with_nothing_cached() {
        let h = harness();
        h.http.on(URL, |_| Err(FakeTransport::offline()));
        let error = h.cache.get(URL, &CacheOptions::default()).err().unwrap();
        assert!(error.is::<NetError>());
    }

    #[test]
    fn maps_server_errors_through_options() {
        let h = harness();
        h.http.on(URL, |_| Ok(FakeTransport::status(500)));
        let options =
            CacheOptions { map_error: Some(Box::new(|_| Some(anyhow::anyhow!("mapped")))), ..Default::default() };

        assert_eq!(h.cache.get(URL, &options).err().unwrap().to_string(), "mapped");
    }

    #[test]
    fn unmapped_server_errors_describe_the_status() {
        let h = harness();
        h.http.on(URL, |_| Ok(FakeTransport::status(500)));

        let error = h.cache.get(URL, &CacheOptions::default()).err().unwrap();
        assert!(error.is::<HttpStatusError>());
        assert_eq!(error.to_string(), "example.com returned 500 Internal server error.");
    }

    #[test]
    fn server_errors_serve_the_stale_copy_unless_told_not_to() {
        let h = harness();
        h.http.on_body(URL, "v1", None);
        h.cache.get(URL, &CacheOptions::default()).unwrap();
        h.http.on(URL, |_| Ok(FakeTransport::status(503)));

        assert!(h.cache.get(URL, &CacheOptions::default()).unwrap().is_stale);
        let strict = CacheOptions { allow_stale_on_error: false, ..Default::default() };
        assert!(h.cache.get(URL, &strict).is_err());
    }

    /// The macOS app keeps its thumbnail cache (a live URLCache) in the same folder; clearing must
    /// not delete its files from under it.
    #[test]
    fn clear_deletes_responses_but_not_other_caches_in_the_folder() {
        let h = harness();
        h.http.on_body(URL, "v1", None);
        h.cache.get(URL, &CacheOptions::default()).unwrap();
        let thumbnails = h.cache.dir().join("images/Cache.db");
        json::write_atomic(&thumbnails, b"db").unwrap();

        h.cache.clear().unwrap();

        assert!(h.cache.cached_response(URL).is_none());
        let names: Vec<_> = std::fs::read_dir(h.cache.dir()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["images"]);
        assert!(thumbnails.exists());
    }

    #[test]
    fn strips_byte_order_mark() {
        let h = harness();
        h.http.on_body(URL, "\u{FEFF}hello", None);
        assert_eq!(h.cache.get(URL, &CacheOptions::default()).unwrap().text(), "hello");
    }

    #[test]
    fn a_failed_cache_write_still_returns_the_response() {
        let h = harness();
        // A file where the shard folder should be makes every cache write fail.
        let (body, _) = h.cache.paths(URL);
        std::fs::create_dir_all(h.cache.dir()).unwrap();
        std::fs::write(body.parent().unwrap(), b"not a folder").unwrap();
        h.http.on_body(URL, "v1", None);

        assert_eq!(h.cache.get(URL, &CacheOptions::default()).unwrap().text(), "v1");
        assert!(h.cache.cached_response(URL).is_none());
    }

    #[test]
    fn metadata_is_written_in_the_shared_format() {
        let h = harness();
        h.http.on(URL, |_| {
            Ok(HttpResponse {
                status: 200,
                headers: vec![
                    ("ETag".into(), "\"a\"".into()),
                    ("Last-Modified".into(), "Sat, 26 Sep 2026 11:00:00 GMT".into()),
                ],
                body: b"v1".to_vec(),
            })
        });

        h.cache.get(URL, &CacheOptions::default()).unwrap();

        let written: serde_json::Value = json::read_json(&h.cache.paths(URL).1).unwrap();
        let expected: serde_json::Value = serde_json::from_str(&fixture("data/http-cache-meta.json")).unwrap();
        assert_eq!(written, expected);
    }

    #[test]
    fn metadata_reads_in_every_format() {
        for file in
            ["data/http-cache-meta.json", "data/legacy/windows-cache-meta.json", "data/legacy/macos-cache-meta.json"]
        {
            let h = harness();
            let (body, meta) = h.cache.paths(URL);
            json::write_atomic(&meta, fixture(file).as_bytes()).unwrap();
            json::write_atomic(&body, b"v1").unwrap();

            let cached = h.cache.cached_response(URL).unwrap_or_else(|| panic!("{file}"));

            assert_eq!(cached.text(), "v1");
            assert_eq!(cached.fetched_at, h.clock.now(), "{file}");
            // Windows v0.1 wrote "eTag" and an ISO Last-Modified; the ETag isn't read any more, which
            // only costs one full download of that URL.
            if file.contains("windows") {
                assert_eq!(cached.etag, None);
            } else {
                assert_eq!(cached.etag.as_deref(), Some("\"a\""));
                assert_eq!(cached.last_modified.as_deref(), Some("Sat, 26 Sep 2026 11:00:00 GMT"));
            }
        }
    }
}
