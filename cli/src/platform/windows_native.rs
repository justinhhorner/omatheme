//! The real Windows calls: HKCU registry values, the WM_SETTINGCHANGE broadcast, and the shell's
//! IDesktopWallpaper COM object (SystemParametersInfo as the fallback). COM runs on the calling
//! (main) thread, initialised as a single-threaded apartment as the shell expects.

use std::ffi::c_void;
use std::path::Path;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{COLORREF, ERROR_FILE_NOT_FOUND, LPARAM, WIN32_ERROR, WPARAM};
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_BINARY, REG_DWORD, REG_EXPAND_SZ, REG_OPTION_NON_VOLATILE, REG_QWORD,
    REG_SZ, REG_VALUE_TYPE, RRF_NOEXPAND, RRF_RT_ANY, RegCloseKey, RegCreateKeyExW, RegDeleteKeyValueW, RegGetValueW,
    RegSetValueExW,
};
use windows::Win32::UI::Shell::{
    DESKTOP_WALLPAPER_POSITION, DWPOS_CENTER, DWPOS_FILL, DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH, DWPOS_TILE,
    DesktopWallpaper, IDesktopWallpaper,
};
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SPI_GETDESKWALLPAPER, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SendMessageTimeoutW, SystemParametersInfoW, WM_SETTINGCHANGE,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::windows::{
    MonitorWallpaper, RegValue, RegistryAccess, SettingsBroadcaster, WallpaperApi, WallpaperState, from_colorref,
    to_colorref,
};
use crate::color::RgbColor;
use crate::theming::WallpaperFit;

fn check(status: WIN32_ERROR, what: &str) -> Result<()> {
    if status.is_ok() {
        Ok(())
    } else {
        Err(anyhow!("{what} failed: {}", windows::core::Error::from(status.to_hresult())))
    }
}

/// HKEY_CURRENT_USER. Everything this tool changes is per-user; it never touches HKLM.
pub struct CurrentUserRegistry;

impl RegistryAccess for CurrentUserRegistry {
    fn read(&self, key: &str, name: &str) -> Result<Option<RegValue>> {
        let (key_w, name_w) = (HSTRING::from(key), HSTRING::from(name));
        let mut kind = REG_VALUE_TYPE::default();
        let mut size = 0u32;
        // SAFETY: sizes and buffers are passed as the API documents.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key_w,
                &name_w,
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status, "Reading the registry")?;
        let mut data = vec![0u8; size as usize];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                &key_w,
                &name_w,
                RRF_RT_ANY | RRF_NOEXPAND,
                Some(&mut kind),
                Some(data.as_mut_ptr() as *mut c_void),
                Some(&mut size),
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status, "Reading the registry")?;
        data.truncate(size as usize);
        let text = |bytes: &[u8]| {
            let wide: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();
            String::from_utf16_lossy(&wide).trim_end_matches('\0').to_string()
        };
        Ok(Some(match kind {
            REG_DWORD if data.len() >= 4 => RegValue::DWord(u32::from_le_bytes(data[..4].try_into()?)),
            REG_QWORD if data.len() >= 8 => RegValue::QWord(i64::from_le_bytes(data[..8].try_into()?)),
            REG_SZ => RegValue::String(text(&data)),
            REG_EXPAND_SZ => RegValue::ExpandString(text(&data)),
            _ => RegValue::Binary(data),
        }))
    }

    fn write(&self, key: &str, name: &str, value: &RegValue) -> Result<()> {
        let wide = |s: &str| s.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect::<Vec<u8>>();
        let (kind, data) = match value {
            RegValue::DWord(v) => (REG_DWORD, v.to_le_bytes().to_vec()),
            RegValue::QWord(v) => (REG_QWORD, v.to_le_bytes().to_vec()),
            RegValue::Binary(b) => (REG_BINARY, b.clone()),
            RegValue::String(s) => (REG_SZ, wide(s)),
            RegValue::ExpandString(s) => (REG_EXPAND_SZ, wide(s)),
        };
        let mut hkey = HKEY::default();
        // SAFETY: the key handle is closed below.
        unsafe {
            check(
                RegCreateKeyExW(
                    HKEY_CURRENT_USER,
                    &HSTRING::from(key),
                    None,
                    PCWSTR::null(),
                    REG_OPTION_NON_VOLATILE,
                    KEY_SET_VALUE,
                    None,
                    &mut hkey,
                    None,
                ),
                "Opening the registry",
            )?;
            let status = RegSetValueExW(hkey, &HSTRING::from(name), None, kind, Some(&data));
            let _ = RegCloseKey(hkey);
            check(status, "Writing the registry")
        }
    }

    fn delete(&self, key: &str, name: &str) -> Result<()> {
        // SAFETY: plain strings in, status out.
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(key), &HSTRING::from(name)) };
        if status == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(status, "Deleting a registry value") }
    }
}

pub struct Win32SettingsBroadcaster;

impl SettingsBroadcaster for Win32SettingsBroadcaster {
    fn broadcast(&self, area: &str) {
        let wide: Vec<u16> = area.encode_utf16().chain([0]).collect();
        // SAFETY: the string outlives the call. SMTO_ABORTIFHUNG: a hung window must not freeze us.
        unsafe {
            SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                1000,
                None,
            );
        }
    }
}

