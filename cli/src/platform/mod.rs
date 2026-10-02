//! The OS backends. Each backend's logic is compiled (and tested) on every OS against fakes; only
//! the thin layer that calls the real OS is behind `cfg`.

pub mod image_convert;
pub mod mac;
pub mod windows;

#[cfg(target_os = "macos")]
pub mod mac_native;
#[cfg(windows)]
pub mod windows_native;

use crate::paths::AppPaths;
use crate::theming::{DesktopBackend, DryRunBackend};

/// "macOS", "Windows", or the OS's own name.
pub fn os_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        "freebsd" => "FreeBSD",
        other => other,
    }
}

/// Where the user changes what this OS doesn't let apps change, if anywhere.
pub fn appearance_settings_hint() -> Option<&'static str> {
    if cfg!(target_os = "macos") { Some("System Settings › Appearance") } else { None }
}

/// The real desktop of this machine, or None where applying themes isn't supported (e.g. Linux,
/// where Omarchy applies its themes itself).
#[allow(unused_variables)]
pub fn live_backend(paths: &AppPaths) -> Option<Box<dyn DesktopBackend>> {
    #[cfg(target_os = "macos")]
    return Some(Box::new(mac::MacDesktopBackend::new(
        Box::new(mac_native::NSWorkspaceWallpaperApi),
        paths.original_desktop_dir(),
        crate::net::system_clock(),
    )));
    #[cfg(windows)]
    return Some(Box::new(windows::WindowsDesktopBackend::new(
        Box::new(windows_native::DesktopWallpaperApi),
        Box::new(windows_native::CurrentUserRegistry),
        Box::new(windows_native::Win32SettingsBroadcaster),
        Box::new(|path: &std::path::Path| {
            image_convert::ImageConverter::new(image_convert::ConvertedLayout::Windows).ensure_supported_format(path)
        }),
        paths.original_desktop_dir(),
        crate::net::system_clock(),
    )));
    #[cfg(not(any(target_os = "macos", windows)))]
    None
}

/// A backend that reports what this OS's backend can do but changes nothing; None where applying
/// isn't supported at all.
pub fn dry_run_backend() -> Option<DryRunBackend> {
    if cfg!(target_os = "macos") {
        Some(DryRunBackend::new(mac::CAPABILITIES, mac::SUPPORTED_FITS.to_vec(), None))
    } else if cfg!(windows) {
        Some(DryRunBackend::new(
            windows::CAPABILITIES,
            crate::theming::WallpaperFit::ALL.to_vec(),
            Some(windows::ACCENT_NOTE),
        ))
    } else {
        None
    }
}
