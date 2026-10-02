//! Applying a theme to the desktop: the backend abstraction, the snapshot taken before the first
//! apply, `ThemeApplier` and the summary wording.

pub mod accent_math;
mod applier;
mod backend;
mod dry_run;
mod models;
mod snapshot;
mod summary;

use std::sync::Arc;

pub use applier::{MissingWallpaperError, ThemeApplier};
pub use backend::{DesktopBackend, DisplayWallpaper};
pub use dry_run::DryRunBackend;
pub use models::{
    ApplyOptions, ApplyRequest, ApplyResult, ApplyStep, DesktopCapabilities, DesktopSnapshot, StepOutcome, StepResult,
    WallpaperFit,
};
pub use snapshot::{
    FileSnapshotStore, MemorySnapshotStore, OverlaySnapshotStore, SnapshotStore, SnapshotUnreadableError,
};
pub use summary::{ApplySummary, SummaryKind, join_list, label};

impl<T: DesktopBackend + ?Sized> DesktopBackend for Arc<T> {
    fn capabilities(&self) -> DesktopCapabilities {
        (**self).capabilities()
    }
    fn supported_fits(&self) -> Vec<WallpaperFit> {
        (**self).supported_fits()
    }
    fn accent_color_note(&self) -> Option<&'static str> {
        (**self).accent_color_note()
    }
    fn capture(&self) -> anyhow::Result<DesktopSnapshot> {
        (**self).capture()
    }
    fn restore(&self, snapshot: &DesktopSnapshot) -> anyhow::Result<()> {
        (**self).restore(snapshot)
    }
    fn set_wallpaper(
        &self,
        image: &std::path::Path,
        fit: WallpaperFit,
        fill_color: Option<crate::color::RgbColor>,
    ) -> anyhow::Result<()> {
        (**self).set_wallpaper(image, fit, fill_color)
    }
    fn set_appearance_mode(&self, mode: crate::palette::AppearanceMode) -> anyhow::Result<()> {
        (**self).set_appearance_mode(mode)
    }
    fn set_accent_color(&self, accent: crate::color::RgbColor) -> anyhow::Result<()> {
        (**self).set_accent_color(accent)
    }
    fn current_wallpapers(&self) -> Vec<DisplayWallpaper> {
        (**self).current_wallpapers()
    }
}

impl<T: SnapshotStore + ?Sized> SnapshotStore for Arc<T> {
    fn load(&self) -> anyhow::Result<Option<DesktopSnapshot>> {
        (**self).load()
    }
    fn save(&self, snapshot: &DesktopSnapshot) -> anyhow::Result<()> {
        (**self).save(snapshot)
    }
    fn clear(&self) -> anyhow::Result<()> {
        (**self).clear()
    }
}
