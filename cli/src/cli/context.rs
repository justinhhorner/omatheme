//! Everything a command needs, built from the arguments and environment: the data folder, the test
//! switches, the network services, the theme store and the desktop backend.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, bail};

use super::matching::{self, Named};
use super::output::Output;
use crate::cancel::CancelToken;
use crate::catalog::{CatalogEntry, CatalogFormatError, CatalogService, ThemeCatalog};
use crate::github::GitHubClient;
use crate::net::{HttpCache, NetError, UreqTransport, system_clock};
use crate::palette::{Palette, PaletteSource};
use crate::paths::{AppPaths, DATA_DIR_VAR};
use crate::platform;
use crate::resolver::{ThemeDetails, ThemeResolver};
use crate::store::{InstalledTheme, SettingsStore, ThemeStore, UreqDownloader};
use crate::terminals::{self, TerminalEnvironment, TerminalExporter};
use crate::theming::{
    DesktopBackend, DryRunBackend, FileSnapshotStore, OverlaySnapshotStore, SnapshotStore, ThemeApplier,
};

pub const DRY_RUN_VAR: &str = "OMARCHY_THEMES_DRY_RUN";

/// Interrupted downloads older than this are tidied up; younger ones may be the app's, still running.
const STALE_WORK_AGE: Duration = Duration::from_secs(6 * 3600);

pub struct Context {
    pub paths: AppPaths,
    pub dry_run: bool,
    /// Another data folder was given (`--data-dir` or OMARCHY_THEMES_DATA_DIR).
    pub custom_data_dir: bool,
    pub out: Output,
    pub cancel: CancelToken,
    cache: Arc<HttpCache>,
    github: GitHubClient,
    settings: SettingsStore,
    dry_run_backend: Option<Arc<DryRunBackend>>,
}

impl Context {
    pub fn new(dry_run_flag: bool, data_dir_flag: Option<PathBuf>, out: Output, cancel: CancelToken) -> Self {
        let env_dir = std::env::var_os(DATA_DIR_VAR).filter(|v| !v.is_empty()).map(PathBuf::from);
        let custom_dir = data_dir_flag.or(env_dir);
        let custom_data_dir = custom_dir.is_some();
        let paths = AppPaths::new(custom_dir.unwrap_or_else(AppPaths::default_root));
        let dry_run = dry_run_flag || std::env::var(DRY_RUN_VAR).is_ok_and(|v| v == "1");

        let cache = Arc::new(HttpCache::new(
            Arc::new(UreqTransport::new(Duration::from_secs(60))),
            paths.cache_dir(),
            system_clock(),
        ));
        let token = std::env::var("GITHUB_TOKEN").ok();
        let github = GitHubClient::new(cache.clone(), token.as_deref());

        // A dry run against the real data folder reads the user's settings but never writes them;
        // with a test data folder it behaves like the apps' dry run, so restore can be exercised.
        let persist = !dry_run || custom_data_dir;
        let settings =
            if persist { SettingsStore::new(paths.clone()) } else { SettingsStore::in_memory(paths.clone()) };
        let dry_run_backend = dry_run.then(platform::dry_run_backend).flatten().map(Arc::new);

        Context { paths, dry_run, custom_data_dir, out, cancel, cache, github, settings, dry_run_backend }
    }

    /// Whether changes to settings.json and the saved desktop are written (not in a dry run against
    /// the real data folder).
    pub fn persists(&self) -> bool {
        !self.dry_run || self.custom_data_dir
    }

    pub fn cache(&self) -> &Arc<HttpCache> {
        &self.cache
    }

    pub fn github(&self) -> &GitHubClient {
        &self.github
    }

    pub fn settings(&self) -> &SettingsStore {
        &self.settings
    }

    pub fn catalog_service(&self) -> CatalogService {
        CatalogService::new(self.cache.clone(), Some(self.github.clone()))
    }

    pub fn resolver(&self) -> ThemeResolver {
        ThemeResolver::new(self.github.clone())
    }

