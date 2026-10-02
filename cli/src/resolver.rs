//! Resolves a catalog entry to its palette and wallpapers: one GitHub API call (the repo's tree,
//! usually a free 304 on revisits), plus the palette file from raw.githubusercontent.com.

use std::cmp::Ordering;
use std::sync::Arc;

use anyhow::Result;

use crate::catalog::CatalogEntry;
use crate::github::{GitHubClient, RepoRef, RepoTree, RepoTreeItem};
use crate::palette::{AppearanceMode, Palette, PaletteParseError, parse_alacritty, parse_colors_toml};

/// A wallpaper image in a theme repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallpaperRef {
    /// Path relative to the theme root, e.g. "backgrounds/1.png".
    pub path: String,
    pub size: Option<u64>,
    pub download_url: String,
}

impl WallpaperRef {
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// Everything needed from a theme's repo to show, download and apply it.
#[derive(Clone)]
pub struct ThemeDetails {
    pub entry: CatalogEntry,
    pub repo: RepoRef,
    /// None when no supported palette file could be read; see `palette_error`.
    pub palette: Option<Palette>,
    pub palette_error: Option<String>,
    pub wallpapers: Vec<WallpaperRef>,
    pub mode: AppearanceMode,
    /// Served from cache because GitHub was unreachable or rate-limited.
    pub is_stale: bool,
    pub stale_reason: Option<Arc<anyhow::Error>>,
}

impl ThemeDetails {
    pub fn can_apply(&self) -> bool {
        !self.wallpapers.is_empty() || self.palette.is_some()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ThemeResolveError(pub String);

type Parser = fn(&str) -> Result<Palette, PaletteParseError>;

/// Palette files in priority order.
const PALETTE_FILES: [(&str, Parser); 2] = [("colors.toml", parse_colors_toml), ("alacritty.toml", parse_alacritty)];
const WALLPAPER_DIRS: [&str; 2] = ["backgrounds/", "wallpapers/"];
const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "webp", "bmp"];

pub struct ThemeResolver {
    github: GitHubClient,
}

impl ThemeResolver {
    pub fn new(github: GitHubClient) -> Self {
        ThemeResolver { github }
    }

    pub fn resolve(&self, entry: &CatalogEntry) -> Result<ThemeDetails> {
        let Some(repo) = RepoRef::parse(&entry.repo_url) else {
            return Err(ThemeResolveError(format!(
                "{} doesn't link to a GitHub repository, so it can't be read.",
                entry.name
            ))
            .into());
        };

        let tree = self.github.tree(&repo)?;
        let (mut palette, palette_error) = self.read_palette(&repo, &tree)?;
        let wallpapers = find_wallpapers(&tree)
            .into_iter()
            .map(|item| WallpaperRef {
                path: item.path.clone(),
                size: item.size,
                download_url: GitHubClient::raw_url(&repo, &item.path),
            })
            .collect();

        let has_light_mode_file = tree.find_file("light.mode").is_some();
        if let Some(palette) = &mut palette
            && palette.declared_mode.is_none()
            && has_light_mode_file
        {
            palette.declared_mode = Some(AppearanceMode::Light);
        }
        let mode = palette.as_ref().map(Palette::mode).unwrap_or(if has_light_mode_file {
            AppearanceMode::Light
        } else {
            AppearanceMode::Dark
        });

        Ok(ThemeDetails {
            entry: entry.clone(),
            repo,
            palette,
            palette_error,
            wallpapers,
            mode,
            is_stale: tree.is_stale,
            stale_reason: tree.stale_reason,
        })
    }

