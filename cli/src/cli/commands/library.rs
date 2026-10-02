//! `download`, `remove`, `cache clear` and `paths`.

use anyhow::Result;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use serde_json::json;

use super::super::Outcome;
use super::super::args::{DownloadArgs, RemoveArgs};
use super::super::context::Context;
use super::super::matching;
use super::super::output::{format_bytes, print_json};
use crate::catalog::CatalogEntry;
use crate::paths::AppPaths;
use crate::store::{DownloadProgress, InstalledTheme};
use crate::theming::{FileSnapshotStore, SnapshotStore};
use crate::{cancel, platform};

/// Resolves and downloads one theme, with a progress bar (bytes, per file) on a terminal.
pub fn download_theme(ctx: &Context, entry: &CatalogEntry) -> Result<InstalledTheme> {
    let details = ctx.resolve(entry)?;
    if let Some(reason) = &details.stale_reason {
        ctx.out.warn(&format!("Using what was saved from GitHub earlier. {reason}"));
    }

    let sizes: Vec<Option<u64>> = details.wallpapers.iter().map(|w| w.size).collect();
    let total: Option<u64> = sizes.iter().copied().sum();
    let bar = if ctx.out.stderr_tty {
        let bar = match total {
            Some(total) => ProgressBar::with_draw_target(Some(total), ProgressDrawTarget::stderr()).with_style(
                ProgressStyle::with_template("{msg} [{bar:28}] {decimal_bytes}/{decimal_total_bytes}")
                    .expect("valid template")
                    .progress_chars("=> "),
            ),
            None => ProgressBar::with_draw_target(None, ProgressDrawTarget::stderr())
                .with_style(ProgressStyle::with_template("{msg} {decimal_bytes}").expect("valid template")),
        };
        bar.set_message(entry.name.clone());
        bar
    } else {
        ProgressBar::hidden()
    };

    let files = details.wallpapers.len();
    let mut report = |p: DownloadProgress| {
        let done_before: u64 = sizes.iter().take(p.file_index).map(|s| s.unwrap_or(0)).sum();
        if p.file_index < files {
            bar.set_message(format!("{} {}/{files}", entry.name, p.file_index + 1));
            bar.set_position(done_before + p.bytes_received);
        } else {
            bar.set_message(format!("{} (screenshot)", entry.name));
        }
    };
    let result = ctx.store().install(&details, &mut report, &ctx.cancel);
    bar.finish_and_clear();
    result
}

pub fn download(ctx: &Context, args: &DownloadArgs) -> Result<Outcome> {
    let index = ctx.theme_index(false)?;
    ctx.show_notices(&index.notices);
    ctx.clean_up_store();

    let mut failed = false;
    for query in &args.themes {
        let entry = match index.find(query) {
            Ok(entry) => entry.clone(),
            Err(error) => {
                ctx.out.error(&error.to_string());
                failed = true;
                continue;
            }
        };
        if !args.force && index.installed(&entry.slug).is_some() {
            println!("{} is already downloaded. Use --force to download it again.", entry.name);
            continue;
        }
        match download_theme(ctx, &entry) {
            Ok(theme) => {
                let size: u64 = theme
                    .wallpapers
                    .iter()
                    .filter_map(|w| std::fs::metadata(theme.wallpaper_path(w)).ok())
                    .map(|m| m.len())
                    .sum();
                let wallpapers = match theme.wallpapers.len() {
                    1 => "1 wallpaper".to_string(),
                    n => format!("{n} wallpapers"),
                };
                println!(
                    "{} {}: {wallpapers} ({}) in {}",
                    ctx.out.green("Downloaded"),
                    theme.name,
                    format_bytes(size),
                    theme.directory.display()
                );
            }
            Err(error) if cancel::is_cancelled(&error) => return Err(error),
            Err(error) => {
                ctx.out.error(&format!("Couldn't download {}: {error:#}", entry.name));
                failed = true;
            }
        }
    }
    Ok(if failed { Outcome::Failure } else { Outcome::Success })
}

