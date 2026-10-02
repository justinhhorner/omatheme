//! The files this tool writes match docs/data-format.md, the format the apps share. The apps'
//! `DataFormatTests` check the same fixtures (fixtures/data), so every client writes identical files.

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use serde_json::Value;

use super::{AppSettings, InstalledTheme, SettingsStore};
use crate::color::c;
use crate::palette::{AppearanceMode, NamedColor, Palette, PaletteSource};
use crate::paths::AppPaths;
use crate::test_support::fixture;
use crate::theming::{ApplyOptions, ApplyResult, ApplyStep, DesktopSnapshot, StepOutcome, StepResult, WallpaperFit};

/// Equal as JSON: same keys, types and values (key order and whitespace don't matter).
fn assert_same_json(expected_fixture: &str, actual: &impl serde::Serialize) {
    let expected: Value = serde_json::from_str(&fixture(expected_fixture)).unwrap();
    let actual = serde_json::to_value(actual).unwrap();
    assert_eq!(actual, expected, "\nexpected {expected_fixture}:\n{expected:#}\nactual:\n{actual:#}");
}

fn read<T: serde::de::DeserializeOwned>(file: &str) -> T {
    crate::json::from_slice(fixture(file).as_bytes()).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn tokyo_night() -> InstalledTheme {
    InstalledTheme {
        slug: "omarchy.tokyo-night".into(),
        name: "Tokyo Night".into(),
        repo_url: "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night".into(),
        palette: Some(Palette {
            selection: Some(c("#292e42")),
            muted: Some(c("#414868")),
            bright_foreground: Some(c("#c0caf5")),
            declared_mode: Some(AppearanceMode::Dark),
            swatches: vec![NamedColor::new("Red", c("#f7768e")), NamedColor::new("Bright red", c("#ff7a93"))],
            ..Palette::new(c("#1a1b26"), c("#a9b1d6"), c("#7aa2f7"), PaletteSource::ColorsToml)
        }),
        mode: AppearanceMode::Dark,
        wallpapers: vec!["0-winding-road.webp".into(), "1-quattro.webp".into()],
        screenshot_file: Some("screenshot.png".into()),
        downloaded_at: Utc.timestamp_millis_opt(1_790_499_600_123).unwrap(), // 2026-09-27T09:00:00.123Z
        directory: PathBuf::from("/Users/me/Library/Application Support/OmarchyThemes/themes/omarchy.tokyo-night"),
    }
}

#[test]
fn theme_manifest_is_written_in_the_shared_format() {
    // "repoUrl", no nulls, no directory, UTC date with "Z".
    assert_same_json("data/theme.json", &tokyo_night());
}

#[test]
fn theme_manifests_read_in_every_format_rewrite_in_the_shared_one() {
    for file in ["data/theme.json", "data/legacy/windows-theme.json", "data/legacy/macos-theme.json"] {
        let theme: InstalledTheme = read(file);

        assert_eq!(theme.repo_url, "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night", "{file}");
        assert_eq!(theme.downloaded_at, tokyo_night().downloaded_at, "{file}");
        assert_eq!(theme.palette.as_ref().unwrap().cursor, None, "{file}");
        assert_eq!(theme.palette.as_ref().unwrap().bright_foreground, Some(c("#c0caf5")), "{file}");
        assert_same_json("data/theme.json", &theme);
    }
}

#[test]
fn a_manifest_without_a_mode_takes_the_palettes() {
    let mut json: Value = serde_json::from_str(&fixture("data/theme.json")).unwrap();
    json.as_object_mut().unwrap().remove("mode");
    json["palette"]["declaredMode"] = "light".into();

    let theme: InstalledTheme = serde_json::from_value(json).unwrap();

    assert_eq!(theme.mode, AppearanceMode::Light);
}

fn sample_settings() -> AppSettings {
    AppSettings {
        welcome_seen: true,
        apply_defaults: ApplyOptions::new(true, false, true, WallpaperFit::Center),
        last_applied_slug: Some("omarchy.tokyo-night".into()),
        last_applied_wallpaper: Some("1-quattro.webp".into()),
        terminal_app: Some("ghostty".into()),
        extra: Default::default(),
    }
}

#[test]
fn settings_are_written_in_the_shared_format() {
    assert_same_json("data/settings.json", &sample_settings());
    assert_same_json("data/settings-default.json", &AppSettings::default());
    let read_back: AppSettings = read("data/settings.json");
    assert_eq!(read_back, sample_settings());
    assert_same_json("data/settings.json", &read_back);
}

#[test]
fn older_windows_settings_still_read() {
    let settings: AppSettings = read("data/legacy/windows-settings.json");

    assert!(settings.welcome_seen);
    assert_eq!(settings.apply_defaults.fit, WallpaperFit::Center);
    assert_eq!(settings.last_applied_slug.as_deref(), Some("omarchy.tokyo-night"));
    assert_eq!(settings.last_applied_wallpaper, None);
    assert_eq!(settings.terminal_app, None);
    assert!(settings.extra.is_empty(), "{:?}", settings.extra);
}

#[test]
fn unknown_settings_are_kept_when_the_file_is_rewritten() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(AppPaths::new(dir.path()));
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{ "welcomeSeen": true, "futureKey": { "a": 1 }, "applyDefaults": { "fit": "fit", "newAspect": false } }"#,
    )
    .unwrap();

    store.update(|s| s.last_applied_slug = Some("tokyo".into())).unwrap();

    let written: Value = crate::json::read_json(&dir.path().join("settings.json")).unwrap();
    assert_eq!(written["futureKey"]["a"], 1);
    assert_eq!(written["applyDefaults"]["newAspect"], false);
    assert_eq!(written["applyDefaults"]["fit"], "fit");
    assert_eq!(written["lastAppliedSlug"], "tokyo");
    assert_eq!(written["welcomeSeen"], true);
}

