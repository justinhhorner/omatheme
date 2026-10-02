//! `list` and `show`.

use std::collections::HashSet;

use anyhow::Result;
use serde::Serialize;
use serde_json::json;

use super::super::Outcome;
use super::super::args::{ListArgs, ShowArgs};
use super::super::context::Context;
use super::super::output::{Output, format_bytes, pad, print_json};
use crate::catalog::CatalogEntry;
use crate::palette::{AppearanceMode, Palette, TerminalColors};
use crate::resolver::ThemeDetails;
use crate::store::InstalledTheme;
use crate::terminals::scheme_name;

/// The apps' search: by name or repo (and, here, slug), case-insensitive, plus the filters.
/// The apps' search, plus the slug: `query` (lowercased) in the name, repo or slug.
pub fn matches_search(entry: &CatalogEntry, query: &str) -> bool {
    entry.name.to_lowercase().contains(query)
        || entry.repo_display().to_lowercase().contains(query)
        || entry.slug.contains(query)
}

pub fn filter<'a>(
    entries: &'a [CatalogEntry],
    downloaded: &HashSet<&str>,
    search: Option<&str>,
    downloaded_only: bool,
    default_only: bool,
    community_only: bool,
) -> Vec<&'a CatalogEntry> {
    let query = search.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty());
    entries
        .iter()
        .filter(|e| query.as_ref().is_none_or(|q| matches_search(e, q)))
        .filter(|e| !downloaded_only || downloaded.contains(e.slug.as_str()))
        .filter(|e| !default_only || e.is_default_theme())
        .filter(|e| !community_only || !e.is_default_theme())
        .collect()
}

pub fn list(ctx: &Context, args: &ListArgs) -> Result<Outcome> {
    let index = ctx.theme_index(args.refresh)?;
    let downloaded: HashSet<&str> = index.installed.iter().map(|t| t.slug.as_str()).collect();
    let current = ctx.settings().load().last_applied_slug;
    let shown =
        filter(&index.entries, &downloaded, args.search.as_deref(), args.downloaded, args.default_only, args.community);

    if args.json {
        let themes: Vec<_> = shown
            .iter()
            .map(|e| {
                json!({
                    "slug": e.slug,
                    "name": e.name,
                    "repoUrl": e.repo_url,
                    "screenshotUrl": e.screenshot_url,
                    "isDefault": e.is_default_theme(),
                    "downloaded": downloaded.contains(e.slug.as_str()),
                    "current": current.as_deref() == Some(e.slug.as_str()),
                })
            })
            .collect();
        print_json(&json!({
            "fetchedAt": index.catalog.as_ref().map(|c| crate::json::format_date(c.fetched_at)),
            "notices": index.notices,
            "themes": themes,
        }))?;
        return Ok(Outcome::Success);
    }

    ctx.show_notices(&index.notices);
    let out = &ctx.out;
    if shown.is_empty() {
        match &args.search {
            Some(search) => println!("No themes match “{search}”."),
            None => println!("No themes to show."),
        }
        return Ok(Outcome::Success);
    }

    let slug_width = shown.iter().map(|e| e.slug.chars().count()).max().unwrap_or(0);
    let name_width = shown.iter().map(|e| e.name.chars().count()).max().unwrap_or(0);
    let (defaults, community): (Vec<&CatalogEntry>, Vec<&CatalogEntry>) =
        shown.iter().partition(|e| e.is_default_theme());
    let mut first = true;
    for (title, section) in [("Included with Omarchy", defaults), ("Community", community)] {
        if section.is_empty() {
            continue;
        }
        if !first {
            println!();
        }
        first = false;
        println!("{}", out.bold(&format!("{title} ({})", section.len())));
        for entry in section {
            let marker = if current.as_deref() == Some(entry.slug.as_str()) {
                out.green("●")
            } else if downloaded.contains(entry.slug.as_str()) {
                out.cyan("✓")
            } else {
                " ".to_string()
            };
            let repo = if entry.is_default_theme() { String::new() } else { out.dim(&entry.repo_display()) };
            let line = format!("{marker} {}  {}  {repo}", pad(&entry.slug, slug_width), pad(&entry.name, name_width));
            println!("{}", line.trim_end());
        }
    }
    if out.color {
        println!("\n{}", out.dim("✓ downloaded   ● on the desktop"));
    }
    Ok(Outcome::Success)
}

/// One wallpaper as `show` lists it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WallpaperView {
    number: usize,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    current: bool,
}