pub fn remove(ctx: &Context, args: &RemoveArgs) -> Result<Outcome> {
    let store = ctx.store();
    let installed = store.list();
    let theme = match matching::find(&installed, &args.theme) {
        Ok(theme) => theme,
        Err(matching::MatchError::NotFound(_)) => {
            anyhow::bail!("No downloaded theme matches '{}'. `omatheme list --downloaded` shows them.", args.theme)
        }
        Err(error) => return Err(error.into()),
    };

    let settings = ctx.settings().load();
    if settings.last_applied_slug.as_deref() == Some(theme.slug.as_str()) {
        ctx.out.warn(&format!(
            "{} is the theme on your desktop. Removing it deletes its wallpapers, including the one your desktop shows.",
            theme.name
        ));
        if !ctx.out.confirm(&format!("Remove {}?", theme.name), args.yes)? {
            println!("Nothing was removed.");
            return Ok(Outcome::Success);
        }
    }
    store.remove(&theme.slug)?;
    println!("Removed {}.", theme.name);
    Ok(Outcome::Success)
}

pub fn clear_cache(ctx: &Context) -> Result<Outcome> {
    let before = ctx.cache().size();
    ctx.cache().clear()?;
    println!("Cleared the cache ({} freed). Downloaded themes weren't touched.", format_bytes(before));
    Ok(Outcome::Success)
}

/// What `original-desktop.json` holds, for `paths`.
fn snapshot_status(paths: &AppPaths) -> (String, serde_json::Value) {
    match FileSnapshotStore::new(paths.clone()).load() {
        Ok(Some(snapshot)) => {
            let when = snapshot.taken_at.with_timezone(&chrono::Local).format("%b %-d, %Y %-I:%M %p").to_string();
            (format!("saved {when}"), json!({ "saved": true, "takenAt": crate::json::format_date(snapshot.taken_at) }))
        }
        Ok(None) => ("none saved".into(), json!({ "saved": false })),
        Err(error) => (format!("unreadable: {error}"), json!({ "saved": true, "error": error.to_string() })),
    }
}

pub fn paths(ctx: &Context, as_json: bool) -> Result<Outcome> {
    let paths = &ctx.paths;
    let themes = ctx.store().list().len();
    let cache_size = ctx.cache().size();
    let (snapshot_text, snapshot_json) = snapshot_status(paths);
    let capabilities = platform::dry_run_backend().map(|b| crate::theming::DesktopBackend::capabilities(&b));

    if as_json {
        print_json(&json!({
            "dataDir": paths.root,
            "settings": paths.settings_file(),
            "themesDir": paths.themes_dir(),
            "downloadedThemes": themes,
            "cacheDir": paths.cache_dir(),
            "cacheBytes": cache_size,
            "originalDesktop": paths.snapshot_file(),
            "originalDesktopCopies": paths.original_desktop_dir(),
            "snapshot": snapshot_json,
            "dryRun": ctx.dry_run,
            "customDataDir": ctx.custom_data_dir,
            "gitHubToken": ctx.github().has_token(),
            "platform": platform::os_name(),
            "canApply": capabilities.is_some(),
            "capabilities": capabilities.map(|c| json!({
                "wallpaper": c.wallpaper, "appearanceMode": c.appearance_mode, "accentColor": c.accent_color
            })),
        }))?;
        return Ok(Outcome::Success);
    }

    let out = &ctx.out;
    let row = |label: &str, value: String| println!("{}  {value}", out.bold(&format!("{label:<17}")));
    let mut data_dir = paths.root.display().to_string();
    if ctx.custom_data_dir {
        data_dir += " (test data folder)";
    }
    row("Data folder", data_dir);
    row("Settings", paths.settings_file().display().to_string());
    row("Themes", format!("{} ({themes} downloaded)", paths.themes_dir().display()));
    row("Cache", format!("{} ({})", paths.cache_dir().display(), format_bytes(cache_size)));
    row("Original desktop", format!("{} ({snapshot_text})", paths.snapshot_file().display()));
    let applies = match capabilities {
        Some(c) => {
            let mut parts = vec!["wallpaper"];
            if c.appearance_mode {
                parts.push("light/dark");
            }
            if c.accent_color {
                parts.push("accent color");
            }
            parts.join(", ")
        }
        None => "nothing (not supported here)".into(),
    };
    row("Applies", format!("{applies} on {}", platform::os_name()));
    row(
        "GitHub token",
        if ctx.github().has_token() { "set (GITHUB_TOKEN)".into() } else { "not set (60 API requests an hour)".into() },
    );
    if ctx.dry_run {
        row("Dry run", "on: the desktop and terminals aren't changed".into());
    }
    Ok(Outcome::Success)
}
