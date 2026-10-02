//! `apply`, `current`, `wallpaper` and `restore`: the commands that change (or describe) the desktop.

use anyhow::Result;
use serde_json::json;

use super::super::args::{ApplyArgs, RestoreArgs, WallpaperArgs};
use super::super::context::Context;
use super::super::output::{Output, print_json};
use super::super::{Outcome, UsageError};
use super::library::download_theme;
use crate::platform;
use crate::store::{AppSettings, InstalledTheme};
use crate::theming::{
    ApplyOptions, ApplyRequest, ApplyResult, ApplyStep, ApplySummary, StepOutcome, SummaryKind, ThemeApplier,
    WallpaperFit, label,
};

/// The wallpaper to preselect for a downloaded theme: the one last applied, else the first.
pub fn preferred_wallpaper(theme: &InstalledTheme, settings: &AppSettings) -> Option<String> {
    if settings.last_applied_slug.as_deref() == Some(theme.slug.as_str())
        && let Some(last) = &settings.last_applied_wallpaper
        && theme.wallpapers.contains(last)
    {
        return Some(last.clone());
    }
    theme.wallpapers.first().cloned()
}

/// A wallpaper by its number in `show` (from 1), its file name, or a unique start of it.
pub fn choose_wallpaper(theme: &InstalledTheme, spec: &str) -> Result<String, UsageError> {
    let spec = spec.trim();
    if theme.wallpapers.is_empty() {
        return Err(UsageError(format!("{} has no wallpapers.", theme.name)));
    }
    if let Ok(number) = spec.parse::<usize>() {
        return theme.wallpapers.get(number.wrapping_sub(1)).cloned().ok_or_else(|| {
            let count = theme.wallpapers.len();
            UsageError(format!("{} has {count} wallpapers; pick a number from 1 to {count}.", theme.name))
        });
    }
    let lower = spec.to_lowercase();
    if let Some(exact) = theme.wallpapers.iter().find(|w| w.to_lowercase() == lower) {
        return Ok(exact.clone());
    }
    let matches: Vec<&String> = theme.wallpapers.iter().filter(|w| w.to_lowercase().starts_with(&lower)).collect();
    match matches.as_slice() {
        [one] => Ok((*one).clone()),
        [] => Err(UsageError(format!(
            "{} has no wallpaper “{spec}”. `omatheme show {}` lists them.",
            theme.name, theme.slug
        ))),
        many => Err(UsageError(format!(
            "“{spec}” matches {} wallpapers: {}.",
            many.len(),
            many.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// The next or previous wallpaper after `current` (wrapping), or the first if none is current.
pub fn step_wallpaper(theme: &InstalledTheme, current: Option<&str>, forward: bool) -> Option<String> {
    let count = theme.wallpapers.len();
    if count == 0 {
        return None;
    }
    let index = current.and_then(|c| theme.wallpapers.iter().position(|w| w == c));
    let next = match (index, forward) {
        (None, _) => 0,
        (Some(i), true) => (i + 1) % count,
        (Some(i), false) => (i + count - 1) % count,
    };
    Some(theme.wallpapers[next].clone())
}

/// The fit to use: the one asked for (which must be supported), else the saved one if this OS has
/// it, else Fill (e.g. Tile saved by the Windows app, used on a Mac).
pub fn choose_fit(
    asked: Option<WallpaperFit>,
    saved: WallpaperFit,
    supported: &[WallpaperFit],
) -> Result<WallpaperFit, UsageError> {
    match asked {
        Some(fit) if supported.contains(&fit) => Ok(fit),
        Some(fit) => Err(UsageError(format!(
            "{} isn't available on {}. Use one of: {}.",
            fit.display_name(),
            platform::os_name(),
            supported.iter().map(|f| f.id()).collect::<Vec<_>>().join(", ")
        ))),
        None if supported.contains(&saved) => Ok(saved),
        None => Ok(WallpaperFit::Fill),
    }
}

/// The apply choices: the app's one-click defaults, changed by the flags.
pub fn apply_options(defaults: &ApplyOptions, args: &ApplyArgs, fit: WallpaperFit) -> ApplyOptions {
    let pick = |off: bool, on: bool, default: bool| if off { false } else { on || default };
    ApplyOptions {
        wallpaper: pick(args.no_wallpaper, args.wallpaper.is_some(), defaults.wallpaper),
        appearance_mode: pick(args.no_mode, args.mode, defaults.appearance_mode),
        accent_color: pick(args.no_accent, args.accent, defaults.accent_color),
        fit,
        extra: defaults.extra.clone(),
    }
}

/// How one step went, in words.
fn step_text(result: &crate::theming::StepResult, request: &ApplyRequest) -> String {
    match result.outcome {
        StepOutcome::Applied if result.step == ApplyStep::SaveOriginal => {
            "saved first, so `omatheme restore` can put it back".into()
        }
        StepOutcome::Applied => match result.step {
            ApplyStep::Wallpaper => {
                let name =
                    request.wallpaper.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
                format!("{} ({})", name.unwrap_or_default(), request.options.fit.display_name())
            }
            ApplyStep::AccentColor => request.accent.map(|a| a.hex()).unwrap_or_default(),
            _ => "done".into(),
        },
        StepOutcome::SkippedByUser => "left alone".into(),
        StepOutcome::NotSupported => {
            let mut text = format!("not supported on {}", platform::os_name());
            if let Some(hint) = platform::appearance_settings_hint() {
                text += &format!(" (change it in {hint})");
            }
            text
        }
        StepOutcome::NoData => match result.step {
            ApplyStep::Wallpaper => "the theme has no wallpapers".into(),
            ApplyStep::AccentColor => "the theme has no colors".into(),
            _ => "nothing to apply".into(),
        },
        StepOutcome::NotAttempted => "not attempted".into(),
        StepOutcome::Failed => format!("failed: {}", result.error.as_deref().unwrap_or("Unknown error.")),
    }
}

fn step_title(step: ApplyStep, mode: crate::palette::AppearanceMode) -> String {
    match step {
        ApplyStep::SaveOriginal => "Your desktop".into(),
        ApplyStep::AppearanceMode => "Light/dark".into(),
        other => crate::catalog::default_themes::uppercase_first(label(other, mode).trim_start_matches("the ")),
    }
}

/// Lists each step and the summary; returns the exit outcome.
fn report(out: &Output, result: &ApplyResult, request: &ApplyRequest, summary: &ApplySummary) -> Outcome {
    for step in &result.steps {
        let mark = match step.outcome {
            StepOutcome::Applied => out.green("✓"),
            StepOutcome::Failed => out.red("✗"),
            _ => out.dim("–"),
        };
        println!("  {mark} {:<14} {}", step_title(step.step, request.mode), step_text(step, request));
    }
    let title = match summary.kind {
        SummaryKind::Success => out.green(&summary.title),
        SummaryKind::Info => out.bold(&summary.title),
        SummaryKind::Warning => out.yellow(&summary.title),
        SummaryKind::Error => out.red(&summary.title),
    };
    println!("{title}. {}", summary.message);
    outcome(summary)
}

fn outcome(summary: &ApplySummary) -> Outcome {
    match summary.kind {
        SummaryKind::Success | SummaryKind::Info => Outcome::Success,
        SummaryKind::Warning => Outcome::Partial,
        SummaryKind::Error => Outcome::Failure,
    }
}

/// Asks before changing the desktop. A dry run asks too (it says so), so the confirmation path can be
/// exercised without changing anything.
fn confirm(ctx: &Context, question: &str, yes: bool) -> Result<bool> {
    let question =
        if ctx.dry_run { format!("{question} (dry run: nothing will change)") } else { question.to_string() };
    ctx.out.confirm(&question, yes)
}

fn print_dry_run_actions(ctx: &Context) {
    let actions = ctx.dry_run_actions();
    if !actions.is_empty() {
        ctx.out.note(&format!(
            "Dry run: nothing changed. A real run would {}.",
            crate::theming::join_list(&actions.iter().map(String::as_str).collect::<Vec<_>>())
        ));
    }
    if !ctx.persists() {
        ctx.out.note("Dry run with the real data folder: settings.json and the saved desktop weren't written.");
    }
}

/// Runs an apply and records it in settings.json as the apps do.
/// What an apply did, after recording it in settings.json.
pub struct Applied {
    pub result: ApplyResult,
    pub summary: ApplySummary,
    pub wallpaper_file: Option<String>,
    /// settings.json couldn't be saved (the desktop changed anyway).
    pub settings_error: Option<anyhow::Error>,
}

/// Applies `request` and records it in settings.json with `AfterApply`, as the apps do.
pub fn apply_and_record(
    ctx: &Context,
    applier: &ThemeApplier,
    theme: &InstalledTheme,
    request: &ApplyRequest,
) -> Result<Applied> {
    let result = applier.apply(request, &mut |_| {}, &ctx.cancel)?;
    let wallpaper_file =
        request.wallpaper.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    let settings_error =
        ctx.settings().update(|s| *s = s.after_apply(&theme.slug, wallpaper_file.as_deref(), &result)).err();
    let summary = ApplySummary::describe(&result, &theme.name, theme.mode, applier.accent_color_note());
    Ok(Applied { result, summary, wallpaper_file, settings_error })
}

/// Puts back the saved desktop and forgets the current theme. Ok(false): nothing was saved. The
/// error inside is settings.json failing to save.
pub fn restore_and_record(ctx: &Context, applier: &ThemeApplier) -> Result<(bool, Option<anyhow::Error>)> {
    let restored = applier.restore_original()?;
    let settings_error = if restored {
        ctx.settings()
            .update(|s| {
                s.last_applied_slug = None;
                s.last_applied_wallpaper = None;
            })
            .err()
    } else {
        None
    };
    Ok((restored, settings_error))
}

fn run_apply(
    ctx: &Context,
    applier: &ThemeApplier,
    theme: &InstalledTheme,
    request: &ApplyRequest,
    as_json: bool,
) -> Result<Outcome> {
    let Applied { result, summary, wallpaper_file, settings_error } = apply_and_record(ctx, applier, theme, request)?;

    let outcome = if as_json {
        print_json(&json!({
            "theme": theme.slug,
            "wallpaper": wallpaper_file,
            "summary": summary,
            "steps": result.steps,
            "dryRun": ctx.dry_run,
            "dryRunActions": ctx.dry_run_actions(),
        }))?;
        outcome(&summary)
    } else {
        report(&ctx.out, &result, request, &summary)
    };
    if let Some(error) = settings_error {
        ctx.out.warn(&format!("The theme was applied, but settings.json couldn't be saved: {error:#}"));
    }
    if !as_json {
        print_dry_run_actions(ctx);
    }
    Ok(outcome)
}

pub fn apply(ctx: &Context, args: &ApplyArgs) -> Result<Outcome> {
    // Before any network: say so at once where applying isn't supported.
    let applier = ctx.applier()?;
    let settings = ctx.settings().load();
    let fit = choose_fit(args.fit, settings.apply_defaults.fit, &applier.supported_fits())?;

    let index = ctx.theme_index(false)?;
    if !args.json {
        ctx.show_notices(&index.notices);
    }
    let entry = index.find(&args.theme)?.clone();
    let theme = match index.installed(&entry.slug) {
        Some(theme) => theme.clone(),
        None => {
            ctx.clean_up_store();
            if !args.json {
                eprintln!("Downloading {} first…", entry.name);
            }
            download_theme(ctx, &entry)?
        }
    };

    let wallpaper = match &args.wallpaper {
        Some(spec) => Some(choose_wallpaper(&theme, spec)?),
        None => preferred_wallpaper(&theme, &settings),
    };
    let options = apply_options(&settings.apply_defaults, args, fit);
    let request = ApplyRequest::from_theme(&theme, wallpaper.as_deref(), options);

    if !args.yes && !args.json {
        print_plan(ctx, &applier, &theme, &request);
    }
    if !confirm(ctx, "Apply it?", args.yes)? {
        println!("Nothing was changed.");
        return Ok(Outcome::Success);
    }
    run_apply(ctx, &applier, &theme, &request, args.json)
}

/// What an apply will do, before asking.
fn print_plan(ctx: &Context, applier: &ThemeApplier, theme: &InstalledTheme, request: &ApplyRequest) {
    let out = &ctx.out;
    let capabilities = applier.capabilities();
    println!("Apply {} to this computer:", out.bold(&theme.name));
    let line = |title: &str, enabled: bool, supported: bool, has_data: bool, what: String| {
        let text = if !enabled {
            "leave alone".to_string()
        } else if !supported {
            format!("not supported on {}", platform::os_name())
        } else if !has_data {
            "nothing to apply".to_string()
        } else {
            what
        };
        println!("  {:<14} {text}", title);
    };
    let wallpaper = request.wallpaper.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    let fill = request
        .background
        .filter(|_| request.options.fit.shows_fill_color())
        .map(|b| format!(", {b} around it"))
        .unwrap_or_default();
    line(
        "Wallpaper",
        request.options.wallpaper,
        capabilities.wallpaper,
        wallpaper.is_some(),
        format!("{} ({}{fill})", wallpaper.unwrap_or_default(), request.options.fit.display_name()),
    );
    line(
        "Light/dark",
        request.options.appearance_mode,
        capabilities.appearance_mode,
        true,
        format!("{} mode", request.mode),
    );
    line(
        "Accent color",
        request.options.accent_color,
        capabilities.accent_color,
        request.accent.is_some(),
        request.accent.map(|a| a.hex()).unwrap_or_default(),
    );
    if !applier.has_original_snapshot() {
        println!("Your current desktop is saved first, so `omatheme restore` can put it back.");
    }
}

pub fn current(ctx: &Context, as_json: bool) -> Result<Outcome> {
    let settings = ctx.settings().load();
    let theme = settings.last_applied_slug.as_deref().and_then(|slug| ctx.store().get(slug));
    // Read-only: what the OS says is on each display right now.
    let displays = platform::live_backend(&ctx.paths).map(|b| b.current_wallpapers()).unwrap_or_default();
    let applier = ctx.applier().ok();
    let has_snapshot = applier.as_ref().is_some_and(|a| a.has_original_snapshot());

    if as_json {
        print_json(&json!({
            "slug": settings.last_applied_slug,
            "name": theme.as_ref().map(|t| t.name.clone()),
            "downloaded": theme.is_some(),
            "wallpaper": settings.last_applied_wallpaper,
            "wallpaperPath": theme.as_ref().zip(settings.last_applied_wallpaper.as_ref()).map(|(t, w)| t.wallpaper_path(w)),
            "mode": theme.as_ref().map(|t| t.mode),
            "palette": theme.as_ref().and_then(|t| t.palette.clone()),
            "originalDesktopSaved": has_snapshot,
            "displays": displays.iter().map(|d| json!({ "display": d.display, "picture": d.picture })).collect::<Vec<_>>(),
        }))?;
        return Ok(Outcome::Success);
    }

    let out = &ctx.out;
    match (&settings.last_applied_slug, &theme) {
        (None, _) => println!("No theme is applied. `omatheme apply <THEME>` applies one."),
        (Some(slug), None) => {
            println!("{slug} was applied last, but it isn't downloaded any more.")
        }
        (Some(_), Some(theme)) => {
            println!("{}  {}", out.bold(&theme.name), out.dim(&theme.slug));
            match &settings.last_applied_wallpaper {
                Some(wallpaper) => {
                    let number = theme.wallpapers.iter().position(|w| w == wallpaper).map(|i| i + 1);
                    let of = number.map(|n| format!(" ({n} of {})", theme.wallpapers.len())).unwrap_or_default();
                    println!("Wallpaper  {wallpaper}{of}");
                }
                None => println!("Wallpaper  {}", out.dim("none of this theme's (its wallpaper wasn't applied)")),
            }
            println!("Mode       {}", theme.mode);
            if let Some(palette) = &theme.palette {
                let colors: String = [palette.background, palette.foreground, palette.accent]
                    .iter()
                    .map(|c| format!("{}{c}  ", out.swatch(*c)))
                    .collect();
                println!("Colors     {}", colors.trim_end());
            }
        }
    }
    for display in &displays {
        let picture = display.picture.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "no picture".into());
        println!("{}", out.dim(&format!("{}: {picture}", display.display)));
    }
    if applier.is_some() {
        println!(
            "{}",
            out.dim(if has_snapshot {
                "Your original desktop is saved; `omatheme restore` puts it back."
            } else {
                "No original desktop is saved yet (it's saved before the first apply)."
            })
        );
    }
    Ok(Outcome::Success)
}

pub fn wallpaper(ctx: &Context, args: &WallpaperArgs) -> Result<Outcome> {
    let applier = ctx.applier()?;
    let settings = ctx.settings().load();
    let Some(slug) = settings.last_applied_slug.clone() else {
        anyhow::bail!("No theme is on the desktop. Apply one first with `omatheme apply <THEME>`.");
    };
    let Some(theme) = ctx.store().get(&slug) else {
        anyhow::bail!("The theme on the desktop ({slug}) isn't downloaded any more. Apply a theme first.");
    };
    let current = settings.last_applied_wallpaper.as_deref();
    let chosen = match args.which.to_lowercase().as_str() {
        "next" => step_wallpaper(&theme, current, true),
        "prev" | "previous" => step_wallpaper(&theme, current, false),
        _ => Some(choose_wallpaper(&theme, &args.which)?),
    };
    let Some(chosen) = chosen else { anyhow::bail!("{} has no wallpapers.", theme.name) };
    if Some(chosen.as_str()) == current && args.fit.is_none() {
        if args.json {
            print_json(&json!({ "theme": theme.slug, "wallpaper": chosen, "changed": false }))?;
        } else {
            println!("{chosen} is already on the desktop.");
        }
        return Ok(Outcome::Success);
    }

    // Wallpaper only: the theme's light/dark and accent are already applied.
    let fit = choose_fit(args.fit, settings.apply_defaults.fit, &applier.supported_fits())?;
    let request = ApplyRequest::from_theme(&theme, Some(&chosen), ApplyOptions::wallpaper_only(fit));
    if !confirm(ctx, &format!("Set {chosen} as the wallpaper?"), args.yes)? {
        println!("Nothing was changed.");
        return Ok(Outcome::Success);
    }
    run_apply(ctx, &applier, &theme, &request, args.json)
}

pub fn restore(ctx: &Context, args: &RestoreArgs) -> Result<Outcome> {
    let applier = ctx.applier()?;
    if !applier.has_original_snapshot() {
        if args.json {
            print_json(&json!({ "restored": false, "message": "No saved desktop was found." }))?;
        } else {
            println!("Nothing to restore: no saved desktop was found.");
        }
        return Ok(Outcome::Success);
    }
    if !confirm(ctx, "Put back the desktop you had before the first apply?", args.yes)? {
        println!("Nothing was changed.");
        return Ok(Outcome::Success);
    }

    match restore_and_record(ctx, &applier) {
        Ok((restored, settings_error)) => {
            if let Some(error) = settings_error {
                ctx.out.warn(&format!("settings.json couldn't be saved: {error:#}"));
            }
            if args.json {
                print_json(&json!({ "restored": restored, "dryRun": ctx.dry_run }))?;
            } else {
                println!("{} Your previous wallpaper is back.", ctx.out.green("Original desktop restored."));
                print_dry_run_actions(ctx);
            }
            Ok(Outcome::Success)
        }
        Err(error) => {
            if args.json {
                print_json(&json!({ "restored": false, "error": format!("{error:#}") }))?;
            } else {
                ctx.out.error(&format!("Couldn't restore your desktop: {error:#}"));
            }
            Ok(Outcome::Failure)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::palette::AppearanceMode;

    fn theme(wallpapers: &[&str]) -> InstalledTheme {
        InstalledTheme {
            slug: "tokyo".into(),
            name: "Tokyo".into(),
            repo_url: "https://github.com/o/tokyo".into(),
            palette: None,
            mode: AppearanceMode::Dark,
            wallpapers: wallpapers.iter().map(|w| w.to_string()).collect(),
            screenshot_file: None,
            downloaded_at: chrono::Utc::now(),
            directory: PathBuf::from("/themes/tokyo"),
        }
    }

    fn args(flags: &[&str]) -> ApplyArgs {
        use clap::Parser;
        let mut argv = vec!["omatheme", "apply", "tokyo"];
        argv.extend(flags);
        match super::super::super::args::Cli::try_parse_from(argv).unwrap().command {
            super::super::super::args::Command::Apply(args) => args,
            _ => unreachable!(),
        }
    }

    #[test]
    fn wallpapers_are_chosen_by_number_name_or_prefix() {
        let t = theme(&["0-winding-road.webp", "1-quattro.webp", "1-swirl.webp"]);
        assert_eq!(choose_wallpaper(&t, "2").unwrap(), "1-quattro.webp");
        assert_eq!(choose_wallpaper(&t, "1-QUATTRO.webp").unwrap(), "1-quattro.webp");
        assert_eq!(choose_wallpaper(&t, "0-wind").unwrap(), "0-winding-road.webp");
        assert!(choose_wallpaper(&t, "1").is_ok());
        assert!(choose_wallpaper(&t, "4").unwrap_err().0.contains("from 1 to 3"));
        assert!(choose_wallpaper(&t, "0").is_err());
        assert!(choose_wallpaper(&t, "1-").unwrap_err().0.contains("matches 2 wallpapers"));
        assert!(choose_wallpaper(&t, "zzz").unwrap_err().0.contains("omatheme show tokyo"));
        assert!(choose_wallpaper(&theme(&[]), "1").is_err());
    }

    #[test]
    fn next_and_previous_wrap_around() {
        let t = theme(&["a.png", "b.png", "c.png"]);
        assert_eq!(step_wallpaper(&t, Some("c.png"), true).as_deref(), Some("a.png"));
        assert_eq!(step_wallpaper(&t, Some("a.png"), false).as_deref(), Some("c.png"));
        assert_eq!(step_wallpaper(&t, Some("b.png"), true).as_deref(), Some("c.png"));
        assert_eq!(step_wallpaper(&t, None, false).as_deref(), Some("a.png"));
        assert_eq!(step_wallpaper(&theme(&[]), None, true), None);
    }

    #[test]
    fn the_preferred_wallpaper_is_the_one_last_applied_for_that_theme() {
        let t = theme(&["a.png", "b.png"]);
        let mut settings = AppSettings {
            last_applied_slug: Some("tokyo".into()),
            last_applied_wallpaper: Some("b.png".into()),
            ..Default::default()
        };
        assert_eq!(preferred_wallpaper(&t, &settings).as_deref(), Some("b.png"));
        settings.last_applied_slug = Some("snow".into());
        assert_eq!(preferred_wallpaper(&t, &settings).as_deref(), Some("a.png"));
    }

    #[test]
    fn fits_must_be_supported_and_saved_ones_fall_back_to_fill() {
        let mac = [WallpaperFit::Fill, WallpaperFit::Fit, WallpaperFit::Stretch, WallpaperFit::Center];
        assert_eq!(choose_fit(Some(WallpaperFit::Center), WallpaperFit::Fill, &mac).unwrap(), WallpaperFit::Center);
        assert!(choose_fit(Some(WallpaperFit::Tile), WallpaperFit::Fill, &mac).is_err());
        assert_eq!(choose_fit(None, WallpaperFit::Span, &mac).unwrap(), WallpaperFit::Fill);
        assert_eq!(choose_fit(None, WallpaperFit::Fit, &mac).unwrap(), WallpaperFit::Fit);
    }

    #[test]
    fn flags_change_the_saved_defaults() {
        let saved = ApplyOptions::new(true, false, true, WallpaperFit::Fill);

        let plain = apply_options(&saved, &args(&[]), WallpaperFit::Fill);
        assert_eq!((plain.wallpaper, plain.appearance_mode, plain.accent_color), (true, false, true));

        let flags = apply_options(&saved, &args(&["--mode", "--no-accent"]), WallpaperFit::Fit);
        assert_eq!(
            (flags.wallpaper, flags.appearance_mode, flags.accent_color, flags.fit),
            (true, true, false, WallpaperFit::Fit)
        );

        let no_wallpaper = ApplyOptions::new(false, true, true, WallpaperFit::Fill);
        assert!(apply_options(&no_wallpaper, &args(&["--wallpaper", "2"]), WallpaperFit::Fill).wallpaper);
        assert!(!apply_options(&saved, &args(&["--no-wallpaper"]), WallpaperFit::Fill).wallpaper);
        // The last of a pair wins.
        assert!(!apply_options(&saved, &args(&["--mode", "--no-mode"]), WallpaperFit::Fill).appearance_mode);
    }
}
