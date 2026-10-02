use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};

use crate::color::RgbColor;
use crate::json;
use crate::palette::AppearanceMode;
use crate::store::InstalledTheme;

/// What a platform backend can change. Add a member when adding a new theming API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DesktopCapabilities {
    pub wallpaper: bool,
    pub appearance_mode: bool,
    pub accent_color: bool,
}

impl DesktopCapabilities {
    pub const NONE: Self = DesktopCapabilities { wallpaper: false, appearance_mode: false, accent_color: false };
    pub const WALLPAPER: Self = DesktopCapabilities { wallpaper: true, appearance_mode: false, accent_color: false };
    pub const ALL: Self = DesktopCapabilities { wallpaper: true, appearance_mode: true, accent_color: true };
}

/// How a wallpaper is scaled. Tile and Span exist on Windows; a backend lists what it supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "camelCase")]
pub enum WallpaperFit {
    Fill,
    Fit,
    Stretch,
    Center,
    Tile,
    Span,
}

impl WallpaperFit {
    pub const ALL: [WallpaperFit; 6] = [Self::Fill, Self::Fit, Self::Stretch, Self::Center, Self::Tile, Self::Span];

    pub fn id(&self) -> &'static str {
        match self {
            WallpaperFit::Fill => "fill",
            WallpaperFit::Fit => "fit",
            WallpaperFit::Stretch => "stretch",
            WallpaperFit::Center => "center",
            WallpaperFit::Tile => "tile",
            WallpaperFit::Span => "span",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id().eq_ignore_ascii_case(text.trim()))
    }

    /// Whether the desktop shows around the picture (where the theme's background color goes).
    pub fn shows_fill_color(&self) -> bool {
        matches!(self, WallpaperFit::Fit | WallpaperFit::Center)
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            WallpaperFit::Fill => "Fill Screen",
            WallpaperFit::Fit => "Fit to Screen",
            WallpaperFit::Stretch => "Stretch to Fill Screen",
            WallpaperFit::Center => "Center",
            WallpaperFit::Tile => "Tile",
            WallpaperFit::Span => "Span",
        }
    }
}

impl fmt::Display for WallpaperFit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// The per-aspect choices for an apply (the apps' Apply dialog), saved as `applyDefaults` in
/// settings.json for one-click apply. Unknown keys are kept when the file is rewritten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOptions {
    #[serde(default = "yes")]
    pub wallpaper: bool,
    #[serde(default = "yes")]
    pub appearance_mode: bool,
    #[serde(default = "yes")]
    pub accent_color: bool,
    /// An unknown fit (from a newer client) falls back to Fill.
    #[serde(default = "fill", deserialize_with = "lenient_fit")]
    pub fit: WallpaperFit,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn yes() -> bool {
    true
}

fn fill() -> WallpaperFit {
    WallpaperFit::Fill
}

fn lenient_fit<'de, D: Deserializer<'de>>(d: D) -> Result<WallpaperFit, D::Error> {
    let value = serde_json::Value::deserialize(d)?;
    Ok(value.as_str().and_then(WallpaperFit::parse).unwrap_or(WallpaperFit::Fill))
}

impl Default for ApplyOptions {
    fn default() -> Self {
        ApplyOptions {
            wallpaper: true,
            appearance_mode: true,
            accent_color: true,
            fit: WallpaperFit::Fill,
            extra: Default::default(),
        }
    }
}

impl ApplyOptions {
    pub fn new(wallpaper: bool, appearance_mode: bool, accent_color: bool, fit: WallpaperFit) -> Self {
        ApplyOptions { wallpaper, appearance_mode, accent_color, fit, extra: Default::default() }
    }

    /// Only the wallpaper, as when switching the current theme's wallpaper.
    pub fn wallpaper_only(fit: WallpaperFit) -> Self {
        Self::new(true, false, false, fit)
    }
}

/// Opaque, backend-defined record of the desktop before it was changed. The envelope is shared by
/// every client; the backend decides what goes in `values`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSnapshot {
    #[serde(with = "json::date")]
    pub taken_at: DateTime<Utc>,
    pub values: BTreeMap<String, String>,
}

/// What to apply. Built from a downloaded theme, so no network is involved.
#[derive(Debug, Clone)]
pub struct ApplyRequest {
    pub theme_name: String,
    pub wallpaper: Option<PathBuf>,
    pub mode: AppearanceMode,
    pub accent: Option<RgbColor>,
    /// The theme's background, shown around wallpapers that don't cover the screen.
    pub background: Option<RgbColor>,
    pub options: ApplyOptions,
}

impl ApplyRequest {
    /// `wallpaper_file` is one of `theme.wallpapers`; anything else means the first.
    pub fn from_theme(theme: &InstalledTheme, wallpaper_file: Option<&str>, options: ApplyOptions) -> Self {
        let file = wallpaper_file
            .filter(|f| theme.wallpapers.iter().any(|w| w == f))
            .or(theme.wallpapers.first().map(String::as_str));
        ApplyRequest {
            theme_name: theme.name.clone(),
            wallpaper: file.map(|f| theme.wallpaper_path(f)),
            mode: theme.mode,
            accent: theme.palette.as_ref().map(|p| p.accent),
            background: theme.palette.as_ref().map(|p| p.background),
            options,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyStep {
    SaveOriginal,
    Wallpaper,
    AppearanceMode,
    AccentColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StepOutcome {
    Applied,
    /// The user turned it off.
    SkippedByUser,
    /// This OS/backend can't change it.
    NotSupported,
    /// The theme has nothing for it (e.g. no wallpaper or palette).
    NoData,
    /// An earlier step failed in a way that made continuing unsafe.
    NotAttempted,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StepResult {
    pub step: ApplyStep,
    pub outcome: StepOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl StepResult {
    pub fn new(step: ApplyStep, outcome: StepOutcome) -> Self {
        StepResult { step, outcome, error: None }
    }

    pub fn failed(step: ApplyStep, error: impl Into<String>) -> Self {
        StepResult { step, outcome: StepOutcome::Failed, error: Some(error.into()) }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ApplyResult {
    pub steps: Vec<StepResult>,
}

impl ApplyResult {
    pub fn any_applied(&self) -> bool {
        self.steps.iter().any(|s| s.outcome == StepOutcome::Applied)
    }

    pub fn result_for(&self, step: ApplyStep) -> Option<&StepResult> {
        self.steps.iter().find(|s| s.step == step)
    }
}
