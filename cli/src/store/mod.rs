//! Local data: downloaded themes, settings, and the downloader.

mod downloader;
mod installed;
mod settings;
mod theme_store;

pub use downloader::{DownloadError, Downloader, UreqDownloader};
pub use installed::InstalledTheme;
pub use settings::{AppSettings, SettingsStore};
pub use theme_store::{DownloadProgress, NothingToDownloadError, ThemeStore};

#[cfg(test)]
mod data_format_tests;