#[test]
fn settings_round_trip_and_default_when_missing_or_corrupt() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::new(AppPaths::new(dir.path()));
    assert!(!store.load().welcome_seen);

    store
        .update(|s| {
            s.welcome_seen = true;
            s.apply_defaults = ApplyOptions::new(true, true, false, WallpaperFit::Center);
        })
        .unwrap();
    let loaded = store.load();
    assert!(loaded.welcome_seen);
    assert!(!loaded.apply_defaults.accent_color);
    assert!(loaded.apply_defaults.wallpaper);
    assert_eq!(loaded.apply_defaults.fit, WallpaperFit::Center);

    std::fs::write(dir.path().join("settings.json"), "{ not json").unwrap();
    assert_eq!(store.load(), AppSettings::default());
}

#[test]
fn settings_with_missing_keys_or_an_unknown_fit_use_defaults() {
    let span: AppSettings =
        serde_json::from_str(r#"{ "welcomeSeen": true, "applyDefaults": { "fit": "span" } }"#).unwrap();
    assert!(span.welcome_seen);
    assert_eq!(span.apply_defaults, ApplyOptions::new(true, true, true, WallpaperFit::Span));

    let unknown: AppSettings = serde_json::from_str(r#"{ "applyDefaults": { "fit": "zoom" } }"#).unwrap();
    assert_eq!(unknown.apply_defaults.fit, WallpaperFit::Fill);
}

#[test]
fn an_in_memory_store_never_writes() {
    let dir = tempfile::tempdir().unwrap();
    let store = SettingsStore::in_memory(AppPaths::new(dir.path()));

    store.update(|s| s.last_applied_slug = Some("tokyo".into())).unwrap();

    assert_eq!(store.load().last_applied_slug.as_deref(), Some("tokyo"));
    assert!(!dir.path().join("settings.json").exists());
}

#[test]
fn snapshot_envelope_is_shared() {
    let snapshot = DesktopSnapshot {
        taken_at: Utc.timestamp_millis_opt(1_790_499_600_250).unwrap(),
        values: [("screens", "1"), ("screen.url.1", "/Users/me/Pictures/beach.jpg")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    };

    assert_same_json("data/original-desktop.json", &snapshot);
    assert_eq!(read::<DesktopSnapshot>("data/original-desktop.json"), snapshot);
}

/// Mirrors the apps' `AfterApplyTests`.
fn result(outcomes: &[StepOutcome]) -> ApplyResult {
    let steps = [ApplyStep::Wallpaper, ApplyStep::AppearanceMode, ApplyStep::AccentColor];
    ApplyResult { steps: steps.iter().zip(outcomes).map(|(s, o)| StepResult::new(*s, *o)).collect() }
}

#[test]
fn applying_records_the_theme_and_the_wallpaper_that_was_set() {
    let after = AppSettings::default().after_apply("tokyo", Some("2.png"), &result(&[StepOutcome::Applied]));

    assert_eq!(after.last_applied_slug.as_deref(), Some("tokyo"));
    assert_eq!(after.last_applied_wallpaper.as_deref(), Some("2.png"));
}

#[test]
fn applying_without_the_wallpaper_does_not_claim_its_wallpaper_is_on_the_desktop() {
    let before = AppSettings {
        last_applied_slug: Some("tokyo".into()),
        last_applied_wallpaper: Some("2.png".into()),
        ..Default::default()
    };

    // Same theme, wallpaper turned off: the desktop still shows 2.png.
    let same = before.after_apply("tokyo", Some("1.png"), &result(&[StepOutcome::SkippedByUser, StepOutcome::Applied]));
    assert_eq!(same.last_applied_wallpaper.as_deref(), Some("2.png"));

    // Another theme, wallpaper off or failed: none of its wallpapers is on the desktop.
    let other = before.after_apply("snow", Some("1.png"), &result(&[StepOutcome::Failed, StepOutcome::Applied]));
    assert_eq!(other.last_applied_slug.as_deref(), Some("snow"));
    assert_eq!(other.last_applied_wallpaper, None);
}

#[test]
fn applying_nothing_changes_nothing() {
    let before = AppSettings {
        last_applied_slug: Some("tokyo".into()),
        last_applied_wallpaper: Some("2.png".into()),
        ..Default::default()
    };

    assert_eq!(before.after_apply("snow", Some("1.png"), &result(&[StepOutcome::Failed, StepOutcome::Failed])), before);
}
