use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Context, Result};

use crate::cancel::{CancelToken, Cancelled};
use crate::net::reason_phrase;

/// Downloads one file, reporting the bytes received so far.
pub trait Downloader: Send + Sync {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<()>;
}

#[derive(Debug, thiserror::Error)]
#[error("Downloading {file_name} failed: {status} {}.", reason_phrase(*.status))]
pub struct DownloadError {
    pub file_name: String,
    pub status: u16,
}

/// Streams to a temporary file next to the destination, so the destination is either complete or
/// absent, and stops between chunks when cancelled.
pub struct UreqDownloader {
    agent: ureq::Agent,
}

impl Default for UreqDownloader {
    fn default() -> Self {
        // No overall timeout (wallpapers can be large); connecting and the first byte still time out.
        UreqDownloader { agent: crate::net::transport_agent(None) }
    }
}

impl Downloader for UreqDownloader {
    fn download(
        &self,
        url: &str,
        destination: &Path,
        progress: &mut dyn FnMut(u64),
        cancel: &CancelToken,
    ) -> Result<()> {
        cancel.check()?;
        let file_name = url.rsplit('/').next().unwrap_or(url).to_string();
        let mut response = self.agent.get(url).call().map_err(|e| crate::net::net_error(url, e))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(DownloadError { file_name: crate::github::percent_decode(&file_name), status }.into());
        }

        let temp = destination.with_file_name(format!(
            ".{}.{}.download",
            destination.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            crate::json::unique_id()
        ));
        let result = (|| -> Result<()> {
            let mut reader = response.body_mut().with_config().limit(u64::MAX).reader();
            let mut file =
                std::fs::File::create(&temp).with_context(|| format!("Couldn't create {}", temp.display()))?;
            let mut buffer = vec![0u8; 64 * 1024];
            let mut received = 0u64;
            progress(0);
            loop {
                if cancel.is_cancelled() {
                    return Err(Cancelled.into());
                }
                let n = reader
                    .read(&mut buffer)
                    .map_err(|e| crate::net::NetError { host: crate::net::host_of(url), detail: e.to_string() })?;
                if n == 0 {
                    break;
                }
                file.write_all(&buffer[..n])?;
                received += n as u64;
                progress(received);
            }
            file.sync_all()?;
            drop(file);
            std::fs::rename(&temp, destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result
    }
}
