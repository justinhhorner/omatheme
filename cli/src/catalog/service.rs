use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, TimeDelta, Utc};

use super::{CatalogEntry, DEFAULT_PAGE_URL, default_themes, parser};
use crate::cancel::is_cancelled;
use crate::github::GitHubClient;
use crate::net::{CacheOptions, HttpCache};

#[derive(Clone)]
pub struct ThemeCatalog {
    /// Omarchy's default themes first, then the community gallery.
    pub entries: Vec<CatalogEntry>,
    pub fetched_at: DateTime<Utc>,
    /// True when omarchy.org couldn't be reached and a cached copy is shown.
    pub is_stale: bool,
    pub stale_reason: Option<Arc<anyhow::Error>>,
    /// Why the default themes are missing or stale, if they are (the community gallery still loads).
    pub default_themes_error: Option<Arc<anyhow::Error>>,
}

impl ThemeCatalog {
    pub fn has_default_themes(&self) -> bool {
        self.entries.iter().any(CatalogEntry::is_default_theme)
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "No themes were found on omarchy.org/themes. The page layout may have changed; check for an update of omatheme."
)]
pub struct CatalogFormatError;

/// Loads the catalog: the community gallery from omarchy.org/themes plus, when a GitHub client is
/// given, the themes that ship with Omarchy. The raw page and repo tree are cached (not the parsed
/// result), so a parser fix applies to the cached copy too and the tool works offline.
pub struct CatalogService {
    cache: Arc<HttpCache>,
    page_url: String,
    github: Option<GitHubClient>,
}

impl CatalogService {
    /// The apps refresh a cached catalog in the background once it's this old.
    pub const REFRESH_AFTER_HOURS: i64 = 12;

    pub fn new(cache: Arc<HttpCache>, github: Option<GitHubClient>) -> Self {
        CatalogService { cache, page_url: DEFAULT_PAGE_URL.to_string(), github }
    }

    /// The last downloaded catalog, without any network access; None on first run.
    pub fn load_cached(&self) -> Option<ThemeCatalog> {
        let cached = self.cache.cached_response(&self.page_url)?;
        let entries = parser::parse(&cached.text(), &self.page_url);
        if entries.is_empty() {
            return None;
        }
        let defaults = self
            .github
            .as_ref()
            .and_then(|g| g.cached_tree(&default_themes::repo()))
            .map(|tree| default_themes::entries(&tree))
            .unwrap_or_default();
        Some(ThemeCatalog {
            entries: defaults.into_iter().chain(entries).collect(),
            fetched_at: cached.fetched_at,
            is_stale: false,
            stale_reason: None,
            default_themes_error: None,
        })
    }

    /// True when a cached catalog is recent and complete enough to use without a request, as the
    /// apps decide on launch.
    pub fn is_fresh(&self, catalog: &ThemeCatalog) -> bool {
        catalog.has_default_themes()
            && self.cache.now() - catalog.fetched_at < TimeDelta::hours(Self::REFRESH_AFTER_HOURS)
    }

    /// Re-fetches omarchy.org/themes (conditional GET), falling back to the cached copy if offline.
    pub fn refresh(&self) -> Result<ThemeCatalog> {
        let response =
            self.cache.get(&self.page_url, &CacheOptions { force_revalidate: true, ..Default::default() })?;
        let entries = parser::parse(&response.text(), &self.page_url);
        if entries.is_empty() {
            return Err(CatalogFormatError.into());
        }

        let (defaults, defaults_error) = self.load_default_themes()?;
        Ok(ThemeCatalog {
            entries: defaults.into_iter().chain(entries).collect(),
            fetched_at: response.fetched_at,
            is_stale: response.is_stale,
            stale_reason: response.error,
            default_themes_error: defaults_error,
        })
    }

