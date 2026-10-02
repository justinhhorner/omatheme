//! The real macOS calls, through objc2. NSWorkspace's desktop-image methods want the main thread,
//! which is where the CLI runs them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSColor, NSColorSpace, NSScreen, NSWorkspace, NSWorkspaceDesktopImageAllowClippingKey,
    NSWorkspaceDesktopImageFillColorKey, NSWorkspaceDesktopImageOptionKey, NSWorkspaceDesktopImageScalingKey,
};
use objc2_foundation::{NSDictionary, NSKeyedArchiver, NSNumber, NSString, NSURL, NSUserDefaults};

use super::mac::{ScreenWallpaper, WallpaperApi, WallpaperOptions};
use crate::color::RgbColor;

fn main_thread() -> Result<MainThreadMarker> {
    MainThreadMarker::new().ok_or_else(|| anyhow!("The desktop picture can only be changed from the main thread."))
}

fn screen_id(screen: &NSScreen) -> Option<String> {
    let key = NSString::from_str("NSScreenNumber");
    let number = screen.deviceDescription().objectForKey(&key)?;
    Some(number.downcast_ref::<NSNumber>()?.as_u64().to_string())
}

fn ns_color(color: RgbColor) -> Retained<NSColor> {
    let (r, g, b) = color.unit_components();
    NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0)
}

fn rgb(color: &NSColor) -> Option<RgbColor> {
    let srgb = color.colorUsingColorSpace(&NSColorSpace::sRGBColorSpace())?;
    let byte = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Some(RgbColor::new(byte(srgb.redComponent()), byte(srgb.greenComponent()), byte(srgb.blueComponent())))
}

fn options_from(dictionary: &NSDictionary<NSWorkspaceDesktopImageOptionKey, AnyObject>) -> WallpaperOptions {
    // SAFETY: the keys are AppKit's own constants, valid for the process's lifetime.
    let (scaling_key, clipping_key, fill_key) = unsafe {
        (
            NSWorkspaceDesktopImageScalingKey,
            NSWorkspaceDesktopImageAllowClippingKey,
            NSWorkspaceDesktopImageFillColorKey,
        )
    };
    let number = |key| dictionary.objectForKey(key).and_then(|o| o.downcast::<NSNumber>().ok());
    WallpaperOptions {
        scaling: number(scaling_key).map(|n| n.as_u64()),
        allow_clipping: number(clipping_key).map(|n| n.as_bool()),
        fill_color: dictionary.objectForKey(fill_key).and_then(|o| o.downcast::<NSColor>().ok()).and_then(|c| rgb(&c)),
    }
}

fn dictionary_from(options: &WallpaperOptions) -> Retained<NSDictionary<NSWorkspaceDesktopImageOptionKey, AnyObject>> {
    // SAFETY: as above.
    let (scaling_key, clipping_key, fill_key) = unsafe {
        (
            NSWorkspaceDesktopImageScalingKey,
            NSWorkspaceDesktopImageAllowClippingKey,
            NSWorkspaceDesktopImageFillColorKey,
        )
    };
    let mut keys: Vec<&NSWorkspaceDesktopImageOptionKey> = Vec::new();
    let mut objects: Vec<Retained<AnyObject>> = Vec::new();
    if let Some(scaling) = options.scaling {
        keys.push(scaling_key);
        objects.push(NSNumber::new_u64(scaling).into());
    }
    if let Some(clipping) = options.allow_clipping {
        keys.push(clipping_key);
        objects.push(NSNumber::new_bool(clipping).into());
    }
    if let Some(fill) = options.fill_color {
        keys.push(fill_key);
        objects.push(ns_color(fill).into());
    }
    NSDictionary::from_retained_objects(&keys, &objects)
}

/// NSWorkspace's per-screen desktop pictures. They only affect the current Space on each screen.
pub struct NSWorkspaceWallpaperApi;

impl WallpaperApi for NSWorkspaceWallpaperApi {
    fn current_wallpapers(&self) -> Vec<ScreenWallpaper> {
        let Ok(mtm) = main_thread() else {
            return Vec::new();
        };
        let workspace = NSWorkspace::sharedWorkspace();
        NSScreen::screens(mtm)
            .iter()
            .filter_map(|screen| {
                Some(ScreenWallpaper {
                    screen_id: screen_id(&screen)?,
                    image: workspace.desktopImageURLForScreen(&screen).and_then(|url| url.to_file_path()),
                    options: workspace
                        .desktopImageOptionsForScreen(&screen)
                        .map(|d| options_from(&d))
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    fn set_wallpaper(&self, image: &Path, options: &WallpaperOptions, screen_id_wanted: &str) -> Result<()> {
        let mtm = main_thread()?;
        let screen = NSScreen::screens(mtm)
            .iter()
            .find(|s| screen_id(s).as_deref() == Some(screen_id_wanted))
            .ok_or_else(|| anyhow!("That display is no longer connected."))?;
        let url =
            NSURL::from_file_path(image).with_context(|| format!("{} isn't a usable file path.", image.display()))?;
        let dictionary = dictionary_from(options);
        // SAFETY: every value in the dictionary has the type AppKit documents for its key.
        unsafe { NSWorkspace::sharedWorkspace().setDesktopImageURL_forScreen_options_error(&url, &screen, &dictionary) }
            .map_err(|e| anyhow!("{}", e.localizedDescription()))
    }
}

/// The app with this bundle identifier, if installed (Launch Services, read-only).
pub fn find_app(bundle_identifier: &str) -> Option<PathBuf> {
    NSWorkspace::sharedWorkspace()
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle_identifier))
        .and_then(|url| url.to_file_path())
}

/// The keys of a dictionary in another app's preferences (read-only), e.g. Terminal's profiles.
pub fn preference_dictionary_keys(domain: &str, key: &str) -> Option<Vec<String>> {
    let defaults = NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(domain)))?;
    let dictionary = defaults.dictionaryForKey(&NSString::from_str(key))?;
    Some(dictionary.allKeys().iter().map(|k| k.to_string()).collect())
}

/// A keyed-archived NSColor (sRGB), as Terminal.app profiles store colors.
pub fn archive_color(color: RgbColor) -> Result<Vec<u8>> {
    // SAFETY: NSColor supports secure coding.
    let data =
        unsafe { NSKeyedArchiver::archivedDataWithRootObject_requiringSecureCoding_error(&ns_color(color), true) }
            .map_err(|e| anyhow!("{}", e.localizedDescription()))?;
    Ok(data.to_vec())
}