    fn read_palette(&self, repo: &RepoRef, tree: &RepoTree) -> Result<(Option<Palette>, Option<String>)> {
        let mut problems = Vec::new();
        for (file, parse) in PALETTE_FILES {
            let Some(item) = tree.find_file(file) else {
                continue;
            };
            let text = match self.github.raw_text(repo, &item.path) {
                Ok(text) => text,
                Err(error) if crate::cancel::is_cancelled(&error) => return Err(error),
                Err(error) => {
                    problems.push(format!("{file} couldn't be downloaded: {error}"));
                    continue;
                }
            };
            match parse(&text) {
                Ok(palette) => return Ok((Some(palette), None)),
                Err(error) => problems.push(error.to_string()),
            }
        }
        let message = if problems.is_empty() {
            "Couldn't read this theme's palette: it has no colors.toml or alacritty.toml.".to_string()
        } else {
            format!("Couldn't read this theme's palette. {}", problems.join(" "))
        };
        Ok((None, Some(message)))
    }
}

pub fn find_wallpapers(tree: &RepoTree) -> Vec<&RepoTreeItem> {
    let images: Vec<&RepoTreeItem> = tree
        .files()
        .filter(|item| {
            let ext = item.file_name().rsplit_once('.').map(|(_, e)| e.to_lowercase()).unwrap_or_default();
            IMAGE_EXTENSIONS.contains(&ext.as_str())
        })
        .collect();

    for dir in WALLPAPER_DIRS {
        let mut in_dir: Vec<_> = images.iter().copied().filter(|i| i.path.to_lowercase().starts_with(dir)).collect();
        if !in_dir.is_empty() {
            in_dir.sort_by(|a, b| natural_cmp(&a.path, &b.path));
            return in_dir;
        }
    }

    // Some repos keep a single background at the root.
    let mut root: Vec<_> = images
        .into_iter()
        .filter(|item| {
            let name = item.file_name().to_lowercase();
            !item.path.contains('/') && (name.starts_with("background") || name.starts_with("wallpaper"))
        })
        .collect();
    root.sort_by(|a, b| natural_cmp(&a.path, &b.path));
    root
}

/// Orders "2.png" before "10.png"; letters compare case-insensitively (ASCII).
pub fn natural_cmp(x: &str, y: &str) -> Ordering {
    let a: Vec<char> = x.chars().collect();
    let b: Vec<char> = y.chars().collect();
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let si = i;
            while i < a.len() && a[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            let na: String = a[si..i].iter().collect::<String>().trim_start_matches('0').to_string();
            let nb: String = b[sj..j].iter().collect::<String>().trim_start_matches('0').to_string();
            if na.len() != nb.len() {
                return na.len().cmp(&nb.len());
            }
            if na != nb {
                return na.cmp(&nb);
            }
        } else {
            let ca = a[i].to_ascii_lowercase();
            let cb = b[j].to_ascii_lowercase();
            if ca != cb {
                return ca.cmp(&cb);
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GitHubError;
    use crate::net::HttpCache;
    use crate::test_support::{FakeTransport, ManualClock, fixture, tree_json};

    const TREE_URL: &str = "https://api.github.com/repos/o/r/git/trees/HEAD?recursive=1";
    const RAW: &str = "https://raw.githubusercontent.com/o/r/HEAD/";

    fn entry() -> CatalogEntry {
        CatalogEntry {
            slug: "r".into(),
            name: "My Theme".into(),
            repo_url: "https://github.com/o/r".into(),
            screenshot_url: Some("https://omarchy.org/assets/themes/r.webp".into()),
        }
    }

    struct Harness {
        _dir: tempfile::TempDir,
        http: Arc<FakeTransport>,
        clock: ManualClock,
        cache: Arc<HttpCache>,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let http = Arc::new(FakeTransport::default());
        let clock = ManualClock::at(0);
        let cache = Arc::new(HttpCache::new(http.clone(), dir.path(), clock.function()));
        Harness { _dir: dir, http, clock, cache }
    }

    impl Harness {
        fn resolver(&self, token: Option<&str>) -> ThemeResolver {
            ThemeResolver::new(GitHubClient::new(self.cache.clone(), token))
        }
        fn resolve(&self) -> Result<ThemeDetails> {
            self.resolver(None).resolve(&entry())
        }
    }

    #[test]
    fn resolves_palette_and_naturally_sorted_wallpapers() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[
                ("colors.toml", "blob", Some(500)),
                ("alacritty.toml", "blob", Some(500)),
                ("backgrounds", "tree", None),
                ("backgrounds/10-night.png", "blob", Some(3000)),
                ("backgrounds/2-dusk.jpg", "blob", Some(2000)),
                ("backgrounds/1-day.webp", "blob", Some(1000)),
                ("backgrounds/notes.txt", "blob", Some(10)),
                ("preview.png", "blob", Some(999)),
            ]),
            None,
        );
        h.http.on_body(&format!("{RAW}colors.toml"), &fixture("colors-ansi.toml"), None);

        let details = h.resolve().unwrap();

        let palette = details.palette.as_ref().unwrap();
        assert!(details.palette_error.is_none());
        assert_eq!(palette.source, crate::palette::PaletteSource::ColorsToml);
        assert_eq!(details.mode, AppearanceMode::Dark);
        let names: Vec<_> = details.wallpapers.iter().map(|w| w.file_name()).collect();
        assert_eq!(names, ["1-day.webp", "2-dusk.jpg", "10-night.png"]);
        assert_eq!(details.wallpapers[0].download_url, format!("{RAW}backgrounds/1-day.webp"));
        assert_eq!(details.wallpapers[0].size, Some(1000));
        assert!(details.can_apply());
        assert_eq!(h.http.count(&format!("{RAW}alacritty.toml")), 0);
    }

