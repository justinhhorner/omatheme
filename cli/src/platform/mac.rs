//! The macOS backend's logic, behind a [`WallpaperApi`] so it's tested (on any OS) without
//! changing the desktop. Only the wallpaper can be changed: macOS has no public API for an app to
//! set dark mode or the accent color, so those are reported as unsupported. The snapshot keys
//! (`screens`, `screen.url.<id>`, `screen.options.<id>`, `screen.copy.<id>`) are the macOS app's,
//! so either can restore a snapshot the other took.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::image_convert::{ConvertedLayout, ImageConverter};
use crate::color::RgbColor;
use crate::json;
use crate::net::Clock;
use crate::palette::AppearanceMode;
use crate::theming::{DesktopBackend, DesktopCapabilities, DesktopSnapshot, DisplayWallpaper, WallpaperFit};

/// `NSImageScaling` raw values.
pub const SCALE_AXES_INDEPENDENTLY: u64 = 1;
pub const SCALE_NONE: u64 = 2;
pub const SCALE_PROPORTIONALLY_UP_OR_DOWN: u64 = 3;

/// Desktop picture options, mirroring `NSWorkspace.DesktopImageOptionKey`. None means the key was
/// absent (the system default).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scaling: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_clipping: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<RgbColor>,
}

impl WallpaperOptions {
    /// Tile and Span have no NSWorkspace equivalent and fall back to Fill.
    pub fn for_fit(fit: WallpaperFit, fill_color: Option<RgbColor>) -> Self {
        let (scaling, allow_clipping) = match fit {
            WallpaperFit::Fill | WallpaperFit::Tile | WallpaperFit::Span => (SCALE_PROPORTIONALLY_UP_OR_DOWN, true),
            WallpaperFit::Fit => (SCALE_PROPORTIONALLY_UP_OR_DOWN, false),
            WallpaperFit::Stretch => (SCALE_AXES_INDEPENDENTLY, true),
            WallpaperFit::Center => (SCALE_NONE, true),
        };
        WallpaperOptions { scaling: Some(scaling), allow_clipping: Some(allow_clipping), fill_color }
    }
}

/// One screen's desktop picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenWallpaper {
    /// The display's `NSScreenNumber` (CGDirectDisplayID), stable while it stays connected.
    pub screen_id: String,
    pub image: Option<PathBuf>,
    pub options: WallpaperOptions,
}

/// The NSWorkspace desktop-picture calls. They change the current Space on each screen.
pub trait WallpaperApi: Send + Sync {
    /// Every connected screen, main screen first.
    fn current_wallpapers(&self) -> Vec<ScreenWallpaper>;
    fn set_wallpaper(&self, image: &Path, options: &WallpaperOptions, screen_id: &str) -> Result<()>;
}

#[derive(Debug, thiserror::Error)]
#[error("No displays were found.")]
pub struct NoScreensError;

/// Some displays got the new wallpaper and some didn't.
#[derive(Debug, thiserror::Error)]
#[error("It was set on {changed} of {total} displays. {underlying}")]
pub struct PartialWallpaperError {
    pub changed: usize,
    pub total: usize,
    pub underlying: String,
}

#[derive(Debug, thiserror::Error)]
#[error("macOS doesn't let apps change this setting.")]
pub struct UnsupportedOnMacError;

const SCREENS_KEY: &str = "screens";
const URL_PREFIX: &str = "screen.url.";
const OPTIONS_PREFIX: &str = "screen.options.";
const COPY_PREFIX: &str = "screen.copy.";

/// Folders the OS owns; their pictures are always there, so they aren't copied.
const SYSTEM_PICTURE_FOLDERS: [&str; 2] = ["/System/", "/Library/Desktop Pictures/"];

pub const CAPABILITIES: DesktopCapabilities = DesktopCapabilities::WALLPAPER;
pub const SUPPORTED_FITS: [WallpaperFit; 4] =
    [WallpaperFit::Fill, WallpaperFit::Fit, WallpaperFit::Stretch, WallpaperFit::Center];

