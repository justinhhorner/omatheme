use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::json;
use crate::palette::{AppearanceMode, Palette};

pub(crate) const MANIFEST_FILE: &str = "theme.json";
pub(crate) const WALLPAPERS_FOLDER: &str = "wallpapers";

/// A downloaded theme (`themes/<slug>/theme.json`). Everything needed to apply it is on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", from = "ManifestOnDisk")]
pub struct InstalledTheme {
    pub slug: String,
    pub name: String,
    pub repo_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<Palette>,
    pub mode: AppearanceMode,
    /// Wallpaper file names in the theme's `wallpapers/` folder, in display order.
    pub wallpapers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_file: Option<String>,
    #[serde(with = "json::date")]
    pub downloaded_at: DateTime<Utc>,
    /// Absolute folder of the theme; set when loaded from disk, never written.
    #[serde(skip)]
    pub directory: PathBuf,
}

impl InstalledTheme {
    pub fn wallpaper_path(&self, file_name: &str) -> PathBuf {
        self.directory.join(WALLPAPERS_FOLDER).join(file_name)
    }

    pub fn screenshot_path(&self) -> Option<PathBuf> {
        self.screenshot_file.as_ref().map(|f| self.directory.join(f))
    }
}

/// theme.json as any client has written it: `repoUrl` (or the older macOS `repoURL`), and a mode
/// that older files may leave to the palette. Windows v0.1's derived values are ignored.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestOnDisk {
    slug: String,
    name: String,
    #[serde(alias = "repoURL")]
    repo_url: String,
    palette: Option<Palette>,
    mode: Option<AppearanceMode>,
    #[serde(default)]
    wallpapers: Option<Vec<String>>,
    screenshot_file: Option<String>,
    #[serde(with = "json::date")]
    downloaded_at: DateTime<Utc>,
}

impl From<ManifestOnDisk> for InstalledTheme {
    fn from(m: ManifestOnDisk) -> Self {
        let mode = m.mode.or_else(|| m.palette.as_ref().map(Palette::mode)).unwrap_or(AppearanceMode::Dark);
        InstalledTheme {
            slug: m.slug,
            name: m.name,
            repo_url: m.repo_url,
            palette: m.palette,
            mode,
            wallpapers: m.wallpapers.unwrap_or_default(),
            screenshot_file: m.screenshot_file,
            downloaded_at: m.downloaded_at,
            directory: PathBuf::new(),
        }
    }
}
