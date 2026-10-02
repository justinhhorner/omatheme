//! The Windows backend's logic, behind small traits for the registry, the wallpaper API and the
//! settings broadcast, so it's tested (on any OS) without changing the machine. Everything is
//! per-user (HKCU) and needs no elevation:
//! - wallpaper: IDesktopWallpaper (SystemParametersInfo fallback);
//! - light/dark: Themes\Personalize AppsUseLightTheme + SystemUsesLightTheme;
//! - accent: DWM AccentColor/ColorizationColor + Explorer\Accent AccentPalette. Windows has no
//!   public API for the accent, so this mirrors what the Settings app stores.
//!
//! The snapshot keys (`reg:<key>|<name>`, `monitor:<id>`, `monitor-copy:<id>`, `wallpaper-fit`,
//! `wallpaper-background`) are the Windows app's, so either can restore a snapshot the other took.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use base64::Engine;

use crate::color::RgbColor;
use crate::json;
use crate::net::Clock;
use crate::palette::AppearanceMode;
use crate::theming::accent_math;
use crate::theming::{DesktopBackend, DesktopCapabilities, DesktopSnapshot, DisplayWallpaper, WallpaperFit};

pub const PERSONALIZE_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
pub const DWM_KEY: &str = r"Software\Microsoft\Windows\DWM";
pub const ACCENT_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
pub const DESKTOP_KEY: &str = r"Control Panel\Desktop";

/// Every registry value this backend may change, and therefore snapshots.
pub const TRACKED_VALUES: [(&str, &str); 9] = [
    (PERSONALIZE_KEY, "AppsUseLightTheme"),
    (PERSONALIZE_KEY, "SystemUsesLightTheme"),
    (DWM_KEY, "AccentColor"),
    (DWM_KEY, "ColorizationColor"),
    (DWM_KEY, "ColorizationAfterglow"),
    (ACCENT_KEY, "AccentPalette"),
    (ACCENT_KEY, "AccentColorMenu"),
    (ACCENT_KEY, "StartColorMenu"),
    (DESKTOP_KEY, "AutoColorization"),
];

pub const ACCENT_NOTE: &str = "Some parts of Windows may only pick up the new accent color after you sign out.";

/// The area Explorer and apps watch for light/dark and accent changes.
pub const IMMERSIVE_COLOR_SET: &str = "ImmersiveColorSet";

pub const CAPABILITIES: DesktopCapabilities = DesktopCapabilities::ALL;

const REG_PREFIX: &str = "reg:";
const MONITOR_PREFIX: &str = "monitor:";
const COPY_PREFIX: &str = "monitor-copy:";
const FIT_KEY: &str = "wallpaper-fit";
const BACKGROUND_KEY: &str = "wallpaper-background";

/// A typed registry value that round-trips through a snapshot string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegValue {
    DWord(u32),
    QWord(i64),
    Binary(Vec<u8>),
    ExpandString(String),
    String(String),
}

impl RegValue {
    /// "absent" for None; otherwise "kind:data", as the Windows app writes it.
    pub fn serialize(value: Option<&RegValue>) -> String {
        match value {
            None => "absent".into(),
            Some(RegValue::DWord(v)) => format!("dword:{v}"),
            Some(RegValue::QWord(v)) => format!("qword:{v}"),
            Some(RegValue::Binary(b)) => format!("binary:{}", base64::engine::general_purpose::STANDARD.encode(b)),
            Some(RegValue::ExpandString(s)) => format!("expand:{s}"),
            Some(RegValue::String(s)) => format!("string:{s}"),
        }
    }

    pub fn deserialize(text: &str) -> Result<Option<RegValue>> {
        if text == "absent" {
            return Ok(None);
        }
        let Some((kind, data)) = text.split_once(':') else { bail!("Invalid registry snapshot value '{text}'.") };
        Ok(Some(match kind {
            "dword" => RegValue::DWord(data.parse()?),
            "qword" => RegValue::QWord(data.parse()?),
            "binary" => RegValue::Binary(base64::engine::general_purpose::STANDARD.decode(data)?),
            "expand" => RegValue::ExpandString(data.to_string()),
            "string" => RegValue::String(data.to_string()),
            other => bail!("Unknown registry value kind '{other}'."),
        }))
    }
}