/// Monitor id used for the single SystemParametersInfo wallpaper.
const ALL_MONITORS: &str = "*";

/// Wallpaper via IDesktopWallpaper (per monitor, with position), falling back to
/// SystemParametersInfo(SPI_SETDESKWALLPAPER) if the COM object is unavailable.
pub struct DesktopWallpaperApi;

impl DesktopWallpaperApi {
    fn create() -> Option<IDesktopWallpaper> {
        // SAFETY: COM is initialised on this thread first; a second init is harmless.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL).ok()
        }
    }
}

fn take_string(value: PWSTR) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // SAFETY: the shell returned a CoTaskMem-allocated, NUL-terminated string, freed here.
    unsafe {
        let text = value.to_string().ok();
        CoTaskMemFree(Some(value.0 as *const c_void));
        text
    }
}

fn set_system_parameters_wallpaper(path: &str) -> Result<()> {
    let mut wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    // SAFETY: the buffer outlives the call.
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_mut_ptr() as *mut c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    }
    .map_err(|e| anyhow!("Windows refused to set the wallpaper ({e})."))
}

fn system_parameters_wallpaper() -> String {
    let mut buffer = [0u16; 1024];
    // SAFETY: the buffer's length is passed.
    let ok = unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr() as *mut c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    if ok.is_err() {
        return String::new();
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

fn to_position(fit: WallpaperFit) -> DESKTOP_WALLPAPER_POSITION {
    match fit {
        WallpaperFit::Fill => DWPOS_FILL,
        WallpaperFit::Fit => DWPOS_FIT,
        WallpaperFit::Stretch => DWPOS_STRETCH,
        WallpaperFit::Center => DWPOS_CENTER,
        WallpaperFit::Tile => DWPOS_TILE,
        WallpaperFit::Span => DWPOS_SPAN,
    }
}

fn from_position(position: DESKTOP_WALLPAPER_POSITION) -> Option<WallpaperFit> {
    WallpaperFit::ALL.into_iter().find(|fit| to_position(*fit) == position)
}

impl WallpaperApi for DesktopWallpaperApi {
    fn get(&self) -> Result<WallpaperState> {
        let Some(wallpaper) = Self::create() else {
            return Ok(WallpaperState {
                monitors: vec![MonitorWallpaper {
                    monitor_id: ALL_MONITORS.into(),
                    path: system_parameters_wallpaper(),
                }],
                fit: None,
                background: None,
            });
        };
        // SAFETY: plain COM calls on a live object; returned strings are freed by take_string.
        unsafe {
            let count = wallpaper.GetMonitorDevicePathCount()?;
            let mut monitors = Vec::new();
            for i in 0..count {
                let Some(id) = take_string(wallpaper.GetMonitorDevicePathAt(i)?).filter(|id| !id.is_empty()) else {
                    continue;
                };
                let path = take_string(wallpaper.GetWallpaper(&HSTRING::from(&id))?).unwrap_or_default();
                monitors.push(MonitorWallpaper { monitor_id: id, path });
            }
            let fit = wallpaper.GetPosition().ok().and_then(from_position);
            let background = wallpaper.GetBackgroundColor().ok().map(|c| from_colorref(c.0));
            Ok(WallpaperState { monitors, fit, background })
        }
    }

    fn set(&self, path: &Path, fit: WallpaperFit, background: Option<RgbColor>) -> Result<()> {
        let path_text = path.to_string_lossy().into_owned();
        if let Some(wallpaper) = Self::create() {
            // SAFETY: plain COM calls on a live object.
            let result = unsafe {
                (|| -> windows::core::Result<()> {
                    if let Some(color) = background {
                        wallpaper.SetBackgroundColor(COLORREF(to_colorref(color)))?;
                    }
                    wallpaper.SetPosition(to_position(fit))?;
                    wallpaper.SetWallpaper(PCWSTR::null(), &HSTRING::from(&path_text)) // null monitor = all monitors
                })()
            };
            if result.is_ok() {
                return Ok(());
            }
        }
        set_system_parameters_wallpaper(&path_text)
    }

    fn restore(&self, state: &WallpaperState) -> Result<()> {
        let wallpaper = Self::create();
        // SAFETY: plain COM calls on a live object.
        unsafe {
            if let Some(wallpaper) = &wallpaper {
                if let Some(background) = state.background {
                    let _ = wallpaper.SetBackgroundColor(COLORREF(to_colorref(background)));
                }
                if let Some(fit) = state.fit {
                    let _ = wallpaper.SetPosition(to_position(fit));
                }
            }
            for monitor in &state.monitors {
                // "No picture" and the fallback can only be expressed through SystemParametersInfo.
                match &wallpaper {
                    Some(wallpaper) if monitor.monitor_id != ALL_MONITORS && !monitor.path.is_empty() => {
                        // A monitor disconnected since the snapshot is skipped.
                        let _ =
                            wallpaper.SetWallpaper(&HSTRING::from(&monitor.monitor_id), &HSTRING::from(&monitor.path));
                    }
                    _ => set_system_parameters_wallpaper(&monitor.path)?,
                }
            }
        }
        Ok(())
    }
}