    pub fn store(&self) -> ThemeStore {
        ThemeStore::new(self.paths.clone(), Arc::new(UreqDownloader::default()), system_clock())
    }

    /// Tidies up after downloads that were interrupted long ago (by this tool or the app).
    pub fn clean_up_store(&self) {
        self.store().clean_up(STALE_WORK_AGE);
    }

    /// What a dry run would have changed so far.
    pub fn dry_run_actions(&self) -> Vec<String> {
        self.dry_run_backend.as_ref().map(|b| b.actions()).unwrap_or_default()
    }

    /// The desktop, or an error where applying isn't supported.
    pub fn applier(&self) -> Result<ThemeApplier> {
        let backend: Box<dyn DesktopBackend> = match &self.dry_run_backend {
            Some(dry) => Box::new(dry.clone()),
            None => match platform::live_backend(&self.paths) {
                Some(backend) => backend,
                None => bail!(UnsupportedPlatform),
            },
        };
        let file = FileSnapshotStore::new(self.paths.clone());
        let snapshots: Box<dyn SnapshotStore> =
            if self.persists() { Box::new(file) } else { Box::new(OverlaySnapshotStore::new(file)) };
        Ok(ThemeApplier::new(backend, snapshots))
    }

    pub fn terminal_exporters(&self) -> Vec<Box<dyn TerminalExporter>> {
        terminals::all(Arc::new(TerminalEnvironment::live(&self.paths, self.dry_run, self.custom_data_dir)))
    }

    /// Loads the catalog and downloaded themes; it can run on another thread (the TUI does).
    pub fn loader(&self) -> CatalogLoader {
        CatalogLoader { service: self.catalog_service(), store: self.store() }
    }

    /// See [`CatalogLoader::theme_index`].
    pub fn theme_index(&self, force_refresh: bool) -> Result<ThemeIndex> {
        self.loader().theme_index(force_refresh)
    }

    /// Prints catalog notices on stderr.
    pub fn show_notices(&self, notices: &[String]) {
        for notice in notices {
            self.out.warn(notice);
        }
    }

    /// Resolves a theme on GitHub, or fails with the reason the apps give.
    pub fn resolve(&self, entry: &CatalogEntry) -> Result<ThemeDetails> {
        self.resolver().resolve(entry)
    }

    /// The palette to send to a terminal. Themes downloaded before `muted` and `bright_foreground`
    /// were read saved a palette without them, so the theme is looked up again (cached, usually
    /// free) for Omarchy's exact bright black, bright white and cursor. Offline, what's saved is used.
    pub fn palette_for_terminal(&self, entry: &CatalogEntry, saved: Option<&Palette>) -> Result<Palette> {
        if let Some(saved) = saved {
            if !lacks_named_extras(saved) {
                return Ok(saved.clone());
            }
            return Ok(self.resolve(entry).ok().and_then(|d| d.palette).unwrap_or_else(|| saved.clone()));
        }
        let details = self.resolve(entry)?;
        match details.palette {
            Some(palette) => Ok(palette),
            None => bail!(details.palette_error.unwrap_or_else(|| "This theme has no palette.".into())),
        }
    }
}

/// The catalog and the downloaded themes, without anything tied to the terminal, so it can be sent
/// to a background thread.
pub struct CatalogLoader {
    service: CatalogService,
    store: ThemeStore,
}

