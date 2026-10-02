use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::json;
use crate::paths::AppPaths;
use crate::theming::{ApplyOptions, ApplyResult, ApplyStep, StepOutcome};

/// settings.json, shared with the app. Keys this tool doesn't know are kept when it's rewritten.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// The app's welcome screen was dismissed.
    #[serde(default)]
    pub welcome_seen: bool,
    /// One-click apply choices (and the starting point for `apply`).
    #[serde(default)]
    pub apply_defaults: ApplyOptions,
    /// The theme on the desktop (the last one applied). Cleared by restore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_slug: Option<String>,
    /// Which of its wallpapers is on the desktop; only set when the wallpaper step applied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_applied_wallpaper: Option<String>,
    /// The terminal the macOS app sends colors to ("iterm2", "ghostty", "terminal"); None means the
    /// platform default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_app: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl AppSettings {
    /// The settings after applying `slug`. The theme becomes current if any step applied; its
    /// wallpaper only if the wallpaper step itself applied (with the wallpaper turned off, or
    /// failing, the desktop keeps showing whatever it showed before).
    pub fn after_apply(&self, slug: &str, wallpaper_file: Option<&str>, result: &ApplyResult) -> AppSettings {
        if !result.any_applied() {
            return self.clone();
        }
        let wallpaper_applied =
            result.result_for(ApplyStep::Wallpaper).is_some_and(|r| r.outcome == StepOutcome::Applied);
        let mut updated = self.clone();
        updated.last_applied_wallpaper = if wallpaper_applied {
            wallpaper_file.map(str::to_string)
        } else if self.last_applied_slug.as_deref() == Some(slug) {
            self.last_applied_wallpaper.clone()
        } else {
            None
        };
        updated.last_applied_slug = Some(slug.to_string());
        updated
    }
}

/// Where settings live. A store that doesn't persist (a dry run against the real data folder)
/// reads the file but keeps changes in memory.
pub struct SettingsStore {
    paths: AppPaths,
    persist: bool,
    memory: std::sync::Mutex<Option<AppSettings>>,
}

impl SettingsStore {
    pub fn new(paths: AppPaths) -> Self {
        SettingsStore { paths, persist: true, memory: Default::default() }
    }

    pub fn in_memory(paths: AppPaths) -> Self {
        SettingsStore { paths, persist: false, memory: Default::default() }
    }

    /// The saved settings; defaults if the file is missing or unreadable.
    pub fn load(&self) -> AppSettings {
        if let Some(settings) = self.memory.lock().unwrap().clone() {
            return settings;
        }
        json::read_json(&self.paths.settings_file()).unwrap_or_default()
    }

    pub fn save(&self, settings: &AppSettings) -> Result<()> {
        if self.persist {
            json::write_json(&self.paths.settings_file(), settings)
        } else {
            *self.memory.lock().unwrap() = Some(settings.clone());
            Ok(())
        }
    }

    pub fn update(&self, change: impl FnOnce(&mut AppSettings)) -> Result<AppSettings> {
        let mut settings = self.load();
        change(&mut settings);
        self.save(&settings)?;
        Ok(settings)
    }
}
