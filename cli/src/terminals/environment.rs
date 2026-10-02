use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::color::RgbColor;
use crate::paths::AppPaths;

type FindApp = Box<dyn Fn(&str) -> Option<PathBuf> + Send + Sync>;
type OpenWith = Box<dyn Fn(&Path, &Path) -> Result<()> + Send + Sync>;
type ReadProfiles = Box<dyn Fn() -> Option<Vec<String>> + Send + Sync>;
type ArchiveColor = Box<dyn Fn(RgbColor) -> Result<Vec<u8>> + Send + Sync>;

/// Everything the exporters need from outside the tool.
pub struct TerminalEnvironment {
    pub home: PathBuf,
    /// `$XDG_CONFIG_HOME`, or `~/.config` when unset.
    pub config_dir: PathBuf,
    /// Where exporters that hand a file to the terminal keep it (in the data folder).
    pub exports_dir: PathBuf,
    /// Windows Terminal's per-user fragments folder for this tool.
    pub windows_terminal_fragments: PathBuf,
    /// The app with this identifier (a macOS bundle identifier, or "ghostty" /
    /// "windows-terminal" elsewhere), if installed.
    pub find_app: FindApp,
    /// Opens `file` with the app at `app`.
    pub open: OpenWith,
    /// The names of Terminal.app's profiles (read-only).
    pub terminal_profiles: ReadProfiles,
    /// A keyed-archived NSColor, as Terminal.app profiles store colors.
    pub archive_color: ArchiveColor,
}

impl TerminalEnvironment {
    /// The real machine. With the test switches (`--dry-run`, or another data folder on Windows,
    /// as the Windows app does) exports stay inside the data folder and nothing is opened, so
    /// development runs can't touch real terminal settings.
    pub fn live(paths: &AppPaths, dry_run: bool, custom_data_dir: bool) -> Self {
        let real_home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let xdg_config = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
        let local_app_data = dirs::data_local_dir().unwrap_or_else(|| real_home.clone());

        let (home, config_dir) = if dry_run {
            let home = paths.root.join("dry-run-home");
            (home.clone(), home.join(".config"))
        } else {
            (real_home.clone(), xdg_config.unwrap_or_else(|| real_home.join(".config")))
        };
        let windows_terminal_fragments = if dry_run || custom_data_dir {
            paths.root.join("windows-terminal-fragments")
        } else {
            local_app_data.join("Microsoft").join("Windows Terminal").join("Fragments").join("OmarchyThemes")
        };

        let open: OpenWith = if dry_run { Box::new(|_, _| Ok(())) } else { Box::new(open_with) };

        TerminalEnvironment {
            home,
            config_dir,
            exports_dir: paths.root.join("terminal"),
            windows_terminal_fragments,
            find_app: Box::new(move |id| find_app(id, &local_app_data)),
            open,
            terminal_profiles: Box::new(terminal_profiles),
            archive_color: Box::new(archive_color),
        }
    }
}

#[cfg(target_os = "macos")]
fn find_app(id: &str, _local_app_data: &Path) -> Option<PathBuf> {
    crate::platform::mac_native::find_app(id)
}

#[cfg(windows)]
fn find_app(id: &str, local_app_data: &Path) -> Option<PathBuf> {
    if id != "windows-terminal" {
        return None;
    }
    let wt = local_app_data.join(r"Microsoft\WindowsApps\wt.exe");
    let packages = ["Microsoft.WindowsTerminal_8wekyb3d8bbwe", "Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe"];
    if wt.exists() {
        return Some(wt);
    }
    packages.iter().map(|p| local_app_data.join("Packages").join(p)).find(|p| p.is_dir())
}

#[cfg(not(any(target_os = "macos", windows)))]
fn find_app(id: &str, _local_app_data: &Path) -> Option<PathBuf> {
    let executable = match id {
        "com.mitchellh.ghostty" | "ghostty" => "ghostty",
        _ => return None,
    };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|dir| dir.join(executable)).find(|p| p.is_file())
}

fn open_with(file: &Path, app: &Path) -> Result<()> {
    let status = std::process::Command::new("/usr/bin/open").arg("-a").arg(app).arg(file).status()?;
    anyhow::ensure!(status.success(), "Couldn't open {} with {}.", file.display(), app.display());
    Ok(())
}

#[cfg(target_os = "macos")]
fn terminal_profiles() -> Option<Vec<String>> {
    crate::platform::mac_native::preference_dictionary_keys("com.apple.Terminal", "Window Settings")
}

#[cfg(not(target_os = "macos"))]
fn terminal_profiles() -> Option<Vec<String>> {
    None
}

#[cfg(target_os = "macos")]
fn archive_color(color: RgbColor) -> Result<Vec<u8>> {
    crate::platform::mac_native::archive_color(color)
}

#[cfg(not(target_os = "macos"))]
fn archive_color(_color: RgbColor) -> Result<Vec<u8>> {
    anyhow::bail!("Terminal profiles can only be made on macOS.")
}