impl CatalogLoader {
    /// The catalog: the cached one if it's recent enough (as the apps decide on launch), else a
    /// refresh, falling back to the cached copy. Notices say what's stale or missing.
    pub fn load_catalog(&self, force_refresh: bool) -> Result<CatalogLoad> {
        let service = &self.service;
        let cached = service.load_cached();
        if !force_refresh && let Some(cached) = cached.as_ref().filter(|c| service.is_fresh(c)) {
            return Ok(CatalogLoad { catalog: Some(cached.clone()), notices: Vec::new() });
        }

        match service.refresh() {
            Ok(catalog) => {
                let mut notices = Vec::new();
                if catalog.is_stale {
                    notices.push(format!(
                        "Couldn't reach omarchy.org, so this is the catalog from {}.",
                        catalog.fetched_at.with_timezone(&chrono::Local).format("%b %-d, %Y %-I:%M %p")
                    ));
                }
                if let Some(error) = &catalog.default_themes_error
                    && !catalog.has_default_themes()
                {
                    notices.push(format!("The themes that come with Omarchy couldn't be loaded from GitHub. {error}"));
                }
                Ok(CatalogLoad { catalog: Some(catalog), notices })
            }
            Err(error) if crate::cancel::is_cancelled(&error) => Err(error),
            Err(error) => {
                let message = if error.is::<CatalogFormatError>() {
                    error.to_string()
                } else {
                    "Couldn't load themes from omarchy.org. Check your internet connection and try again.".to_string()
                };
                match cached {
                    Some(cached) => Ok(CatalogLoad { catalog: Some(cached), notices: vec![message] }),
                    None if !self.store.list().is_empty() => Ok(CatalogLoad {
                        catalog: None,
                        notices: vec![format!("{message} Only downloaded themes are listed.")],
                    }),
                    None if error.is::<NetError>() => bail!(message),
                    None => Err(error),
                }
            }
        }
    }

    /// Every theme an argument can name: the catalog plus downloaded themes it doesn't list.
    pub fn theme_index(&self, force_refresh: bool) -> Result<ThemeIndex> {
        let load = self.load_catalog(force_refresh)?;
        let installed = self.store.list();
        let mut entries = load.catalog.as_ref().map(|c| c.entries.clone()).unwrap_or_default();
        for theme in &installed {
            if !entries.iter().any(|e| e.slug == theme.slug) {
                entries.push(entry_for(theme));
            }
        }
        Ok(ThemeIndex { entries, installed, catalog: load.catalog, notices: load.notices })
    }
}

#[derive(Debug, thiserror::Error)]
#[error(
    "Applying themes isn't supported on {}. omatheme can list, show and download themes and send their colors to a terminal here; {}",
    platform::os_name(),
    unsupported_hint()
)]
pub struct UnsupportedPlatform;

fn unsupported_hint() -> &'static str {
    if cfg!(target_os = "linux") {
        "on Omarchy itself, use Omarchy's own theme switcher."
    } else {
        "applying works on macOS and Windows."
    }
}

pub struct CatalogLoad {
    pub catalog: Option<ThemeCatalog>,
    pub notices: Vec<String>,
}

pub struct ThemeIndex {
    /// Catalog order (default themes first), then downloaded themes the catalog doesn't list.
    pub entries: Vec<CatalogEntry>,
    pub installed: Vec<InstalledTheme>,
    pub catalog: Option<ThemeCatalog>,
    pub notices: Vec<String>,
}

impl ThemeIndex {
    pub fn find(&self, query: &str) -> Result<&CatalogEntry> {
        Ok(matching::find(&self.entries, query)?)
    }

    pub fn installed(&self, slug: &str) -> Option<&InstalledTheme> {
        self.installed.iter().find(|t| t.slug() == slug)
    }
}

/// A catalog entry for a downloaded theme the catalog doesn't (or can't) list.
pub fn entry_for(theme: &InstalledTheme) -> CatalogEntry {
    CatalogEntry {
        slug: theme.slug.clone(),
        name: theme.name.clone(),
        repo_url: theme.repo_url.clone(),
        screenshot_url: None,
    }
}

/// Themes downloaded before `muted` and `bright_foreground` were read saved a colors.toml palette
/// without them; looking the theme up again gives Omarchy's exact terminal colors.
pub fn lacks_named_extras(palette: &Palette) -> bool {
    palette.source == PaletteSource::ColorsToml
        && palette.muted.is_none()
        && palette.bright_foreground.is_none()
        && !palette.swatches.iter().any(|s| s.name == "Bright black")
}