/// HKEY_CURRENT_USER access. Everything this backend changes is per-user; it never touches HKLM.
pub trait RegistryAccess: Send + Sync {
    fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>>;
    fn write(&self, key: &str, name: &str, value: &RegValue) -> Result<()>;
    fn delete(&self, key: &str, name: &str) -> Result<()>;
}

/// Tells running apps and the shell that a settings area changed (WM_SETTINGCHANGE).
pub trait SettingsBroadcaster: Send + Sync {
    fn broadcast(&self, area: &str);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorWallpaper {
    pub monitor_id: String,
    /// Empty means "no picture" (a solid color).
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WallpaperState {
    pub monitors: Vec<MonitorWallpaper>,
    pub fit: Option<WallpaperFit>,
    /// The desktop fill color around Fit/Center wallpapers (system-wide).
    pub background: Option<RgbColor>,
}

/// The shell's wallpaper API.
pub trait WallpaperApi: Send + Sync {
    fn get(&self) -> Result<WallpaperState>;
    /// Sets `path` on every monitor, and the fill color around it when given.
    fn set(&self, path: &Path, fit: WallpaperFit, background: Option<RgbColor>) -> Result<()>;
    /// Puts back per-monitor wallpapers.
    fn restore(&self, state: &WallpaperState) -> Result<()>;
}

pub type ConvertImage = Box<dyn Fn(&Path) -> Result<PathBuf> + Send + Sync>;

pub struct WindowsDesktopBackend {
    wallpaper: Box<dyn WallpaperApi>,
    registry: Box<dyn RegistryAccess>,
    broadcaster: Box<dyn SettingsBroadcaster>,
    convert: ConvertImage,
    snapshot_assets_dir: PathBuf,
    now: Clock,
}

impl WindowsDesktopBackend {
    pub fn new(
        wallpaper: Box<dyn WallpaperApi>,
        registry: Box<dyn RegistryAccess>,
        broadcaster: Box<dyn SettingsBroadcaster>,
        convert: ConvertImage,
        snapshot_assets_dir: PathBuf,
        now: Clock,
    ) -> Self {
        WindowsDesktopBackend { wallpaper, registry, broadcaster, convert, snapshot_assets_dir, now }
    }

    fn capture_copies(
        &self,
        state: &WallpaperState,
        staging: &Path,
        values: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let mut copies: HashMap<String, String> = HashMap::new();
        for (i, monitor) in state.monitors.iter().enumerate() {
            values.insert(format!("{MONITOR_PREFIX}{}", monitor.monitor_id), monitor.path.clone());
            if monitor.path.is_empty() || !Path::new(&monitor.path).is_file() {
                continue;
            }
            let key = monitor.path.to_lowercase();
            let copy = match copies.get(&key) {
                Some(copy) => copy.clone(),
                None => {
                    let file_name = format!("{i}{}", image_extension(&monitor.path));
                    std::fs::create_dir_all(staging)?;
                    std::fs::copy(&monitor.path, staging.join(&file_name))?;
                    let copy = self.snapshot_assets_dir.join(&file_name).to_string_lossy().into_owned();
                    copies.insert(key, copy.clone());
                    copy
                }
            };
            values.insert(format!("{COPY_PREFIX}{}", monitor.monitor_id), copy);
        }
        json::remove_dir_if_present(&self.snapshot_assets_dir)?;
        if staging.exists() {
            std::fs::rename(staging, &self.snapshot_assets_dir)?;
        }
        Ok(())
    }
}

/// TranscodedWallpaper has no extension; Windows sniffs the content, so any image extension works.
fn image_extension(path: &str) -> String {
    Path::new(path).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_else(|| ".jpg".into())
}

/// "reg:<key>|<name>": the snapshot key of a tracked registry value.
fn reg_snapshot_key(key: &str, name: &str) -> String {
    format!("{REG_PREFIX}{key}|{name}")
}

/// The Windows app writes the fit as its .NET enum name ("Fill").
fn fit_name(fit: WallpaperFit) -> &'static str {
    match fit {
        WallpaperFit::Fill => "Fill",
        WallpaperFit::Fit => "Fit",
        WallpaperFit::Stretch => "Stretch",
        WallpaperFit::Center => "Center",
        WallpaperFit::Tile => "Tile",
        WallpaperFit::Span => "Span",
    }
}

fn parse_fit_name(text: &str) -> Option<WallpaperFit> {
    WallpaperFit::ALL.into_iter().find(|f| fit_name(*f) == text)
}

impl DesktopBackend for WindowsDesktopBackend {
    fn capabilities(&self) -> DesktopCapabilities {
        CAPABILITIES
    }

