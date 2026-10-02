use std::path::{Path, PathBuf};

use anyhow::Result;

use super::models::{DesktopCapabilities, DesktopSnapshot, WallpaperFit};
use crate::color::RgbColor;
use crate::palette::AppearanceMode;

/// One display's picture, as the OS reports it (read-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayWallpaper {
    pub display: String,
    pub picture: Option<PathBuf>,
}

/// The OS-specific side of theming. Implementations call the platform's real APIs (macOS:
/// NSWorkspace; Windows: IDesktopWallpaper and the Personalize/DWM registry values). Methods for
/// capabilities the backend doesn't report are never called.
pub trait DesktopBackend: Send + Sync {
    fn capabilities(&self) -> DesktopCapabilities;

    /// The fits `apply --fit` accepts, in display order.
    fn supported_fits(&self) -> Vec<WallpaperFit>;

    /// Added to the summary after the accent color is applied, for what the user should know about
    /// how this OS picks it up. None for nothing.
    fn accent_color_note(&self) -> Option<&'static str> {
        None
    }

    fn capture(&self) -> Result<DesktopSnapshot>;

    fn restore(&self, snapshot: &DesktopSnapshot) -> Result<()>;

    /// Sets `image` (a local file) as the wallpaper on every display. `fill_color` shows around
    /// images that don't cover the screen (Fit, Center), where the OS supports it.
    fn set_wallpaper(&self, image: &Path, fit: WallpaperFit, fill_color: Option<RgbColor>) -> Result<()>;

    fn set_appearance_mode(&self, mode: AppearanceMode) -> Result<()>;

    fn set_accent_color(&self, accent: RgbColor) -> Result<()>;

    /// The wallpaper on each display right now (read-only), for `current`.
    fn current_wallpapers(&self) -> Vec<DisplayWallpaper> {
        Vec::new()
    }
}