pub struct MacDesktopBackend {
    api: Box<dyn WallpaperApi>,
    converter: ImageConverter,
    snapshot_assets_dir: PathBuf,
    now: Clock,
}

impl MacDesktopBackend {
    pub fn new(api: Box<dyn WallpaperApi>, snapshot_assets_dir: PathBuf, now: Clock) -> Self {
        MacDesktopBackend { api, converter: ImageConverter::new(ConvertedLayout::Mac), snapshot_assets_dir, now }
    }

    fn needs_copy(path: &Path) -> bool {
        path.is_file() && !SYSTEM_PICTURE_FOLDERS.iter().any(|folder| path.to_string_lossy().starts_with(folder))
    }

    fn capture_copies(
        &self,
        screens: &[ScreenWallpaper],
        staging: &Path,
        values: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let mut copies: HashMap<PathBuf, PathBuf> = HashMap::new();
        for (index, screen) in screens.iter().enumerate() {
            let id = &screen.screen_id;
            values.insert(
                format!("{URL_PREFIX}{id}"),
                screen.image.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
            );
            values.insert(format!("{OPTIONS_PREFIX}{id}"), serde_json::to_string(&screen.options)?);

            let Some(image) = screen.image.as_ref().filter(|p| Self::needs_copy(p)) else {
                continue;
            };
            let copy = match copies.get(image) {
                Some(existing) => existing.clone(),
                None => {
                    let ext = image.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                    let file_name = format!("{index}{ext}");
                    std::fs::create_dir_all(staging)?;
                    std::fs::copy(image, staging.join(&file_name))?;
                    let copy = self.snapshot_assets_dir.join(&file_name);
                    copies.insert(image.clone(), copy.clone());
                    copy
                }
            };
            values.insert(format!("{COPY_PREFIX}{id}"), copy.to_string_lossy().into_owned());
        }
        json::remove_dir_if_present(&self.snapshot_assets_dir)?;
        if staging.exists() {
            std::fs::rename(staging, &self.snapshot_assets_dir)?;
        }
        Ok(())
    }
}

impl DesktopBackend for MacDesktopBackend {
    fn capabilities(&self) -> DesktopCapabilities {
        CAPABILITIES
    }

    fn supported_fits(&self) -> Vec<WallpaperFit> {
        SUPPORTED_FITS.to_vec()
    }

    fn capture(&self) -> Result<DesktopSnapshot> {
        let screens = self.api.current_wallpapers();
        if screens.is_empty() {
            return Err(NoScreensError.into());
        }
        let ids: Vec<&str> = screens.iter().map(|s| s.screen_id.as_str()).collect();
        let mut values = BTreeMap::from([(SCREENS_KEY.to_string(), ids.join(","))]);

        // Keep a private copy of each original wallpaper: the user may delete or move the file
        // later, and restore should still work. The copies go to a staging folder that replaces the
        // previous ones only once they're all made, so a failed capture doesn't lose them.
        let mut staging = self.snapshot_assets_dir.clone().into_os_string();
        staging.push(".new");
        let staging = PathBuf::from(staging);
        json::remove_dir_if_present(&staging)?;
        if let Err(error) = self.capture_copies(&screens, &staging, &mut values) {
            let _ = json::remove_dir_if_present(&staging);
            return Err(error);
        }
        Ok(DesktopSnapshot { taken_at: (self.now)(), values })
    }