    fn supported_fits(&self) -> Vec<WallpaperFit> {
        WallpaperFit::ALL.to_vec()
    }

    fn accent_color_note(&self) -> Option<&'static str> {
        Some(ACCENT_NOTE)
    }

    fn capture(&self) -> Result<DesktopSnapshot> {
        let mut values = BTreeMap::new();
        for (key, name) in TRACKED_VALUES {
            values.insert(reg_snapshot_key(key, name), RegValue::serialize(self.registry.read(key, name)?.as_ref()));
        }

        let state = self.wallpaper.get()?;
        if let Some(fit) = state.fit {
            values.insert(FIT_KEY.into(), fit_name(fit).into());
        }
        // The fill color around Fit/Center wallpapers is system-wide, so it's restored too.
        if let Some(background) = state.background {
            values.insert(BACKGROUND_KEY.into(), background.hex());
        }

        // Keep a private copy of each original wallpaper: Windows' own copy (TranscodedWallpaper) is
        // overwritten when a new one is set, and the user may delete the original file later. The
        // copies go to a staging folder that replaces the previous ones only once they're all made.
        let mut staging = self.snapshot_assets_dir.clone().into_os_string();
        staging.push(".new");
        let staging = PathBuf::from(staging);
        json::remove_dir_if_present(&staging)?;
        if let Err(error) = self.capture_copies(&state, &staging, &mut values) {
            let _ = json::remove_dir_if_present(&staging);
            return Err(error);
        }
        Ok(DesktopSnapshot { taken_at: (self.now)(), values })
    }

    fn restore(&self, snapshot: &DesktopSnapshot) -> Result<()> {
        for (key, name) in TRACKED_VALUES {
            let Some(serialized) = snapshot.values.get(&reg_snapshot_key(key, name)) else {
                continue;
            };
            match RegValue::deserialize(serialized)? {
                Some(value) => self.registry.write(key, name, &value)?,
                None => self.registry.delete(key, name)?,
            }
        }
        self.broadcaster.broadcast(IMMERSIVE_COLOR_SET);

        let monitors: Vec<MonitorWallpaper> = snapshot
            .values
            .iter()
            .filter_map(|(k, v)| Some((k.strip_prefix(MONITOR_PREFIX)?, v)))
            .map(|(id, path)| {
                let mut path = path.clone();
                if !path.is_empty()
                    && !Path::new(&path).exists()
                    && let Some(copy) = snapshot.values.get(&format!("{COPY_PREFIX}{id}"))
                    && Path::new(copy).exists()
                {
                    path = copy.clone();
                }
                MonitorWallpaper { monitor_id: id.to_string(), path }
            })
            .collect();
        if !monitors.is_empty() {
            let fit = snapshot.values.get(FIT_KEY).and_then(|f| parse_fit_name(f));
            let background = snapshot.values.get(BACKGROUND_KEY).and_then(|b| RgbColor::parse(b));
            self.wallpaper.restore(&WallpaperState { monitors, fit, background })?;
        }
        Ok(())
    }

    fn set_wallpaper(&self, image: &Path, fit: WallpaperFit, fill_color: Option<RgbColor>) -> Result<()> {
        let usable = (self.convert)(image)?;
        self.wallpaper.set(&usable, fit, fill_color)
    }

