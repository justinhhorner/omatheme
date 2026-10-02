//! On-disk layout, shared with the GUI app on the same OS (docs/data-format.md).

use std::path::{Path, PathBuf};

use anyhow::Result;

/// The environment variable both apps read for another data folder.
pub const DATA_DIR_VAR: &str = "OMARCHY_THEMES_DATA_DIR";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPaths {
    pub root: PathBuf,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid theme slug '{0}'.")]
pub struct InvalidSlugError(pub String);

impl AppPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let root = std::path::absolute(&root).unwrap_or(root);
        AppPaths { root }
    }

    /// The GUI app's folder: `~/Library/Application Support/OmarchyThemes` on macOS,
    /// `%LOCALAPPDATA%\OmarchyThemes` on Windows, `$XDG_DATA_HOME/omarchy-themes` elsewhere.
    pub fn default_root() -> PathBuf {
        #[cfg(target_os = "macos")]
        let root = dirs::data_dir().map(|d| d.join("OmarchyThemes"));
        #[cfg(windows)]
        let root = dirs::data_local_dir().map(|d| d.join("OmarchyThemes"));
        #[cfg(not(any(target_os = "macos", windows)))]
        let root = dirs::data_dir().map(|d| d.join("omarchy-themes"));
        root.unwrap_or_else(|| PathBuf::from(".omarchy-themes"))
    }

    /// HTTP/API response cache (catalog page, GitHub trees, palette files) with their ETags.
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Downloaded themes, one folder per slug.
    pub fn themes_dir(&self) -> PathBuf {
        self.root.join("themes")
    }

    pub fn settings_file(&self) -> PathBuf {
        self.root.join("settings.json")
    }

    /// Snapshot of the desktop taken before the first apply, for restore.
    pub fn snapshot_file(&self) -> PathBuf {
        self.root.join("original-desktop.json")
    }

    /// Private copies of the original wallpapers, used by restore if the originals are gone.
    pub fn original_desktop_dir(&self) -> PathBuf {
        self.root.join("original-desktop")
    }

    /// Folder for one downloaded theme. Rejects slugs that could escape `themes/`.
    pub fn theme_dir(&self, slug: &str) -> Result<PathBuf, InvalidSlugError> {
        if !is_valid_slug(slug) {
            return Err(InvalidSlugError(slug.to_string()));
        }
        Ok(self.themes_dir().join(slug))
    }

    pub fn ensure_created(&self) -> Result<()> {
        std::fs::create_dir_all(self.cache_dir())?;
        std::fs::create_dir_all(self.themes_dir())?;
        Ok(())
    }

    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

/// True for slugs that are safe as a single folder or file name.
pub fn is_valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.chars().count() <= 100
        && slug != "."
        && slug != ".."
        && slug.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths() -> AppPaths {
        AppPaths::new(std::env::temp_dir().join("omatheme-tests"))
    }

    #[test]
    fn theme_dir_accepts_simple_slugs() {
        for slug in ["tokyo-night", "omarchy_arc.blueberry"] {
            assert_eq!(paths().theme_dir(slug).unwrap(), paths().themes_dir().join(slug));
        }
    }

    #[test]
    fn theme_dir_rejects_path_escapes() {
        for slug in ["", "..", "../evil", "a/b", "a\\b", "C:", "~"] {
            assert!(paths().theme_dir(slug).is_err(), "{slug}");
        }
    }

    #[test]
    fn layout_matches_the_apps() {
        let p = paths();
        assert!(p.cache_dir().ends_with("cache"));
        assert!(p.themes_dir().ends_with("themes"));
        assert!(p.settings_file().ends_with("settings.json"));
        assert!(p.snapshot_file().ends_with("original-desktop.json"));
        assert!(p.original_desktop_dir().ends_with("original-desktop"));
        #[cfg(target_os = "macos")]
        assert!(AppPaths::default_root().ends_with("Library/Application Support/OmarchyThemes"));
    }
}
