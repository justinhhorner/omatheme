use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use super::downloader::Downloader;
use super::installed::{InstalledTheme, MANIFEST_FILE, WALLPAPERS_FOLDER};
use crate::cancel::{CancelToken, is_cancelled};
use crate::json;
use crate::net::Clock;
use crate::paths::{AppPaths, is_valid_slug};
use crate::resolver::ThemeDetails;

/// Progress through one theme's files: the wallpapers in order, then the screenshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadProgress {
    pub file_index: usize,
    pub bytes_received: u64,
}

#[derive(Debug, thiserror::Error)]
#[error("{0} has neither wallpapers nor a readable palette, so there's nothing to download.")]
pub struct NothingToDownloadError(pub String);

// Hidden working folders beside the themes (listing skips names starting with a dot). The names
// are the Windows app's, so either client tidies up after the other.
const STAGING_PREFIX: &str = ".staging-";
const PREVIOUS_PREFIX: &str = ".old-";
const REMOVED_PREFIX: &str = ".removed-";

/// Downloaded themes in `themes/<slug>/`.
pub struct ThemeStore {
    paths: AppPaths,
    downloader: Arc<dyn Downloader>,
    now: Clock,
    /// Renames a folder; replaceable so tests can make a move fail.
    move_dir: fn(&Path, &Path) -> std::io::Result<()>,
}

impl ThemeStore {
    pub fn new(paths: AppPaths, downloader: Arc<dyn Downloader>, now: Clock) -> Self {
        ThemeStore { paths, downloader, now, move_dir: |from, to| std::fs::rename(from, to) }
    }