    fn set_appearance_mode(&self, mode: AppearanceMode) -> Result<()> {
        let light = RegValue::DWord(u32::from(mode == AppearanceMode::Light));
        self.registry.write(PERSONALIZE_KEY, "AppsUseLightTheme", &light)?;
        self.registry.write(PERSONALIZE_KEY, "SystemUsesLightTheme", &light)?;
        self.broadcaster.broadcast(IMMERSIVE_COLOR_SET);
        Ok(())
    }

    fn set_accent_color(&self, accent: RgbColor) -> Result<()> {
        let color = accent_math::normalize_accent(accent);
        let abgr = accent_math::to_abgr(color, 0xFF);
        let colorization = accent_math::to_argb(color, 0xC4);
        let start_menu = accent_math::to_abgr(accent_math::shades(color)[4], 0xFF);

        // Stop Windows from re-deriving the accent from the wallpaper that may have just been set.
        self.registry.write(DESKTOP_KEY, "AutoColorization", &RegValue::DWord(0))?;

        self.registry.write(
            ACCENT_KEY,
            "AccentPalette",
            &RegValue::Binary(accent_math::accent_palette_bytes(color)),
        )?;
        self.registry.write(ACCENT_KEY, "AccentColorMenu", &RegValue::DWord(abgr))?;
        self.registry.write(ACCENT_KEY, "StartColorMenu", &RegValue::DWord(start_menu))?;

        self.registry.write(DWM_KEY, "AccentColor", &RegValue::DWord(abgr))?;
        self.registry.write(DWM_KEY, "ColorizationColor", &RegValue::DWord(colorization))?;
        self.registry.write(DWM_KEY, "ColorizationAfterglow", &RegValue::DWord(colorization))?;

        self.broadcaster.broadcast(IMMERSIVE_COLOR_SET);
        Ok(())
    }