    fn restore(&self, snapshot: &DesktopSnapshot) -> Result<()> {
        let recorded: Vec<&str> = snapshot
            .values
            .get(SCREENS_KEY)
            .map(|s| s.split(',').filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        let Some(fallback_id) = recorded.first() else {
            return Ok(());
        };

        // Failing keeps the snapshot (the applier only forgets it after a successful restore), so
        // with no display connected right now the original desktop isn't lost.
        let current = self.api.current_wallpapers();
        if current.is_empty() {
            return Err(NoScreensError.into());
        }

        let mut attempted = 0;
        let mut failures = Vec::new();
        for screen in &current {
            // A display connected after the snapshot gets the main screen's original picture.
            let id =
                if recorded.contains(&screen.screen_id.as_str()) { screen.screen_id.as_str() } else { fallback_id };
            let Some(path) = snapshot.values.get(&format!("{URL_PREFIX}{id}")).filter(|p| !p.is_empty()) else {
                continue;
            };
            let mut image = PathBuf::from(path);
            if !image.exists()
                && let Some(copy) = snapshot.values.get(&format!("{COPY_PREFIX}{id}")).map(PathBuf::from)
                && copy.exists()
            {
                image = copy;
            }
            let options = snapshot
                .values
                .get(&format!("{OPTIONS_PREFIX}{id}"))
                .and_then(|o| serde_json::from_str(o).ok())
                .unwrap_or_default();

            attempted += 1;
            if let Err(error) = self.api.set_wallpaper(&image, &options, &screen.screen_id) {
                failures.push(error);
            }
        }
        // Fail only if nothing could be put back (displays without a saved picture are skipped).
        if !failures.is_empty() && failures.len() == attempted {
            return Err(failures.remove(0));
        }
        Ok(())
    }

    /// Sets the wallpaper on every display, carrying on past a display that fails so the others
    /// still change; the error then says how many did.
    fn set_wallpaper(&self, image: &Path, fit: WallpaperFit, fill_color: Option<RgbColor>) -> Result<()> {
        let usable = self.converter.ensure_supported_format(image)?;
        let options = WallpaperOptions::for_fit(fit, fill_color);
        let screens = self.api.current_wallpapers();
        if screens.is_empty() {
            return Err(NoScreensError.into());
        }
        let mut failures: Vec<anyhow::Error> = Vec::new();
        for screen in &screens {
            if let Err(error) = self.api.set_wallpaper(&usable, &options, &screen.screen_id) {
                failures.push(error);
            }
        }
        match failures.len() {
            0 => Ok(()),
            n if n == screens.len() => Err(failures.remove(0)),
            n => Err(PartialWallpaperError {
                changed: screens.len() - n,
                total: screens.len(),
                underlying: failures[0].to_string(),
            }
            .into()),
        }
    }

    fn set_appearance_mode(&self, _mode: AppearanceMode) -> Result<()> {
        Err(UnsupportedOnMacError.into())
    }

    fn set_accent_color(&self, _accent: RgbColor) -> Result<()> {
        Err(UnsupportedOnMacError.into())
    }

    fn current_wallpapers(&self) -> Vec<DisplayWallpaper> {
        self.api
            .current_wallpapers()
            .into_iter()
            .enumerate()
            .map(|(i, s)| DisplayWallpaper {
                display: if i == 0 {
                    format!("Display {} (main)", s.screen_id)
                } else {
                    format!("Display {}", s.screen_id)
                },
                picture: s.image,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::cancel::CancelToken;
    use crate::color::c;
    use crate::test_support::ManualClock;
    use crate::theming::{ApplyOptions, ApplyRequest, ApplyStep, MemorySnapshotStore, StepOutcome, ThemeApplier};

    /// In-memory screens; records every set call instead of changing the real desktop.
    #[derive(Default)]
    struct FakeWallpaperApi {
        screens: Mutex<Vec<ScreenWallpaper>>,
        calls: Mutex<Vec<(PathBuf, WallpaperOptions, String)>>,
        failing: Mutex<Vec<String>>,
    }

    impl FakeWallpaperApi {
        fn calls(&self) -> Vec<(PathBuf, WallpaperOptions, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl WallpaperApi for Arc<FakeWallpaperApi> {
        fn current_wallpapers(&self) -> Vec<ScreenWallpaper> {
            self.screens.lock().unwrap().clone()
        }

        fn set_wallpaper(&self, image: &Path, options: &WallpaperOptions, screen_id: &str) -> Result<()> {
            if self.failing.lock().unwrap().iter().any(|s| s == screen_id) {
                anyhow::bail!("That display is no longer connected.");
            }
            self.calls.lock().unwrap().push((image.to_path_buf(), options.clone(), screen_id.to_string()));
            let mut screens = self.screens.lock().unwrap();
            if let Some(screen) = screens.iter_mut().find(|s| s.screen_id == screen_id) {
                screen.image = Some(image.to_path_buf());
                screen.options = options.clone();
            }
            Ok(())
        }
    }

    struct Harness {
        temp: tempfile::TempDir,
        original: PathBuf,
        api: Arc<FakeWallpaperApi>,
        backend: MacDesktopBackend,
    }

    fn harness() -> Harness {
        let temp = tempfile::tempdir().unwrap();
        let original = temp.path().join("Pictures/beach.jpg");
        std::fs::create_dir_all(original.parent().unwrap()).unwrap();
        std::fs::write(&original, [1, 2, 3]).unwrap();
        let api = Arc::new(FakeWallpaperApi::default());
        *api.screens.lock().unwrap() = vec![
            ScreenWallpaper {
                screen_id: "1".into(),
                image: Some(original.clone()),
                options: WallpaperOptions { scaling: Some(3), allow_clipping: Some(true), fill_color: None },
            },
            ScreenWallpaper {
                screen_id: "2".into(),
                image: Some(original.clone()),
                options: WallpaperOptions {
                    scaling: Some(1),
                    allow_clipping: Some(false),
                    fill_color: Some(c("#102030")),
                },
            },
        ];
        let backend = MacDesktopBackend::new(
            Box::new(api.clone()),
            temp.path().join("original-desktop"),
            ManualClock::at(1_790_380_800).function(),
        );
        Harness { temp, original, api, backend }
    }

    impl Harness {
        fn copies_dir(&self) -> PathBuf {
            self.temp.path().join("original-desktop")
        }
    }

    #[test]
    fn reports_only_wallpaper_and_the_fits_nsworkspace_supports() {
        let h = harness();
        assert_eq!(h.backend.capabilities(), DesktopCapabilities::WALLPAPER);
        assert_eq!(h.backend.supported_fits(), SUPPORTED_FITS);
    }

    #[test]
    fn fits_map_to_nsworkspace_options() {
        let black = Some(c("#000000"));
        assert_eq!(
            WallpaperOptions::for_fit(WallpaperFit::Fill, black),
            WallpaperOptions { scaling: Some(3), allow_clipping: Some(true), fill_color: black }
        );
        assert_eq!(
            WallpaperOptions::for_fit(WallpaperFit::Fit, None),
            WallpaperOptions { scaling: Some(3), allow_clipping: Some(false), fill_color: None }
        );
        assert_eq!(WallpaperOptions::for_fit(WallpaperFit::Stretch, None).scaling, Some(SCALE_AXES_INDEPENDENTLY));
        assert_eq!(WallpaperOptions::for_fit(WallpaperFit::Center, None).scaling, Some(SCALE_NONE));
        assert_eq!(
            WallpaperOptions::for_fit(WallpaperFit::Tile, None),
            WallpaperOptions::for_fit(WallpaperFit::Fill, None)
        );
    }

    #[test]
    fn options_read_the_mac_apps_snapshot_json() {
        // The macOS app writes them pretty-printed with sorted keys.
        let written = "{\n  \"allowClipping\" : false,\n  \"fillColor\" : \"#102030\",\n  \"scaling\" : 1\n}";
        let options: WallpaperOptions = serde_json::from_str(written).unwrap();
        assert_eq!(
            options,
            WallpaperOptions { scaling: Some(1), allow_clipping: Some(false), fill_color: Some(c("#102030")) }
        );
        assert_eq!(serde_json::to_string(&WallpaperOptions::default()).unwrap(), "{}");
    }

    #[test]
    fn capture_records_every_screen_and_copies_each_original_once() {
        let h = harness();
        let snapshot = h.backend.capture().unwrap();

        assert_eq!(snapshot.values["screens"], "1,2");
        assert_eq!(snapshot.values["screen.url.1"], h.original.to_string_lossy());
        let copy = &snapshot.values["screen.copy.1"];
        assert_eq!(&snapshot.values["screen.copy.2"], copy);
        assert_eq!(std::fs::read(copy).unwrap(), [1, 2, 3]);
        let copies: Vec<_> = std::fs::read_dir(h.copies_dir()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(copies, ["0.jpg"]);
        let options: WallpaperOptions = serde_json::from_str(&snapshot.values["screen.options.2"]).unwrap();
        assert_eq!(options.fill_color, Some(c("#102030")));
    }

    #[test]
    fn system_pictures_are_not_copied() {
        let h = harness();
        h.api.screens.lock().unwrap()[0].image = Some(PathBuf::from("/System/Library/Desktop Pictures/Example.heic"));
        h.api.screens.lock().unwrap().truncate(1);

        let snapshot = h.backend.capture().unwrap();

        assert!(!snapshot.values.contains_key("screen.copy.1"));
        assert!(!h.copies_dir().exists());
    }

    /// Mirrors the apps' "a failed capture keeps the previous wallpaper copies".
    #[cfg(unix)]
    #[test]
    fn a_failed_capture_keeps_the_previous_wallpaper_copies() {
        use std::os::unix::fs::PermissionsExt;
        let h = harness();
        std::fs::create_dir_all(h.copies_dir()).unwrap();
        std::fs::write(h.copies_dir().join("0.jpg"), [9]).unwrap();
        // The original can't be read, so copying it fails.
        std::fs::set_permissions(&h.original, std::fs::Permissions::from_mode(0o000)).unwrap();

        let result = h.backend.capture();
        std::fs::set_permissions(&h.original, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert!(result.is_err());
        assert_eq!(std::fs::read(h.copies_dir().join("0.jpg")).unwrap(), [9]);
        assert!(!h.temp.path().join("original-desktop.new").exists());
    }

    #[test]
    fn capture_fails_without_screens() {
        let h = harness();
        h.api.screens.lock().unwrap().clear();
        assert!(h.backend.capture().err().unwrap().is::<NoScreensError>());
    }

    #[test]
    fn setting_a_wallpaper_sets_every_screen_with_the_fit_and_fill_color() {
        let h = harness();
        let wall = h.temp.path().join("wall.png");
        std::fs::write(&wall, [1]).unwrap();

        h.backend.set_wallpaper(&wall, WallpaperFit::Center, Some(c("#1a1b26"))).unwrap();

        let calls = h.api.calls();
        assert_eq!(calls.len(), 2);
        assert!(
            calls.iter().all(
                |(p, o, _)| *p == wall && *o == WallpaperOptions::for_fit(WallpaperFit::Center, Some(c("#1a1b26")))
            )
        );
    }

    #[test]
    fn webp_is_converted_before_setting() {
        let h = harness();
        let webp = h.temp.path().join("theme/1.webp");
        std::fs::create_dir_all(webp.parent().unwrap()).unwrap();
        image::DynamicImage::ImageRgb8(image::RgbImage::new(2, 2))
            .save_with_format(&webp, image::ImageFormat::WebP)
            .unwrap();

        h.backend.set_wallpaper(&webp, WallpaperFit::Fill, None).unwrap();

        assert_eq!(h.api.calls()[0].0, h.temp.path().join("theme/.converted/1.webp.png"));
    }

    #[test]
    fn a_failing_display_does_not_stop_the_others() {
        let h = harness();
        let wall = h.temp.path().join("wall.png");
        std::fs::write(&wall, [1]).unwrap();
        h.api.failing.lock().unwrap().push("2".into());

        let error = h.backend.set_wallpaper(&wall, WallpaperFit::Fill, None).err().unwrap();

        assert_eq!(h.api.calls().len(), 1);
        let partial = error.downcast_ref::<PartialWallpaperError>().unwrap();
        assert_eq!((partial.changed, partial.total), (1, 2));
        assert!(error.to_string().starts_with("It was set on 1 of 2 displays."));
    }

    #[test]
    fn restore_puts_back_each_screens_picture_and_options() {
        let h = harness();
        let snapshot = h.backend.capture().unwrap();
        let wall = h.temp.path().join("wall.png");
        std::fs::write(&wall, [1]).unwrap();
        h.backend.set_wallpaper(&wall, WallpaperFit::Fill, None).unwrap();

        h.backend.restore(&snapshot).unwrap();

        let screens = h.api.current_wallpapers();
        assert_eq!(screens[0].image.as_ref(), Some(&h.original));
        assert_eq!(screens[1].options.fill_color, Some(c("#102030")));
        assert_eq!(screens[1].options.scaling, Some(1));
    }

    #[test]
    fn restore_falls_back_to_the_private_copy_when_the_original_is_gone() {
        let h = harness();
        let snapshot = h.backend.capture().unwrap();
        std::fs::remove_file(&h.original).unwrap();

        h.backend.restore(&snapshot).unwrap();

        assert_eq!(h.api.calls()[0].0, h.copies_dir().join("0.jpg"));
    }

    #[test]
    fn a_display_connected_later_gets_the_main_displays_picture() {
        let h = harness();
        let snapshot = h.backend.capture().unwrap();
        h.api.screens.lock().unwrap().push(ScreenWallpaper {
            screen_id: "3".into(),
            image: None,
            options: Default::default(),
        });

        h.backend.restore(&snapshot).unwrap();

        let calls = h.api.calls();
        let third = calls.iter().find(|(_, _, id)| id == "3").unwrap();
        assert_eq!(third.0, h.original);
        assert_eq!(third.1.scaling, Some(3));
    }

    #[test]
    fn restore_fails_only_when_every_screen_it_tried_fails() {
        let h = harness();
        let snapshot = h.backend.capture().unwrap();
        h.api.failing.lock().unwrap().push("2".into());
        assert!(h.backend.restore(&snapshot).is_ok());

        h.api.failing.lock().unwrap().push("1".into());
        assert!(h.backend.restore(&snapshot).is_err());
    }

    #[test]
    fn restoring_with_no_display_connected_keeps_the_snapshot() {
        let h = harness();
        let applier = ThemeApplier::new(Box::new(h.backend), Box::new(MemorySnapshotStore::new(None)));
        let wall = h.temp.path().join("wall.png");
        std::fs::write(&wall, [1]).unwrap();
        let request = ApplyRequest {
            theme_name: "T".into(),
            wallpaper: Some(wall),
            mode: AppearanceMode::Dark,
            accent: Some(c("#7aa2f7")),
            background: None,
            options: ApplyOptions::default(),
        };
        applier.apply(&request, &mut |_| {}, &CancelToken::new()).unwrap();
        h.api.screens.lock().unwrap().clear();

        assert!(applier.restore_original().is_err());
        assert!(applier.has_original_snapshot());
    }

    #[test]
    fn through_the_applier_light_dark_and_accent_are_not_supported() {
        let h = harness();
        let wall = h.temp.path().join("wall.png");
        std::fs::write(&wall, [1]).unwrap();
        let applier = ThemeApplier::new(Box::new(h.backend), Box::new(MemorySnapshotStore::new(None)));
        let request = ApplyRequest {
            theme_name: "T".into(),
            wallpaper: Some(wall),
            mode: AppearanceMode::Light,
            accent: Some(c("#7aa2f7")),
            background: Some(c("#1a1b26")),
            options: ApplyOptions::default(),
        };

        let result = applier.apply(&request, &mut |_| {}, &CancelToken::new()).unwrap();

        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::Applied);
        assert_eq!(result.result_for(ApplyStep::AppearanceMode).unwrap().outcome, StepOutcome::NotSupported);
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::NotSupported);
    }

    #[test]
    fn restores_a_snapshot_the_mac_app_took() {
        let h = harness();
        let snapshot: DesktopSnapshot = serde_json::from_value(serde_json::json!({
            "takenAt": "2026-09-27T09:00:00.250Z",
            "values": {
                "screens": "1",
                "screen.url.1": h.original.to_string_lossy(),
                "screen.options.1": "{\n  \"allowClipping\" : true,\n  \"scaling\" : 3\n}"
            }
        }))
        .unwrap();

        h.backend.restore(&snapshot).unwrap();

        let calls = h.api.calls();
        assert_eq!(calls.len(), 2); // display 2 wasn't recorded: it gets the main display's picture
        assert!(calls.iter().all(|(p, o, _)| *p == h.original && o.scaling == Some(3)));
    }
}
