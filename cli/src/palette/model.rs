use std::fmt;

use serde::{Deserialize, Serialize};

use crate::color::RgbColor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceMode {
    Dark,
    Light,
}

impl fmt::Display for AppearanceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            AppearanceMode::Dark => "dark",
            AppearanceMode::Light => "light",
        })
    }
}

/// Which file in the theme repo the palette was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PaletteSource {
    ColorsToml,
    Alacritty,
}

impl PaletteSource {
    pub fn file_name(&self) -> &'static str {
        match self {
            PaletteSource::ColorsToml => "colors.toml",
            PaletteSource::Alacritty => "alacritty.toml",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NamedColor {
    pub name: String,
    pub color: RgbColor,
}

impl NamedColor {
    pub fn new(name: impl Into<String>, color: RgbColor) -> Self {
        NamedColor { name: name.into(), color }
    }
}

/// A theme's colors, normalized from whichever file format the theme ships.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Palette {
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub accent: RgbColor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<RgbColor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection: Option<RgbColor>,
    /// Named colors.toml `muted` (Omarchy uses it as the terminal's bright black).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub muted: Option<RgbColor>,
    /// Named colors.toml `bright_foreground` (the terminal's bright white and cursor).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bright_foreground: Option<RgbColor>,
    /// Mode stated by the theme itself (colors.toml `mode` or a light.mode file).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub declared_mode: Option<AppearanceMode>,
    /// Terminal-style colors for display, in a stable order.
    #[serde(default)]
    pub swatches: Vec<NamedColor>,
    pub source: PaletteSource,
}

impl Palette {
    pub fn new(background: RgbColor, foreground: RgbColor, accent: RgbColor, source: PaletteSource) -> Self {
        Palette {
            background,
            foreground,
            accent,
            cursor: None,
            selection: None,
            muted: None,
            bright_foreground: None,
            declared_mode: None,
            swatches: Vec::new(),
            source,
        }
    }

    /// The declared mode, otherwise inferred from the background's luminance.
    pub fn mode(&self) -> AppearanceMode {
        self.declared_mode.unwrap_or(if self.background.is_light() {
            AppearanceMode::Light
        } else {
            AppearanceMode::Dark
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct PaletteParseError(pub String);
