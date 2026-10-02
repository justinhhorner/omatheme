use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;
use chrono::Utc;

use super::backend::{DesktopBackend, DisplayWallpaper};
use super::models::{DesktopCapabilities, DesktopSnapshot, WallpaperFit};
use crate::color::RgbColor;
use crate::palette::AppearanceMode;

/// Used for `--dry-run` / `OMARCHY_THEMES_DRY_RUN=1`: reports what the platform's backend does
/// (capabilities, fits, the accent note) but changes nothing, so apply and restore can be exercised
/// end to end. It records what it would have done, for the output.
pub struct DryRunBackend {
    capabilities: DesktopCapabilities,
    fits: Vec<WallpaperFit>,
    accent_note: Option<&'static str>,
    actions: Mutex<Vec<String>>,
}

impl DryRunBackend {
    pub fn new(capabilities: DesktopCapabilities, fits: Vec<WallpaperFit>, accent_note: Option<&'static str>) -> Self {
        DryRunBackend { capabilities, fits, accent_note, actions: Mutex::new(Vec::new()) }
    }

    /// What a real run would have changed, in order.
    pub fn actions(&self) -> Vec<String> {
        self.actions.lock().unwrap().clone()
    }

    fn record(&self, action: String) {
        self.actions.lock().unwrap().push(action);
    }
}

impl DesktopBackend for DryRunBackend {
    fn capabilities(&self) -> DesktopCapabilities {
        self.capabilities
    }

    fn supported_fits(&self) -> Vec<WallpaperFit> {
        self.fits.clone()
    }

    fn accent_color_note(&self) -> Option<&'static str> {
        self.accent_note
    }

    fn capture(&self) -> Result<DesktopSnapshot> {
        self.record("save the current desktop".into());
        Ok(DesktopSnapshot { taken_at: Utc::now(), values: [("dryRun".to_string(), "1".to_string())].into() })
    }

    fn restore(&self, _snapshot: &DesktopSnapshot) -> Result<()> {
        self.record("restore the saved desktop".into());
        Ok(())
    }

    fn set_wallpaper(&self, image: &Path, fit: WallpaperFit, fill_color: Option<RgbColor>) -> Result<()> {
        let fill =
            fill_color.filter(|_| fit.shows_fill_color()).map(|c| format!(", {c} around it")).unwrap_or_default();
        self.record(format!("set the wallpaper to {} ({}{fill})", image.display(), fit.display_name()));
        Ok(())
    }

    fn set_appearance_mode(&self, mode: AppearanceMode) -> Result<()> {
        self.record(format!("switch to {mode} mode"));
        Ok(())
    }

    fn set_accent_color(&self, accent: RgbColor) -> Result<()> {
        self.record(format!("set the accent color to {accent}"));
        Ok(())
    }

    fn current_wallpapers(&self) -> Vec<DisplayWallpaper> {
        Vec::new()
    }
}