/// Everything `show` prints, from disk (downloaded) or from GitHub.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ThemeView {
    slug: String,
    name: String,
    repo_url: String,
    is_default: bool,
    downloaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    downloaded_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    directory: Option<String>,
    current: bool,
    mode: AppearanceMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette: Option<Palette>,
    #[serde(skip_serializing_if = "Option::is_none")]
    palette_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    terminal_colors: Option<TerminalColors>,
    wallpapers: Vec<WallpaperView>,
    stale: bool,
}

fn from_installed(theme: &InstalledTheme, entry: &CatalogEntry, current: Option<(&str, Option<&str>)>) -> ThemeView {
    let is_current = current.is_some_and(|(slug, _)| slug == theme.slug);
    ThemeView {
        slug: theme.slug.clone(),
        name: theme.name.clone(),
        repo_url: theme.repo_url.clone(),
        is_default: entry.is_default_theme(),
        downloaded: true,
        downloaded_at: Some(crate::json::format_date(theme.downloaded_at)),
        directory: Some(theme.directory.display().to_string()),
        current: is_current,
        mode: theme.mode,
        terminal_colors: theme.palette.as_ref().map(TerminalColors::from_palette),
        palette: theme.palette.clone(),
        palette_error: theme.palette.is_none().then(|| "No palette was saved with this theme.".to_string()),
        wallpapers: theme
            .wallpapers
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let path = theme.wallpaper_path(name);
                WallpaperView {
                    number: i + 1,
                    name: name.clone(),
                    size: std::fs::metadata(&path).ok().map(|m| m.len()),
                    url: None,
                    path: Some(path.display().to_string()),
                    current: is_current && current.and_then(|(_, w)| w) == Some(name.as_str()),
                }
            })
            .collect(),
        stale: false,
    }
}

fn from_details(details: &ThemeDetails) -> ThemeView {
    ThemeView {
        slug: details.entry.slug.clone(),
        name: details.entry.name.clone(),
        repo_url: details.entry.repo_url.clone(),
        is_default: details.entry.is_default_theme(),
        downloaded: false,
        downloaded_at: None,
        directory: None,
        current: false,
        mode: details.mode,
        terminal_colors: details.palette.as_ref().map(TerminalColors::from_palette),
        palette: details.palette.clone(),
        palette_error: details.palette_error.clone(),
        wallpapers: details
            .wallpapers
            .iter()
            .enumerate()
            .map(|(i, w)| WallpaperView {
                number: i + 1,
                name: w.file_name().to_string(),
                size: w.size,
                url: Some(w.download_url.clone()),
                path: None,
                current: false,
            })
            .collect(),
        stale: details.is_stale,
    }
}

pub fn show(ctx: &Context, args: &ShowArgs) -> Result<Outcome> {
    let index = ctx.theme_index(false)?;
    let entry = index.find(&args.theme)?;
    let settings = ctx.settings().load();
    let current = settings.last_applied_slug.as_deref().map(|s| (s, settings.last_applied_wallpaper.as_deref()));

    let (view, stale_reason) = match index.installed(&entry.slug) {
        Some(theme) => (from_installed(theme, entry, current), None),
        None => {
            let details = ctx.resolve(entry)?;
            (from_details(&details), details.stale_reason.map(|r| r.to_string()))
        }
    };

    if args.json {
        print_json(&view)?;
        return Ok(Outcome::Success);
    }
    if let Some(reason) = stale_reason {
        ctx.out.warn(&format!("Showing what was saved from GitHub earlier. {reason}"));
    }
    print_theme(&ctx.out, &view);
    Ok(Outcome::Success)
}