    /// Newest first.
    pub fn list(&self) -> Vec<InstalledTheme> {
        let Ok(entries) = std::fs::read_dir(self.paths.themes_dir()) else {
            return Vec::new();
        };
        let mut themes: Vec<_> = entries
            .flatten()
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| load(&e.path()))
            .collect();
        themes.sort_by_key(|t| std::cmp::Reverse(t.downloaded_at));
        themes
    }

    pub fn get(&self, slug: &str) -> Option<InstalledTheme> {
        load(&self.paths.theme_dir(slug).ok()?)
    }

    /// Downloads every wallpaper (plus the screenshot) into a staging folder and swaps it in, so a
    /// cancelled or failed download never leaves a half-installed theme.
    pub fn install(
        &self,
        details: &ThemeDetails,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelToken,
    ) -> Result<InstalledTheme> {
        if !details.can_apply() {
            return Err(NothingToDownloadError(details.entry.name.clone()).into());
        }
        let slug = &details.entry.slug;
        let final_dir = self.paths.theme_dir(slug)?;
        let staging = self.paths.themes_dir().join(format!("{STAGING_PREFIX}{slug}-{}", json::unique_id()));
        std::fs::create_dir_all(staging.join(WALLPAPERS_FOLDER))?;

        let result = self.install_into(details, &staging, &final_dir, progress, cancel);
        if staging.exists() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        result
    }

    fn install_into(
        &self,
        details: &ThemeDetails,
        staging: &Path,
        final_dir: &Path,
        progress: &mut dyn FnMut(DownloadProgress),
        cancel: &CancelToken,
    ) -> Result<InstalledTheme> {
        let mut wallpaper_names = Vec::new();
        let mut used = HashSet::new();
        for (index, wallpaper) in details.wallpapers.iter().enumerate() {
            let name = unique_file_name(wallpaper.file_name(), &mut used);
            progress(DownloadProgress { file_index: index, bytes_received: 0 });
            cancel.check()?;
            self.downloader.download(
                &wallpaper.download_url,
                &staging.join(WALLPAPERS_FOLDER).join(&name),
                &mut |bytes| progress(DownloadProgress { file_index: index, bytes_received: bytes }),
                cancel,
            )?;
            wallpaper_names.push(name);
        }

        // The screenshot is a nice-to-have for offline browsing; don't fail the install over it.
        let mut screenshot_file = None;
        if let Some(url) = &details.entry.screenshot_url {
            let ext = url::Url::parse(url)
                .ok()
                .and_then(|u| u.path_segments().and_then(|mut s| s.next_back()).map(str::to_string))
                .and_then(|name| name.rsplit_once('.').map(|(_, e)| e.to_lowercase()))
                .filter(|e| !e.is_empty())
                .unwrap_or_else(|| "webp".to_string());
            let candidate = format!("screenshot.{ext}");
            progress(DownloadProgress { file_index: details.wallpapers.len(), bytes_received: 0 });
            match self.downloader.download(url, &staging.join(&candidate), &mut |_| {}, cancel) {
                Ok(()) => screenshot_file = Some(candidate),
                Err(error) if is_cancelled(&error) => return Err(error),
                Err(_) => {}
            }
        }
        cancel.check()?;

        let mut theme = InstalledTheme {
            slug: details.entry.slug.clone(),
            name: details.entry.name.clone(),
            repo_url: details.entry.repo_url.clone(),
            palette: details.palette.clone(),
            mode: details.mode,
            wallpapers: wallpaper_names,
            screenshot_file,
            downloaded_at: (self.now)(),
            directory: PathBuf::new(),
        };
        json::write_json(&staging.join(MANIFEST_FILE), &theme)?;

        self.replace_installed(&details.entry.slug, staging, final_dir)?;
        theme.directory = final_dir.to_path_buf();
        Ok(theme)
    }

    /// Swaps a staged theme in for the installed copy. The old folder is renamed aside first and
    /// deleted only once the new one is in place, so a failure at any point leaves a complete copy.
    fn replace_installed(&self, slug: &str, staging: &Path, final_dir: &Path) -> Result<()> {
        let mut previous = None;
        if final_dir.exists() {
            let aside = self.paths.themes_dir().join(format!("{PREVIOUS_PREFIX}{slug}-{}", json::unique_id()));
            (self.move_dir)(final_dir, &aside)?;
            previous = Some(aside);
        }
        if let Err(error) = (self.move_dir)(staging, final_dir) {
            // Put the old copy back. If even that fails, clean_up puts it back next time.
            if let Some(previous) = &previous {
                let _ = (self.move_dir)(previous, final_dir);
            }
            return Err(error.into());
        }
        if let Some(previous) = previous {
            let _ = std::fs::remove_dir_all(previous);
        }
        Ok(())
    }

    /// Renames the theme aside before deleting it, so a locked file fails the removal with the
    /// theme still whole rather than half-deleted.
    pub fn remove(&self, slug: &str) -> Result<()> {
        let dir = self.paths.theme_dir(slug)?;
        if !dir.exists() {
            return Ok(());
        }
        let removed = self.paths.themes_dir().join(format!("{REMOVED_PREFIX}{slug}-{}", json::unique_id()));
        (self.move_dir)(&dir, &removed)?;
        let _ = std::fs::remove_dir_all(removed);
        Ok(())
    }

    /// Tidies up after an interrupted download, reinstall or removal: deletes staging and removed
    /// folders, and puts back a previous copy that a failed reinstall left aside. Only folders older
    /// than `min_age` are touched, so a download the app is running right now is left alone.
    pub fn clean_up(&self, min_age: Duration) {
        let Ok(entries) = std::fs::read_dir(self.paths.themes_dir()) else {
            return;
        };
        let old_enough = |path: &Path| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .map(|modified| SystemTime::now().duration_since(modified).unwrap_or_default() >= min_age)
                .unwrap_or(false)
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if !path.is_dir() || !old_enough(&path) {
                continue;
            }
            if name.starts_with(STAGING_PREFIX) || name.starts_with(REMOVED_PREFIX) {
                let _ = std::fs::remove_dir_all(&path);
            } else if name.starts_with(PREVIOUS_PREFIX) {
                match slug_of_previous(&name).and_then(|slug| self.paths.theme_dir(&slug).ok()) {
                    Some(target) if !target.exists() => {
                        let _ = (self.move_dir)(&path, &target);
                    }
                    _ => {
                        let _ = std::fs::remove_dir_all(&path);
                    }
                }
            }
        }
    }
}

fn load(dir: &Path) -> Option<InstalledTheme> {
    let mut theme: InstalledTheme = json::read_json(&dir.join(MANIFEST_FILE))?;
    theme.directory = dir.to_path_buf();
    Some(theme)
}