    /// The default themes, from Omarchy's repo tree (re-checked at most hourly, then with a free ETag
    /// revalidation). A GitHub failure with nothing cached drops them rather than the catalog.
    fn load_default_themes(&self) -> Result<(Vec<CatalogEntry>, Option<Arc<anyhow::Error>>)> {
        let Some(github) = &self.github else {
            return Ok((Vec::new(), None));
        };
        match github.tree(&default_themes::repo()) {
            Ok(tree) => Ok((default_themes::entries(&tree), tree.stale_reason)),
            Err(error) if is_cancelled(&error) => Err(error),
            Err(error) => Ok((Vec::new(), Some(Arc::new(error)))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::GitHubError;
    use crate::net::NetError;
    use crate::test_support::{FakeTransport, ManualClock, fixture};

    const TREE_URL: &str = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1";
    const RAW: &str = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/";

    struct Harness {
        _dir: tempfile::TempDir,
        http: Arc<FakeTransport>,
        clock: ManualClock,
        cache: Arc<HttpCache>,
        github: GitHubClient,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let http = Arc::new(FakeTransport::default());
        let clock = ManualClock::at(1_790_467_200); // 2026-09-27 UTC
        let cache = Arc::new(HttpCache::new(http.clone(), dir.path(), clock.function()));
        let github = GitHubClient::new(cache.clone(), None);
        Harness { _dir: dir, http, clock, cache, github }
    }

    impl Harness {
        fn catalog(&self) -> CatalogService {
            CatalogService::new(self.cache.clone(), Some(self.github.clone()))
        }
    }

    #[test]
    fn lists_each_folder_under_themes_with_omarchys_naming() {
        let h = harness();
        h.http.on_body(TREE_URL, &fixture("omarchy-tree.json"), None);

        let entries = default_themes::entries(&h.github.tree(&default_themes::repo()).unwrap());

        let slugs: Vec<_> = entries.iter().map(|e| e.slug.as_str()).collect();
        assert_eq!(slugs, ["omarchy.catppuccin-latte", "omarchy.retro-82", "omarchy.tokyo-night"]);
        let names: Vec<_> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["Catppuccin Latte", "Retro 82", "Tokyo Night"]);

        let tokyo = &entries[2];
        assert_eq!(tokyo.repo_url, "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night");
        assert_eq!(tokyo.screenshot_url.as_deref(), Some(format!("{RAW}tokyo-night/preview.png").as_str()));
        assert!(tokyo.is_default_theme());
        assert_eq!(tokyo.repo_display(), "Included with Omarchy");
        // No preview.png in the folder: no screenshot rather than a broken link.
        assert_eq!(entries[1].screenshot_url, None);
    }

    #[test]
    fn loads_cache_offline_and_rejects_pages_without_themes() {
        let h = harness();
        let catalog = CatalogService::new(h.cache.clone(), None);
        assert!(catalog.load_cached().is_none());

        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        let fresh = catalog.refresh().unwrap();
        assert_eq!(fresh.entries.len(), 4);
        assert!(!fresh.is_stale);
        assert_eq!(catalog.load_cached().unwrap().entries.len(), 4);

        h.http.on(DEFAULT_PAGE_URL, |_| Err(FakeTransport::offline()));
        let offline = catalog.refresh().unwrap();
        assert!(offline.is_stale);
        assert_eq!(offline.entries.len(), 4);

        h.http.on_body(DEFAULT_PAGE_URL, "<html><body>Under maintenance</body></html>", None);
        assert!(catalog.refresh().err().unwrap().is::<CatalogFormatError>());
    }

    #[test]
    fn catalog_lists_default_themes_first_then_the_community_gallery() {
        let h = harness();
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        h.http.on_body(TREE_URL, &fixture("omarchy-tree.json"), None);

        let catalog = h.catalog().refresh().unwrap();

        assert_eq!(catalog.entries.len(), 7);
        assert!(catalog.entries[..3].iter().all(CatalogEntry::is_default_theme));
        assert_eq!(catalog.entries[3].slug, "aetheria");
        assert!(catalog.default_themes_error.is_none());
        let unique: HashSet<_> = catalog.entries.iter().map(|e| &e.slug).collect();
        assert_eq!(unique.len(), catalog.entries.len());
    }

    use std::collections::HashSet;

    #[test]
    fn catalog_still_loads_when_github_fails_and_nothing_is_cached() {
        let h = harness();
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        h.http.on(TREE_URL, |_| Ok(FakeTransport::status_with(403, &[("x-ratelimit-remaining", "0")])));

        let catalog = h.catalog().refresh().unwrap();

        assert_eq!(catalog.entries.len(), 4);
        let error = catalog.default_themes_error.unwrap();
        assert!(matches!(error.downcast_ref::<GitHubError>(), Some(GitHubError::RateLimited { .. })));
        assert!(!catalog.is_stale);
    }

    #[test]
    fn default_themes_load_from_cache_offline() {
        let h = harness();
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        h.http.on_body(TREE_URL, &fixture("omarchy-tree.json"), Some("\"tree\""));
        h.catalog().refresh().unwrap();

        assert_eq!(h.catalog().load_cached().unwrap().entries.len(), 7);

        h.clock.advance(2 * 3600);
        h.http.on(DEFAULT_PAGE_URL, |_| Err(FakeTransport::offline()));
        h.http.on(TREE_URL, |_| Err(FakeTransport::offline()));
        let offline = h.catalog().refresh().unwrap();

        assert!(offline.is_stale);
        assert_eq!(offline.entries.len(), 7);
        assert!(offline.default_themes_error.unwrap().is::<NetError>());
    }

    #[test]
    fn catalog_without_a_github_client_is_community_only() {
        let h = harness();
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);

        let catalog = CatalogService::new(h.cache.clone(), None).refresh().unwrap();

        assert_eq!(catalog.entries.len(), 4);
        assert_eq!(h.http.count(TREE_URL), 0);
    }

    #[test]
    fn a_cached_catalog_is_fresh_for_twelve_hours_if_it_has_default_themes() {
        let h = harness();
        h.http.on_body(DEFAULT_PAGE_URL, &fixture("catalog-live-structure.html"), None);
        h.http.on_body(TREE_URL, &fixture("omarchy-tree.json"), None);
        h.catalog().refresh().unwrap();

        let cached = h.catalog().load_cached().unwrap();
        assert!(h.catalog().is_fresh(&cached));
        h.clock.advance(13 * 3600);
        assert!(!h.catalog().is_fresh(&cached));

        let community_only = CatalogService::new(h.cache.clone(), None).load_cached().unwrap();
        assert!(!h.catalog().is_fresh(&community_only));
    }
}
