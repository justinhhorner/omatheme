//! Fakes and fixtures shared by the unit tests (like the apps' TestSupport).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, TimeDelta, TimeZone, Utc};

use crate::net::{Clock, HttpRequest, HttpResponse, NetError, Transport};

/// A file from the repo's shared `fixtures/` folder.
pub fn fixture(name: &str) -> String {
    std::fs::read_to_string(fixture_path(name)).unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

pub fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures").join(name)
}

/// A GitHub recursive-tree response.
pub fn tree_json(items: &[(&str, &str, Option<u64>)]) -> String {
    let tree: Vec<_> = items
        .iter()
        .map(|(path, kind, size)| {
            let mut entry = serde_json::json!({ "path": path, "type": kind });
            if let Some(size) = size {
                entry["size"] = (*size).into();
            }
            entry
        })
        .collect();
    serde_json::json!({ "sha": "abc", "truncated": false, "tree": tree }).to_string()
}

type Handler = Box<dyn Fn(&HttpRequest) -> Result<HttpResponse, NetError> + Send + Sync>;

/// Routes requests by exact URL; unknown URLs are 404s. Records every request.
#[derive(Default)]
pub struct FakeTransport {
    routes: Mutex<HashMap<String, Arc<Handler>>>,
    log: Mutex<Vec<HttpRequest>>,
}

impl FakeTransport {
    pub fn on(
        &self,
        url: &str,
        handler: impl Fn(&HttpRequest) -> Result<HttpResponse, NetError> + Send + Sync + 'static,
    ) {
        self.routes.lock().unwrap().insert(url.to_string(), Arc::new(Box::new(handler)));
    }

    /// 200 with `body` (and an ETag); a matching If-None-Match gets a 304.
    pub fn on_body(&self, url: &str, body: &str, etag: Option<&str>) {
        let body = body.as_bytes().to_vec();
        let etag = etag.map(str::to_string);
        self.on(url, move |request| {
            if let Some(etag) = &etag
                && request.header("If-None-Match") == Some(etag.as_str())
            {
                return Ok(Self::status(304));
            }
            let headers = etag.iter().map(|e| ("ETag".to_string(), e.clone())).collect();
            Ok(HttpResponse { status: 200, headers, body: body.clone() })
        });
    }

    pub fn status(status: u16) -> HttpResponse {
        HttpResponse { status, headers: Vec::new(), body: Vec::new() }
    }

    pub fn status_with(status: u16, headers: &[(&str, &str)]) -> HttpResponse {
        HttpResponse {
            status,
            headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            body: Vec::new(),
        }
    }

    pub fn offline() -> NetError {
        NetError { host: "example.com".into(), detail: "offline".into() }
    }

    pub fn count(&self, url: &str) -> usize {
        self.log.lock().unwrap().iter().filter(|r| r.url == url).count()
    }

    pub fn requests(&self) -> Vec<HttpRequest> {
        self.log.lock().unwrap().clone()
    }
}

impl Transport for FakeTransport {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, NetError> {
        self.log.lock().unwrap().push(request.clone());
        let handler = self.routes.lock().unwrap().get(&request.url).cloned();
        match handler {
            Some(handler) => handler(request),
            None => Ok(Self::status(404)),
        }
    }
}

/// A clock tests move by hand.
#[derive(Clone)]
pub struct ManualClock(Arc<Mutex<DateTime<Utc>>>);

impl ManualClock {
    pub fn at(unix_seconds: i64) -> Self {
        ManualClock(Arc::new(Mutex::new(Utc.timestamp_opt(unix_seconds, 0).unwrap())))
    }