    #[test]
    fn falls_back_to_alacritty_when_colors_toml_is_unusable() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[
                ("colors.toml", "blob", Some(5)),
                ("alacritty.toml", "blob", Some(5)),
                ("backgrounds/a.png", "blob", Some(5)),
            ]),
            None,
        );
        h.http.on_body(&format!("{RAW}colors.toml"), "# empty", None);
        h.http.on_body(&format!("{RAW}alacritty.toml"), &fixture("alacritty.toml"), None);

        assert_eq!(h.resolve().unwrap().palette.unwrap().source, crate::palette::PaletteSource::Alacritty);
    }

    #[test]
    fn reports_a_clear_palette_error_but_keeps_wallpapers() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[("README.md", "blob", Some(5)), ("backgrounds/a.png", "blob", Some(5))]),
            None,
        );

        let details = h.resolve().unwrap();

        assert!(details.palette.is_none());
        assert!(details.palette_error.as_ref().unwrap().starts_with("Couldn't read this theme's palette"));
        assert_eq!(details.wallpapers.len(), 1);
        assert!(details.can_apply());
    }

    #[test]
    fn palette_download_failure_is_reported() {
        let h = harness();
        h.http.on_body(TREE_URL, &tree_json(&[("colors.toml", "blob", Some(5))]), None);
        h.http.on(&format!("{RAW}colors.toml"), |_| Err(FakeTransport::offline()));

        let details = h.resolve().unwrap();

        assert!(details.palette.is_none());
        assert!(details.palette_error.unwrap().contains("colors.toml couldn't be downloaded"));
    }

    #[test]
    fn theme_with_nothing_usable_cannot_be_applied() {
        let h = harness();
        h.http.on_body(TREE_URL, &tree_json(&[("README.md", "blob", Some(5))]), None);
        assert!(!h.resolve().unwrap().can_apply());
    }

    #[test]
    fn light_mode_file_marks_theme_light() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[("colors.toml", "blob", Some(5)), ("light.mode", "blob", Some(0))]),
            None,
        );
        h.http.on_body(&format!("{RAW}colors.toml"), &fixture("colors-ansi.toml"), None); // dark background

        let details = h.resolve().unwrap();

        assert_eq!(details.mode, AppearanceMode::Light);
        assert_eq!(details.palette.unwrap().declared_mode, Some(AppearanceMode::Light));
    }

    #[test]
    fn uses_root_background_image_when_there_is_no_backgrounds_folder() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[
                ("background.jpg", "blob", Some(5)),
                ("preview.png", "blob", Some(5)),
                ("assets/wallpaper.png", "blob", Some(5)),
            ]),
            None,
        );

        let paths: Vec<_> = h.resolve().unwrap().wallpapers.into_iter().map(|w| w.path).collect();
        assert_eq!(paths, ["background.jpg"]);
    }

    #[test]
    fn uses_a_wallpapers_folder_when_there_is_no_backgrounds_folder() {
        let h = harness();
        h.http.on_body(
            TREE_URL,
            &tree_json(&[("wallpapers/b.png", "blob", Some(5)), ("background.jpg", "blob", Some(5))]),
            None,
        );

        let paths: Vec<_> = h.resolve().unwrap().wallpapers.into_iter().map(|w| w.path).collect();
        assert_eq!(paths, ["wallpapers/b.png"]);
    }

    #[test]
    fn resolves_themes_inside_a_repo_sub_folder() {
        let h = harness();
        let mut entry = entry();
        entry.repo_url = "https://github.com/o/mono/tree/main/themes/foo".into();
        h.http.on_body(
            "https://api.github.com/repos/o/mono/git/trees/main?recursive=1",
            &tree_json(&[
                ("themes/foo/colors.toml", "blob", Some(5)),
                ("themes/foo/backgrounds/1.png", "blob", Some(5)),
                ("themes/bar/backgrounds/1.png", "blob", Some(5)),
            ]),
            None,
        );
        h.http.on_body(
            "https://raw.githubusercontent.com/o/mono/main/themes/foo/colors.toml",
            &fixture("colors-ansi.toml"),
            None,
        );

        let details = h.resolver(None).resolve(&entry).unwrap();

        assert!(details.palette.is_some());
        let urls: Vec<_> = details.wallpapers.iter().map(|w| w.download_url.as_str()).collect();
        assert_eq!(urls, ["https://raw.githubusercontent.com/o/mono/main/themes/foo/backgrounds/1.png"]);
    }

    #[test]
    fn rate_limit_surfaces_reset_time() {
        let h = harness();
        h.http.on(TREE_URL, |_| {
            Ok(FakeTransport::status_with(403, &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1790482111")]))
        });

        let error = h.resolve().err().unwrap();
        match error.downcast_ref::<GitHubError>() {
            Some(GitHubError::RateLimited { resets_at }) => {
                assert_eq!(resets_at.unwrap().timestamp(), 1_790_482_111)
            }
            other => panic!("expected rateLimited, got {other:?}"),
        }
        assert!(error.to_string().starts_with("GitHub's rate limit was reached. It resets at "));
    }

    #[test]
    fn rate_limited_revisit_serves_cached_tree_as_stale() {
        let h = harness();
        h.http.on_body(TREE_URL, &tree_json(&[("backgrounds/a.png", "blob", Some(5))]), Some("\"t\""));
        h.resolve().unwrap();
        h.clock.advance(2 * 3600);
        h.http.on(TREE_URL, |_| Ok(FakeTransport::status(429)));

        let details = h.resolve().unwrap();

        assert!(details.is_stale);
        assert!(matches!(
            details.stale_reason.as_ref().unwrap().downcast_ref::<GitHubError>(),
            Some(GitHubError::RateLimited { .. })
        ));
        assert_eq!(details.wallpapers.len(), 1);
    }

    #[test]
    fn revisit_within_an_hour_makes_no_request() {
        let h = harness();
        h.http.on_body(TREE_URL, &tree_json(&[("backgrounds/a.png", "blob", Some(5))]), Some("\"t\""));
        h.resolve().unwrap();
        h.clock.advance(30 * 60);

        h.resolve().unwrap();

        assert_eq!(h.http.count(TREE_URL), 1);
    }

    #[test]
    fn missing_repo_is_reported_as_not_found() {
        let h = harness();
        h.http.on(TREE_URL, |_| Ok(FakeTransport::status(404)));

        match h.resolve().err().unwrap().downcast_ref::<GitHubError>() {
            Some(GitHubError::NotFound { repo }) => assert_eq!(repo, "o/r"),
            other => panic!("expected notFound, got {other:?}"),
        }
    }

    #[test]
    fn non_github_entries_are_rejected() {
        let h = harness();
        let entry = CatalogEntry {
            slug: "r".into(),
            name: "R".into(),
            repo_url: "https://gitlab.com/o/r".into(),
            screenshot_url: None,
        };
        assert!(h.resolver(None).resolve(&entry).err().unwrap().is::<ThemeResolveError>());
    }

    #[test]
    fn token_is_sent_to_the_api_only() {
        let h = harness();
        h.http.on_body(TREE_URL, &tree_json(&[("colors.toml", "blob", Some(5))]), None);
        h.http.on_body(&format!("{RAW}colors.toml"), &fixture("colors-ansi.toml"), None);

        h.resolver(Some(" secret ")).resolve(&entry()).unwrap();

        let requests = h.http.requests();
        let api = requests.iter().find(|r| r.url.starts_with("https://api.github.com/")).unwrap();
        let raw = requests.iter().find(|r| r.url.starts_with("https://raw.githubusercontent.com/")).unwrap();
        assert_eq!(api.header("Authorization"), Some("Bearer secret"));
        assert!(!api.header("User-Agent").unwrap_or_default().is_empty());
        assert_eq!(raw.header("Authorization"), None);
    }

    #[test]
    fn resolving_a_default_theme_reuses_the_cached_tree() {
        use crate::catalog::{CatalogService, DEFAULT_PAGE_URL};
        let h = harness();
        let tree = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1";
        let raw = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/";
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        h.http.on_body(tree, &fixture("omarchy-tree.json"), None);
        h.http.on_body(&format!("{raw}tokyo-night/colors.toml"), &fixture("colors-named.toml"), None);
        let github = GitHubClient::new(h.cache.clone(), None);
        let catalog = CatalogService::new(h.cache.clone(), Some(github.clone())).refresh().unwrap();
        let tokyo = catalog.entries.iter().find(|e| e.slug == "omarchy.tokyo-night").unwrap();

        let details = ThemeResolver::new(github).resolve(tokyo).unwrap();

        assert_eq!(h.http.count(tree), 1);
        assert_eq!(details.palette.unwrap().source, crate::palette::PaletteSource::ColorsToml);
        assert_eq!(details.mode, AppearanceMode::Light);
        // backgrounds/ only: not preview.png or unlock.png, naturally sorted.
        let names: Vec<_> = details.wallpapers.iter().map(|w| w.file_name()).collect();
        assert_eq!(names, ["0-winding-road.webp", "2-swirl-buck.webp", "10-oma.webp"]);
        assert_eq!(details.wallpapers[0].download_url, format!("{raw}tokyo-night/backgrounds/0-winding-road.webp"));
    }

    #[test]
    fn natural_sort_orders_numbers_by_value() {
        let mut input = vec!["b10.png", "b2.png", "a.png", "b1.png", "B3.png", "b02.png"];
        input.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(input, ["a.png", "b1.png", "b2.png", "b02.png", "B3.png", "b10.png"]);
    }
}
