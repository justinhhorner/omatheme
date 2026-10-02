//! Opt-in checks against the real omarchy.org, GitHub and this computer, all read-only:
//!
//!     OMATHEME_LIVE=1 cargo test -- --ignored --test-threads=1
//!
//! They resolve a handful of themes (the unauthenticated GitHub API allows 60 calls an hour; a
//! run makes about three) and never change the desktop.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use omatheme::cancel::CancelToken;
use omatheme::catalog::{CatalogService, ThemeCatalog};
use omatheme::github::GitHubClient;
use omatheme::net::{HttpCache, UreqTransport, system_clock};
use omatheme::platform::image_convert::{ConvertedLayout, ImageConverter};
use omatheme::resolver::ThemeResolver;
use omatheme::store::{Downloader, UreqDownloader};

fn enabled() -> bool {
    let on = std::env::var("OMATHEME_LIVE").is_ok_and(|v| v == "1");
    if !on {
        eprintln!("skipped: set OMATHEME_LIVE=1 to run live checks");
    }
    on
}

fn services(dir: &std::path::Path) -> (Arc<HttpCache>, GitHubClient) {
    let cache = Arc::new(HttpCache::new(
        Arc::new(UreqTransport::new(Duration::from_secs(60))),
        dir.join("cache"),
        system_clock(),
    ));
    let github = GitHubClient::new(cache.clone(), std::env::var("GITHUB_TOKEN").ok().as_deref());
    (cache, github)
}

fn catalog(cache: &Arc<HttpCache>, github: &GitHubClient) -> ThemeCatalog {
    CatalogService::new(cache.clone(), Some(github.clone())).refresh().expect("catalog")
}

#[test]
#[ignore = "live: set OMATHEME_LIVE=1"]
fn catalog_default_themes_and_a_few_themes_resolve() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (cache, github) = services(dir.path());

    let catalog = catalog(&cache, &github);
    let defaults: Vec<_> = catalog.entries.iter().filter(|e| e.is_default_theme()).collect();
    let community = catalog.entries.len() - defaults.len();
    println!("Catalog: {} themes ({} default, {community} community)", catalog.entries.len(), defaults.len());
    assert!(community > 100, "{community} community themes");
    assert!(defaults.len() >= 10, "{} default themes", defaults.len());
    assert!(catalog.default_themes_error.is_none(), "{:?}", catalog.default_themes_error.map(|e| e.to_string()));
    assert!(defaults.iter().all(|e| e.screenshot_url.is_some()));
    let slugs: std::collections::HashSet<_> = catalog.entries.iter().map(|e| &e.slug).collect();
    assert_eq!(slugs.len(), catalog.entries.len());

    let resolver = ThemeResolver::new(github);
    // Tokyo Night comes from the tree the catalog already fetched; Vulkanite ships WebP wallpapers.
    for slug in ["omarchy.tokyo-night", "vulkanite"] {
        let Some(entry) = catalog.entries.iter().find(|e| e.slug == slug) else {
            println!("{slug}: not in the catalog");
            continue;
        };
        let details = resolver.resolve(entry).unwrap_or_else(|e| panic!("{slug}: {e:#}"));
        let palette = details
            .palette
            .as_ref()
            .map(|p| {
                format!(
                    "{} bg {} accent {}, {} swatches",
                    p.source.file_name(),
                    p.background,
                    p.accent,
                    p.swatches.len()
                )
            })
            .unwrap_or_else(|| format!("no palette: {}", details.palette_error.clone().unwrap_or_default()));
        println!("{slug}: {} wallpapers, {}, {palette}", details.wallpapers.len(), details.mode);
        assert!(details.can_apply());
        assert!(details.palette.is_some());
        assert!(!details.wallpapers.is_empty());
    }
}

#[test]
#[ignore = "live: set OMATHEME_LIVE=1"]
fn a_download_reports_byte_progress_and_webp_converts_to_png() {
    if !enabled() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let (cache, github) = services(dir.path());
    let catalog = CatalogService::new(cache.clone(), None).refresh().expect("catalog");
    let entry = catalog.entries.iter().find(|e| e.slug == "vulkanite").unwrap_or(&catalog.entries[0]);
    let details = ThemeResolver::new(github).resolve(entry).expect("resolve");
    let wallpaper = details.wallpapers.first().expect("a wallpaper");

    let destination = dir.path().join(wallpaper.file_name());
    let mut reports = Vec::new();
    UreqDownloader::default()
        .download(&wallpaper.download_url, &destination, &mut |bytes| reports.push(bytes), &CancelToken::new())
        .expect("download");

    let size = std::fs::metadata(&destination).unwrap().len();
    println!("Downloaded {}: {size} bytes, {} progress reports", wallpaper.file_name(), reports.len());
    assert_eq!(Some(size), wallpaper.size);
    assert!(reports.len() > 1);
    assert_eq!(reports.last(), Some(&size));
    assert!(reports.windows(2).all(|w| w[0] <= w[1]));

    for layout in [ConvertedLayout::Mac, ConvertedLayout::Windows] {
        let usable = ImageConverter::new(layout).ensure_supported_format(&destination).expect("convert");
        let image = image::open(&usable).expect("decodable");
        println!("Usable as wallpaper ({layout:?}): {} {}x{}", usable.display(), image.width(), image.height());
    }
}

/// Reads (never sets) the current wallpaper through the built binary, which runs the OS calls on
/// the main thread as AppKit wants. The test switches keep everything else away from real data.
#[test]
#[ignore = "live: set OMATHEME_LIVE=1"]
fn the_current_wallpaper_can_be_read() {
    if !enabled() {
        return;
    }
    if !cfg!(any(target_os = "macos", windows)) {
        println!("skipped: applying isn't supported on this OS");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_omatheme"))
        .args(["current", "--json"])
        .env("OMARCHY_THEMES_DRY_RUN", "1")
        .env("OMARCHY_THEMES_DATA_DIR", dir.path())
        .output()
        .expect("run omatheme");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let displays = json["displays"].as_array().unwrap();
    for display in displays {
        println!("{}: {}", display["display"], display["picture"]);
    }
    assert!(!displays.is_empty(), "no displays reported");
}