    pub fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }

    pub fn advance(&self, seconds: i64) {
        *self.0.lock().unwrap() += TimeDelta::seconds(seconds);
    }

    pub fn function(&self) -> Clock {
        let inner = self.0.clone();
        Arc::new(move || *inner.lock().unwrap())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct FakeFailure(pub String);

/// Writes the URL into each file instead of downloading; can be told to fail a URL.
#[derive(Default)]
pub struct FakeDownloader {
    failing: Mutex<Vec<String>>,
    log: Mutex<Vec<String>>,
}

impl FakeDownloader {
    pub fn fail(&self, url: &str) {
        self.failing.lock().unwrap().push(url.to_string());
    }

    pub fn count(&self, url: &str) -> usize {
        self.log.lock().unwrap().iter().filter(|u| *u == url).count()
    }
}

impl crate::store::Downloader for FakeDownloader {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        progress: &mut dyn FnMut(u64),
        cancel: &crate::cancel::CancelToken,
    ) -> anyhow::Result<()> {
        cancel.check()?;
        self.log.lock().unwrap().push(url.to_string());
        if self.failing.lock().unwrap().iter().any(|u| u == url) {
            return Err(NetError { host: crate::net::host_of(url), detail: "offline".into() }.into());
        }
        progress(0);
        std::fs::write(destination, url.as_bytes())?;
        progress(url.len() as u64);
        Ok(())
    }
}

/// Records every call instead of changing the desktop; can be told to fail a call.
pub struct FakeDesktopBackend {
    capabilities: Mutex<crate::theming::DesktopCapabilities>,
    calls: Mutex<Vec<String>>,
    failing: Mutex<Vec<String>>,
    restored: Mutex<Option<crate::theming::DesktopSnapshot>>,
}

impl Default for FakeDesktopBackend {
    fn default() -> Self {
        FakeDesktopBackend {
            capabilities: Mutex::new(crate::theming::DesktopCapabilities::ALL),
            calls: Default::default(),
            failing: Default::default(),
            restored: Default::default(),
        }
    }
}

impl FakeDesktopBackend {
    pub fn snapshot() -> crate::theming::DesktopSnapshot {
        crate::theming::DesktopSnapshot {
            taken_at: Utc.timestamp_millis_opt(1_790_380_800_250).unwrap(),
            values: [("wallpaper".to_string(), "/original.jpg".to_string())].into(),
        }
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }

    pub fn clear_calls(&self) {
        self.calls.lock().unwrap().clear();
    }

    pub fn fail_on(&self, call: &str) {
        self.failing.lock().unwrap().push(call.to_string());
    }

    pub fn set_capabilities(&self, capabilities: crate::theming::DesktopCapabilities) {
        *self.capabilities.lock().unwrap() = capabilities;
    }

    pub fn restored(&self) -> Option<crate::theming::DesktopSnapshot> {
        self.restored.lock().unwrap().clone()
    }

    fn call(&self, name: &str, record: String) -> anyhow::Result<()> {
        self.calls.lock().unwrap().push(record);
        if self.failing.lock().unwrap().iter().any(|f| f == name) {
            return Err(FakeFailure(format!("{name} exploded")).into());
        }
        Ok(())
    }
}

impl crate::theming::DesktopBackend for FakeDesktopBackend {
    fn capabilities(&self) -> crate::theming::DesktopCapabilities {
        *self.capabilities.lock().unwrap()
    }

    fn supported_fits(&self) -> Vec<crate::theming::WallpaperFit> {
        crate::theming::WallpaperFit::ALL.to_vec()
    }

    fn capture(&self) -> anyhow::Result<crate::theming::DesktopSnapshot> {
        self.call("capture", "capture".into())?;
        Ok(Self::snapshot())
    }

    fn restore(&self, snapshot: &crate::theming::DesktopSnapshot) -> anyhow::Result<()> {
        self.call("restore", "restore".into())?;
        *self.restored.lock().unwrap() = Some(snapshot.clone());
        Ok(())
    }

    fn set_wallpaper(
        &self,
        image: &Path,
        fit: crate::theming::WallpaperFit,
        fill_color: Option<crate::color::RgbColor>,
    ) -> anyhow::Result<()> {
        let name = image.file_name().unwrap().to_string_lossy();
        let fill = fill_color.map(|c| c.hex()).unwrap_or_else(|| "none".into());
        self.call("wallpaper", format!("wallpaper:{name}:{fit}:{fill}"))
    }

    fn set_appearance_mode(&self, mode: crate::palette::AppearanceMode) -> anyhow::Result<()> {
        self.call("mode", format!("mode:{mode}"))
    }

    fn set_accent_color(&self, accent: crate::color::RgbColor) -> anyhow::Result<()> {
        self.call("accent", format!("accent:{accent}"))
    }
}