    fn current_wallpapers(&self) -> Vec<DisplayWallpaper> {
        self.wallpaper
            .get()
            .map(|state| {
                state
                    .monitors
                    .into_iter()
                    .map(|m| DisplayWallpaper {
                        display: m.monitor_id,
                        picture: (!m.path.is_empty()).then(|| PathBuf::from(m.path)),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// COLORREF is 0x00BBGGRR: ABGR with a zero alpha byte.
pub fn to_colorref(color: RgbColor) -> u32 {
    accent_math::to_abgr(color, 0)
}

pub fn from_colorref(value: u32) -> RgbColor {
    accent_math::from_abgr(value)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::color::c;
    use crate::test_support::ManualClock;

    #[derive(Default)]
    struct FakeRegistry(Mutex<HashMap<(String, String), RegValue>>);

    impl FakeRegistry {
        fn get(&self, key: &str, name: &str) -> Option<RegValue> {
            self.0.lock().unwrap().get(&(key.to_string(), name.to_string())).cloned()
        }
        fn set(&self, key: &str, name: &str, value: RegValue) {
            self.0.lock().unwrap().insert((key.to_string(), name.to_string()), value);
        }
        fn dword(&self, key: &str, name: &str) -> u32 {
            match self.get(key, name) {
                Some(RegValue::DWord(v)) => v,
                other => panic!("{key}\\{name}: {other:?}"),
            }
        }
    }

    impl RegistryAccess for Arc<FakeRegistry> {
        fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>> {
            Ok(self.get(key, name))
        }
        fn write(&self, key: &str, name: &str, value: &RegValue) -> Result<()> {
            self.set(key, name, value.clone());
            Ok(())
        }
        fn delete(&self, key: &str, name: &str) -> Result<()> {
            self.0.lock().unwrap().remove(&(key.to_string(), name.to_string()));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeWallpaper {
        state: Mutex<WallpaperState>,
        last_set: Mutex<Option<(PathBuf, WallpaperFit, Option<RgbColor>)>>,
        restored: Mutex<Option<WallpaperState>>,
    }

    impl WallpaperApi for Arc<FakeWallpaper> {
        fn get(&self) -> Result<WallpaperState> {
            Ok(self.state.lock().unwrap().clone())
        }
        fn set(&self, path: &Path, fit: WallpaperFit, background: Option<RgbColor>) -> Result<()> {
            *self.last_set.lock().unwrap() = Some((path.to_path_buf(), fit, background));
            Ok(())
        }
        fn restore(&self, state: &WallpaperState) -> Result<()> {
            *self.restored.lock().unwrap() = Some(state.clone());
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeBroadcaster(Mutex<Vec<String>>);

    impl SettingsBroadcaster for Arc<FakeBroadcaster> {
        fn broadcast(&self, area: &str) {
            self.0.lock().unwrap().push(area.to_string());
        }
    }

    struct Harness {
        dir: tempfile::TempDir,
        registry: Arc<FakeRegistry>,
        wallpaper: Arc<FakeWallpaper>,
        broadcaster: Arc<FakeBroadcaster>,
        backend: WindowsDesktopBackend,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let registry = Arc::new(FakeRegistry::default());
        let wallpaper = Arc::new(FakeWallpaper::default());
        let broadcaster = Arc::new(FakeBroadcaster::default());
        let backend = WindowsDesktopBackend::new(
            Box::new(wallpaper.clone()),
            Box::new(registry.clone()),
            Box::new(broadcaster.clone()),
            Box::new(|p: &Path| {
                Ok(if p.extension().is_some_and(|e| e == "webp") {
                    PathBuf::from(format!("{}.png", p.display()))
                } else {
                    p.to_path_buf()
                })
            }),
            dir.path().join("snapshot"),
            ManualClock::at(1_790_380_800).function(),
        );
        Harness { dir, registry, wallpaper, broadcaster, backend }
    }

    #[test]
    fn reports_all_three_capabilities_and_the_sign_out_note() {
        let h = harness();
        assert_eq!(h.backend.capabilities(), DesktopCapabilities::ALL);
        assert!(h.backend.accent_color_note().unwrap().contains("sign out"));
    }

    #[test]
    fn appearance_mode_sets_apps_and_system_then_broadcasts() {
        for (mode, expected) in [(AppearanceMode::Light, 1), (AppearanceMode::Dark, 0)] {
            let h = harness();
            h.backend.set_appearance_mode(mode).unwrap();

            assert_eq!(h.registry.dword(PERSONALIZE_KEY, "AppsUseLightTheme"), expected);
            assert_eq!(h.registry.dword(PERSONALIZE_KEY, "SystemUsesLightTheme"), expected);
            assert_eq!(*h.broadcaster.0.lock().unwrap(), ["ImmersiveColorSet"]);
        }
    }

    #[test]
    fn accent_writes_the_values_the_settings_app_uses() {
        let h = harness();
        h.backend.set_accent_color(c("#7aa2f7")).unwrap();

        assert_eq!(h.registry.dword(DWM_KEY, "AccentColor"), 0xFFF7_A27A);
        assert_eq!(h.registry.dword(DWM_KEY, "ColorizationColor"), 0xC47A_A2F7);
        assert_eq!(h.registry.dword(DWM_KEY, "ColorizationAfterglow"), 0xC47A_A2F7);
        assert_eq!(h.registry.dword(ACCENT_KEY, "AccentColorMenu"), 0xFFF7_A27A);
        assert!(h.registry.get(ACCENT_KEY, "StartColorMenu").is_some());
        let Some(RegValue::Binary(palette)) = h.registry.get(ACCENT_KEY, "AccentPalette") else { panic!() };
        assert_eq!(palette.len(), 32);
        assert_eq!(palette[12..16], [0x7a, 0xa2, 0xf7, 0x00]);
        assert_eq!(h.registry.dword(DESKTOP_KEY, "AutoColorization"), 0);
        assert_eq!(*h.broadcaster.0.lock().unwrap(), ["ImmersiveColorSet"]);
    }

    #[test]
    fn near_black_accents_are_lifted_before_writing() {
        let h = harness();
        h.backend.set_accent_color(c("#0a0a0a")).unwrap(); // Snow theme

        let written = accent_math::from_abgr(h.registry.dword(DWM_KEY, "AccentColor"));
        let (_, _, l) = written.to_hsl();
        assert!((0.24..=0.76).contains(&l), "{l}");
    }

    #[test]
    fn only_tracked_values_are_ever_written() {
        let h = harness();
        h.backend.set_appearance_mode(AppearanceMode::Light).unwrap();
        h.backend.set_accent_color(c("#123456")).unwrap();

        for (key, name) in h.registry.0.lock().unwrap().keys() {
            assert!(TRACKED_VALUES.contains(&(key.as_str(), name.as_str())), "{key}\\{name}");
        }
    }

    #[test]
    fn wallpaper_is_converted_when_needed_then_set_on_all_monitors() {
        let h = harness();
        h.backend.set_wallpaper(Path::new(r"C:\themes\vulkanite\wallpapers\1.webp"), WallpaperFit::Span, None).unwrap();

        assert_eq!(
            *h.wallpaper.last_set.lock().unwrap(),
            Some((PathBuf::from(r"C:\themes\vulkanite\wallpapers\1.webp.png"), WallpaperFit::Span, None))
        );
    }

    #[test]
    fn theme_background_is_passed_as_the_desktop_fill_color() {
        let h = harness();
        h.backend.set_wallpaper(Path::new("1.png"), WallpaperFit::Fit, Some(c("#1a1b26"))).unwrap();

        assert_eq!(
            *h.wallpaper.last_set.lock().unwrap(),
            Some((PathBuf::from("1.png"), WallpaperFit::Fit, Some(c("#1a1b26"))))
        );
    }

    #[test]
    fn colorref_is_0x00bbggrr() {
        assert_eq!(to_colorref(c("#7aa2f7")), 0x00F7_A27A);
        assert_eq!(from_colorref(to_colorref(c("#7aa2f7"))), c("#7aa2f7"));
    }

    #[test]
    fn capture_then_restore_puts_back_registry_and_per_monitor_wallpapers() {
        let h = harness();
        let original = h.dir.path().join("original.jpg");
        std::fs::write(&original, [1, 2, 3]).unwrap();
        let original = original.to_string_lossy().into_owned();
        h.registry.set(PERSONALIZE_KEY, "AppsUseLightTheme", RegValue::DWord(1));
        h.registry.set(DWM_KEY, "AccentColor", RegValue::DWord(0xFFD4_7800));
        h.registry.set(ACCENT_KEY, "AccentPalette", RegValue::Binary(vec![9, 9, 9]));
        *h.wallpaper.state.lock().unwrap() = WallpaperState {
            monitors: vec![
                MonitorWallpaper { monitor_id: "MON1".into(), path: original.clone() },
                MonitorWallpaper { monitor_id: "MON2".into(), path: String::new() },
            ],
            fit: Some(WallpaperFit::Fit),
            background: Some(c("#000000")),
        };

        let snapshot = h.backend.capture().unwrap();
        assert_eq!(snapshot.values["wallpaper-fit"], "Fit");

        // Apply a theme, which changes values and adds ones that didn't exist before.
        h.backend.set_appearance_mode(AppearanceMode::Dark).unwrap();
        h.backend.set_accent_color(c("#ff0000")).unwrap();
        h.backend.set_wallpaper(&h.dir.path().join("theme.png"), WallpaperFit::Fill, Some(c("#1a1b26"))).unwrap();

        h.backend.restore(&snapshot).unwrap();

        assert_eq!(h.registry.dword(PERSONALIZE_KEY, "AppsUseLightTheme"), 1);
        assert_eq!(h.registry.dword(DWM_KEY, "AccentColor"), 0xFFD4_7800);
        assert_eq!(h.registry.get(ACCENT_KEY, "AccentPalette"), Some(RegValue::Binary(vec![9, 9, 9])));
        // Values that were absent before are removed again.
        assert_eq!(h.registry.get(PERSONALIZE_KEY, "SystemUsesLightTheme"), None);
        assert_eq!(h.registry.get(DESKTOP_KEY, "AutoColorization"), None);
        assert_eq!(h.registry.get(DWM_KEY, "ColorizationColor"), None);

        let restored = h.wallpaper.restored.lock().unwrap().clone().unwrap();
        assert_eq!(restored.fit, Some(WallpaperFit::Fit));
        assert_eq!(restored.background, Some(c("#000000")));
        assert_eq!(
            restored.monitors,
            [
                MonitorWallpaper { monitor_id: "MON1".into(), path: original },
                MonitorWallpaper { monitor_id: "MON2".into(), path: String::new() },
            ]
        );
    }

    #[test]
    fn restore_uses_the_private_copy_when_the_original_wallpaper_is_gone() {
        let h = harness();
        let original = h.dir.path().join("TranscodedWallpaper");
        std::fs::write(&original, [1, 2, 3]).unwrap();
        *h.wallpaper.state.lock().unwrap() = WallpaperState {
            monitors: vec![MonitorWallpaper { monitor_id: "MON1".into(), path: original.to_string_lossy().into() }],
            fit: Some(WallpaperFit::Fill),
            background: None,
        };

        let snapshot = h.backend.capture().unwrap();
        std::fs::remove_file(&original).unwrap();
        h.backend.restore(&snapshot).unwrap();

        let restored = h.wallpaper.restored.lock().unwrap().clone().unwrap();
        let path = PathBuf::from(&restored.monitors[0].path);
        assert_eq!(path, h.dir.path().join("snapshot/0.jpg"));
        assert_eq!(std::fs::read(path).unwrap(), [1, 2, 3]);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_capture_keeps_the_previous_wallpaper_copies() {
        use std::os::unix::fs::PermissionsExt;
        let h = harness();
        let original = h.dir.path().join("original.jpg");
        std::fs::write(&original, [1, 2, 3]).unwrap();
        *h.wallpaper.state.lock().unwrap() = WallpaperState {
            monitors: vec![MonitorWallpaper { monitor_id: "MON1".into(), path: original.to_string_lossy().into() }],
            fit: Some(WallpaperFit::Fill),
            background: None,
        };
        let first = h.backend.capture().unwrap();

        std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o000)).unwrap();
        let second = h.backend.capture();
        std::fs::set_permissions(&original, std::fs::Permissions::from_mode(0o644)).unwrap();

        assert!(second.is_err());
        assert_eq!(std::fs::read(&first.values["monitor-copy:MON1"]).unwrap(), [1, 2, 3]);
        let dirs: Vec<_> = std::fs::read_dir(h.dir.path())
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_dir())
            .map(|e| e.file_name())
            .collect();
        assert_eq!(dirs, ["snapshot"]);
    }

    #[test]
    fn snapshot_survives_json_round_trip() {
        let h = harness();
        h.registry.set(ACCENT_KEY, "AccentPalette", RegValue::Binary(vec![1, 2, 3, 4]));
        h.registry.set(DESKTOP_KEY, "AutoColorization", RegValue::String("1".into()));
        let snapshot = h.backend.capture().unwrap();

        let loaded: DesktopSnapshot = serde_json::from_str(&serde_json::to_string(&snapshot).unwrap()).unwrap();
        h.backend.set_accent_color(c("#00ff00")).unwrap();
        h.backend.restore(&loaded).unwrap();

        assert_eq!(h.registry.get(ACCENT_KEY, "AccentPalette"), Some(RegValue::Binary(vec![1, 2, 3, 4])));
        assert_eq!(h.registry.get(DESKTOP_KEY, "AutoColorization"), Some(RegValue::String("1".into())));
    }

    #[test]
    fn registry_values_round_trip_through_snapshot_strings() {
        for value in [
            None,
            Some(RegValue::DWord(0xFFD4_7800)),
            Some(RegValue::Binary(vec![0, 255, 7])),
            Some(RegValue::QWord(1_234_567_890_123)),
            Some(RegValue::String(r"%USERPROFILE%\x: y".into())),
            Some(RegValue::ExpandString(r"%USERPROFILE%\x: y".into())),
        ] {
            assert_eq!(RegValue::deserialize(&RegValue::serialize(value.as_ref())).unwrap(), value);
        }
    }

    #[test]
    fn reads_the_windows_apps_snapshot_strings() {
        assert_eq!(RegValue::deserialize("dword:4292114432").unwrap(), Some(RegValue::DWord(0xFFD4_7800)));
        assert_eq!(RegValue::deserialize("binary:AP8H").unwrap(), Some(RegValue::Binary(vec![0, 255, 7])));
        assert!(RegValue::deserialize("nonsense").is_err());
    }
}