/// "tokyo" from ".old-tokyo-<32 hex digits>", or None if the name isn't one of ours.
fn slug_of_previous(folder_name: &str) -> Option<String> {
    const ID_LENGTH: usize = 32;
    let rest = folder_name.strip_prefix(PREVIOUS_PREFIX)?;
    if rest.len() <= ID_LENGTH + 1 || rest.as_bytes()[rest.len() - ID_LENGTH - 1] != b'-' {
        return None;
    }
    let slug = &rest[..rest.len() - ID_LENGTH - 1];
    is_valid_slug(slug).then(|| slug.to_string())
}

/// A file name that's safe on every OS and unique (case-insensitively, as APFS and NTFS are).
fn unique_file_name(file_name: &str, used: &mut HashSet<String>) -> String {
    let mut safe: String =
        file_name
            .chars()
            .map(|c| {
                if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '_'
                } else {
                    c
                }
            })
            .collect();
    if safe.trim().is_empty() || safe.starts_with('.') {
        safe = format!("wallpaper{safe}");
    }
    let (stem, ext) = match safe.rfind('.') {
        Some(i) if i > 0 => (safe[..i].to_string(), safe[i..].to_string()),
        _ => (safe.clone(), String::new()),
    };
    let mut candidate = safe.clone();
    let mut n = 2;
    while !used.insert(candidate.to_lowercase()) {
        candidate = format!("{stem}-{n}{ext}");
        n += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CatalogEntry;
    use crate::color::c;
    use crate::github::{GitHubClient, RepoRef};
    use crate::palette::{AppearanceMode, Palette, PaletteSource};
    use crate::resolver::WallpaperRef;
    use crate::test_support::{FakeDownloader, ManualClock};

    struct Harness {
        dir: tempfile::TempDir,
        downloader: Arc<FakeDownloader>,
        clock: ManualClock,
        store: ThemeStore,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let downloader = Arc::new(FakeDownloader::default());
        let clock = ManualClock::at(1_790_380_800); // 2026-09-26 UTC
        let store = ThemeStore::new(AppPaths::new(dir.path()), downloader.clone(), clock.function());
        Harness { dir, downloader, clock, store }
    }

    impl Harness {
        fn themes_dir_contents(&self) -> Vec<String> {
            let mut names: Vec<_> = std::fs::read_dir(self.dir.path().join("themes"))
                .map(|e| e.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect())
                .unwrap_or_default();
            names.sort();
            names
        }

        fn install(&self, details: &ThemeDetails) -> Result<InstalledTheme> {
            self.store.install(details, &mut |_| {}, &CancelToken::new())
        }
    }

    fn details(slug: &str, wallpapers: &[&str], palette: bool) -> ThemeDetails {
        let repo = RepoRef::new("o", slug);
        ThemeDetails {
            entry: CatalogEntry {
                slug: slug.into(),
                name: "Tokyo".into(),
                repo_url: format!("https://github.com/o/{slug}"),
                screenshot_url: Some(format!("https://omarchy.org/assets/themes/{slug}.webp")),
            },
            palette: palette.then(|| Palette::new(c("#1a1b26"), c("#a9b1d6"), c("#7aa2f7"), PaletteSource::ColorsToml)),
            palette_error: None,
            wallpapers: wallpapers
                .iter()
                .map(|name| {
                    let path = format!("backgrounds/{name}");
                    WallpaperRef { download_url: GitHubClient::raw_url(&repo, &path), path, size: Some(100) }
                })
                .collect(),
            repo,
            mode: AppearanceMode::Dark,
            is_stale: false,
            stale_reason: None,
        }
    }

    #[test]
    fn install_downloads_wallpapers_and_screenshot_and_writes_manifest() {
        let h = harness();
        let mut reports = Vec::new();

        let installed = h
            .store
            .install(&details("tokyo", &["1.png", "2.jpg"], true), &mut |p| reports.push(p), &CancelToken::new())
            .unwrap();

        assert_eq!(installed.wallpapers, ["1.png", "2.jpg"]);
        assert!(installed.wallpaper_path("1.png").exists());
        assert!(installed.screenshot_path().unwrap().exists());
        assert_eq!(installed.downloaded_at, h.clock.now());

        let reloaded = h.store.get("tokyo").unwrap();
        assert_eq!(reloaded.directory, installed.directory);
        assert_eq!(reloaded.slug, "tokyo");
        assert_eq!(reloaded.name, "Tokyo");
        assert_eq!(reloaded.repo_url, "https://github.com/o/tokyo");
        assert_eq!(reloaded.mode, AppearanceMode::Dark);
        assert_eq!(reloaded.downloaded_at, h.clock.now());
        assert_eq!(reloaded.wallpapers, installed.wallpapers);
        assert_eq!(reloaded.screenshot_file, installed.screenshot_file);
        assert_eq!(reloaded.palette.unwrap().accent, c("#7aa2f7"));
        assert_eq!(h.store.list().len(), 1);
        // Both wallpapers, then the screenshot.
        let files: HashSet<_> = reports.iter().map(|p| p.file_index).collect();
        assert_eq!(files, HashSet::from([0, 1, 2]));
        // Bytes are reported per file, ending at the file's size.
        assert!(reports.iter().any(|p| p.file_index == 0 && p.bytes_received > 0));
    }

    #[test]
    fn failed_download_leaves_no_partial_theme_behind() {
        let h = harness();
        let d = details("tokyo", &["1.png", "2.png"], true);
        h.downloader.fail(&d.wallpapers[1].download_url);

        assert!(h.install(&d).is_err());

        assert!(h.store.get("tokyo").is_none());
        assert!(h.themes_dir_contents().is_empty());
    }

    #[test]
    fn cancelled_download_leaves_no_partial_theme_behind() {
        let h = harness();
        let cancel = CancelToken::new();
        cancel.cancel();

        let error = h.store.install(&details("tokyo", &["1.png"], true), &mut |_| {}, &cancel).err().unwrap();

        assert!(is_cancelled(&error));
        assert!(h.themes_dir_contents().is_empty());
    }

    #[test]
    fn cancelling_mid_download_stops_and_cleans_up() {
        let h = harness();
        let cancel = CancelToken::new();
        let d = details("tokyo", &["1.png", "2.png"], true);
        let cancel_after_first = cancel.clone();
        let error = h
            .store
            .install(
                &d,
                &mut |p| {
                    if p.file_index == 1 {
                        cancel_after_first.cancel()
                    }
                },
                &cancel,
            )
            .err()
            .unwrap();

        assert!(is_cancelled(&error));
        assert_eq!(h.downloader.count(&d.wallpapers[1].download_url), 0);
        assert!(h.themes_dir_contents().is_empty());
    }

    #[test]
    fn screenshot_failure_does_not_fail_install() {
        let h = harness();
        let d = details("tokyo", &["1.png"], true);
        h.downloader.fail(d.entry.screenshot_url.as_ref().unwrap());

        let installed = h.install(&d).unwrap();

        assert_eq!(installed.screenshot_file, None);
        assert!(h.store.get("tokyo").is_some());
    }

    #[test]
    fn reinstall_replaces_the_previous_copy() {
        let h = harness();
        h.install(&details("tokyo", &["old.png"], true)).unwrap();
        let installed = h.install(&details("tokyo", &["new.png"], true)).unwrap();

        assert_eq!(installed.wallpapers, ["new.png"]);
        assert!(!installed.wallpaper_path("old.png").exists());
        assert_eq!(h.store.get("tokyo").unwrap().wallpapers, ["new.png"]);
        assert_eq!(h.themes_dir_contents(), ["tokyo"]); // no staging or backup folder left behind
    }

    #[test]
    fn a_failed_swap_keeps_the_previous_copy() {
        let mut h = harness();
        h.install(&details("tokyo", &["old.png"], true)).unwrap();
        // Moving the staged folder into place fails; moving the old copy aside and back works.
        h.store.move_dir = |from, to| {
            if from.file_name().unwrap().to_string_lossy().starts_with(STAGING_PREFIX) {
                Err(std::io::Error::other("locked"))
            } else {
                std::fs::rename(from, to)
            }
        };

        assert!(h.install(&details("tokyo", &["new.png"], true)).is_err());

        assert_eq!(h.store.get("tokyo").unwrap().wallpapers, ["old.png"]);
        assert_eq!(h.themes_dir_contents(), ["tokyo"]);
    }

    #[test]
    fn duplicate_wallpaper_names_are_made_unique() {
        let h = harness();
        let mut d = details("tokyo", &["a.png"], true);
        let mut second = d.wallpapers[0].clone();
        second.path = "backgrounds/dark/A.png".into();
        d.wallpapers.push(second);

        assert_eq!(h.install(&d).unwrap().wallpapers, ["a.png", "A-2.png"]);
    }

    #[test]
    fn unsafe_wallpaper_names_are_made_safe() {
        let mut used = HashSet::new();
        assert_eq!(unique_file_name("a:b?.png", &mut used), "a_b_.png");
        assert_eq!(unique_file_name(".hidden.png", &mut used), "wallpaper.hidden.png");
        assert_eq!(unique_file_name("noext", &mut used), "noext");
        assert_eq!(unique_file_name("NOEXT", &mut used), "NOEXT-2");
    }

    #[test]
    fn remove_deletes_the_theme() {
        let h = harness();
        h.install(&details("tokyo", &["1.png"], true)).unwrap();

        h.store.remove("tokyo").unwrap();

        assert!(h.store.get("tokyo").is_none());
        assert!(h.store.list().is_empty());
        assert!(h.themes_dir_contents().is_empty());
    }

    #[test]
    fn a_removal_that_cannot_move_the_theme_leaves_it_whole() {
        let mut h = harness();
        h.install(&details("tokyo", &["1.png"], true)).unwrap();
        h.store.move_dir = |_, _| Err(std::io::Error::other("in use"));

        assert!(h.store.remove("tokyo").is_err());
        assert_eq!(h.store.get("tokyo").unwrap().wallpapers, ["1.png"]);
    }

    #[test]
    fn list_is_newest_first_and_ignores_junk() {
        let h = harness();
        h.install(&details("older", &["1.png"], true)).unwrap();
        h.clock.advance(60);
        h.install(&details("newer", &["1.png"], true)).unwrap();
        std::fs::create_dir_all(h.dir.path().join("themes/no-manifest")).unwrap();

        let slugs: Vec<_> = h.store.list().into_iter().map(|t| t.slug).collect();
        assert_eq!(slugs, ["newer", "older"]);
    }

    #[test]
    fn theme_without_wallpapers_or_palette_cannot_be_installed() {
        let h = harness();
        assert!(h.install(&details("tokyo", &[], false)).err().unwrap().is::<NothingToDownloadError>());
    }

    #[test]
    fn palette_only_theme_installs_without_wallpapers() {
        let h = harness();
        let installed = h.install(&details("tokyo", &[], true)).unwrap();
        assert!(installed.wallpapers.is_empty());
        assert!(installed.palette.is_some());
    }

    #[test]
    fn clean_up_removes_interrupted_folders_and_restores_a_copy_left_aside() {
        let h = harness();
        let themes = h.dir.path().join("themes");
        let id = json::unique_id();
        std::fs::create_dir_all(themes.join(format!(".staging-tokyo-{id}"))).unwrap();
        std::fs::create_dir_all(themes.join(format!(".removed-tokyo-{id}"))).unwrap();
        let aside = themes.join(format!(".old-snow-{id}"));
        std::fs::create_dir_all(&aside).unwrap();
        std::fs::write(aside.join("theme.json"), "{}").unwrap();

        h.store.clean_up(Duration::ZERO);

        assert_eq!(h.themes_dir_contents(), ["snow"]);
        assert!(themes.join("snow/theme.json").exists());
    }

    #[test]
    fn clean_up_leaves_recent_folders_alone() {
        let h = harness();
        let staging = h.dir.path().join("themes/.staging-tokyo-123");
        std::fs::create_dir_all(&staging).unwrap();

        h.store.clean_up(Duration::from_secs(3600));

        assert!(staging.exists());
    }

    #[test]
    fn previous_copy_names_are_recognised() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(slug_of_previous(&format!(".old-omarchy.tokyo-night-{id}")).as_deref(), Some("omarchy.tokyo-night"));
        assert_eq!(slug_of_previous(".old-tokyo-123"), None);
        assert_eq!(slug_of_previous(&format!(".old-../x-{id}")), None);
    }
}