fn print_theme(out: &Output, view: &ThemeView) {
    println!("{}  {}", out.bold(&view.name), out.dim(&view.slug));
    let source = if view.is_default { "Included with Omarchy".to_string() } else { "Community theme".to_string() };
    println!("{source} · {}", view.repo_url);
    let declared = view.palette.as_ref().and_then(|p| p.declared_mode).is_some();
    let mode = match view.mode {
        AppearanceMode::Dark => "Dark",
        AppearanceMode::Light => "Light",
    };
    println!("{mode} theme{}", if declared || view.palette.is_none() { "" } else { " (from its background color)" });
    match (&view.downloaded_at, &view.directory) {
        (Some(at), Some(dir)) => {
            let when = crate::json::parse_date(at)
                .map(|d| d.with_timezone(&chrono::Local).format("%b %-d, %Y").to_string())
                .unwrap_or_default();
            println!("Downloaded {when} to {dir}");
        }
        _ => println!(
            "{}",
            out.dim(&format!("Not downloaded. `omatheme download {}` saves it for offline use.", view.slug))
        ),
    }
    if view.current {
        println!("{}", out.green("● On the desktop"));
    }

    println!();
    match &view.palette {
        Some(palette) => {
            println!("{}", out.bold(&format!("Colors ({})", palette.source.file_name())));
            let rows = [
                ("Background", Some(palette.background)),
                ("Foreground", Some(palette.foreground)),
                ("Accent", Some(palette.accent)),
                ("Cursor", palette.cursor),
                ("Selection", palette.selection),
                ("Muted", palette.muted),
                ("Bright foreground", palette.bright_foreground),
            ];
            for (label, color) in rows {
                if let Some(color) = color {
                    println!("  {}{}  {color}", out.swatch(color), pad(label, 17));
                }
            }
            if !palette.swatches.is_empty() {
                let width = palette.swatches.iter().map(|s| s.name.chars().count()).max().unwrap_or(0);
                for swatch in &palette.swatches {
                    println!("  {}{}  {}", out.swatch(swatch.color), pad(&swatch.name, width.max(17)), swatch.color);
                }
            }
        }
        None => println!("{}", out.yellow(view.palette_error.as_deref().unwrap_or("This theme has no palette."))),
    }

    if let Some(terminal) = &view.terminal_colors {
        println!();
        println!("{}", out.bold(&format!("Terminal colors (“{}”)", scheme_name(&view.name))));
        for (label, colors) in [("normal", &terminal.ansi[..8]), ("bright", &terminal.ansi[8..])] {
            let swatches: String = colors.iter().map(|c| out.swatch(*c)).collect();
            let hexes: Vec<String> = colors.iter().map(|c| c.hex()).collect();
            println!("  {label}  {swatches}{}", out.dim(&hexes.join(" ")));
        }
    }

    println!();
    if view.wallpapers.is_empty() {
        println!("{}", out.dim("No wallpapers."));
        return;
    }
    println!("{}", out.bold(&format!("Wallpapers ({})", view.wallpapers.len())));
    let width = view.wallpapers.iter().map(|w| w.name.chars().count()).max().unwrap_or(0);
    let number_width = view.wallpapers.len().to_string().len();
    for w in &view.wallpapers {
        let size = w.size.map(format_bytes).unwrap_or_default();
        let marker = if w.current { format!("  {}", out.green("● on the desktop")) } else { String::new() };
        println!("  {:>number_width$}  {}  {}{marker}", w.number, pad(&w.name, width), out.dim(&size));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(slug: &str, name: &str, repo: &str) -> CatalogEntry {
        CatalogEntry { slug: slug.into(), name: name.into(), repo_url: repo.into(), screenshot_url: None }
    }

    fn entries() -> Vec<CatalogEntry> {
        vec![
            entry(
                "omarchy.tokyo-night",
                "Tokyo Night",
                "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night",
            ),
            entry("aetheria", "Aetheria", "https://github.com/JJDizz1L/aetheria"),
            entry("amberbyte", "Amberbyte", "https://github.com/tahfizhabib/omarchy-amberbyte-theme"),
        ]
    }

    fn slugs(found: Vec<&CatalogEntry>) -> Vec<&str> {
        found.into_iter().map(|e| e.slug.as_str()).collect()
    }

    #[test]
    fn search_matches_name_repo_or_slug_case_insensitively() {
        let all = entries();
        let none = HashSet::new();
        assert_eq!(slugs(filter(&all, &none, Some("TOKYO"), false, false, false)), ["omarchy.tokyo-night"]);
        assert_eq!(slugs(filter(&all, &none, Some("tahfizhabib"), false, false, false)), ["amberbyte"]);
        assert_eq!(slugs(filter(&all, &none, Some("  "), false, false, false)).len(), 3);
    }

    #[test]
    fn filters_combine() {
        let all = entries();
        let downloaded = HashSet::from(["aetheria", "omarchy.tokyo-night"]);
        assert_eq!(slugs(filter(&all, &downloaded, None, true, false, false)), ["omarchy.tokyo-night", "aetheria"]);
        assert_eq!(slugs(filter(&all, &downloaded, None, true, false, true)), ["aetheria"]);
        assert_eq!(slugs(filter(&all, &downloaded, None, false, true, false)), ["omarchy.tokyo-night"]);
    }
}
