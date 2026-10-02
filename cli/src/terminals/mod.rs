//! Sending a theme's colors to a terminal app, as a color scheme or profile of its own.
//! Exporters only add their own files; they never edit a terminal's settings.
//!
//! To support another terminal, add a type implementing [`TerminalExporter`] (`ITermExporter` is a
//! small one) and list it in [`all`]. Everything outside the tool (folders, finding and opening
//! apps, reading another app's preferences) goes through [`TerminalEnvironment`], so exporters are
//! tested against temp folders and fakes.

mod environment;
mod ghostty;
mod iterm;
mod terminal_app;
mod windows_terminal;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

pub use environment::TerminalEnvironment;
pub use ghostty::{GhosttyExporter, GhosttyThemeNameTakenError};
pub use iterm::ITermExporter;
pub use terminal_app::TerminalAppExporter;
pub use windows_terminal::WindowsTerminalExporter;

use crate::palette::TerminalColors;

pub trait TerminalExporter: Send + Sync {
    /// Stable identifier, used by `--app` and saved as `terminalApp` by the macOS app (e.g. "iterm2").
    fn id(&self) -> &'static str;

    /// The app's name (e.g. "iTerm2").
    fn display_name(&self) -> &'static str;

    /// Whether the terminal is on this machine; adding needs it.
    fn is_installed(&self) -> bool;

    /// Whether `remove` can take back what `add` did. When false, `remove_instructions` says how.
    fn can_remove(&self) -> bool {
        true
    }

    /// One line on what adding does.
    fn add_hint(&self) -> String;

    /// The name the theme's colors appear under in the terminal: "Tokyo Night (Omarchy)", the same
    /// in every terminal and both apps.
    fn scheme_name(&self, theme_name: &str) -> String {
        scheme_name(theme_name)
    }

    fn is_added(&self, slug: &str, theme_name: &str) -> bool;

    fn add(&self, slug: &str, theme_name: &str, colors: &TerminalColors) -> Result<()>;

    fn remove(&self, slug: &str, theme_name: &str) -> Result<()>;

    /// Where `add` writes the theme's file, for the output.
    fn location(&self, slug: &str, theme_name: &str) -> Option<PathBuf>;

    /// What to do next after adding, e.g. where to pick the scheme.
    fn added_message(&self, scheme: &str) -> String;

    fn removed_message(&self, scheme: &str) -> String;

    /// How to remove the scheme by hand, for terminals where `can_remove` is false.
    fn remove_instructions(&self, _scheme: &str) -> String {
        String::new()
    }
}

pub fn scheme_name(theme_name: &str) -> String {
    format!("{theme_name} (Omarchy)")
}

/// The scheme name made usable as a file name (theme names can contain "/").
pub fn scheme_file_name(theme_name: &str) -> String {
    scheme_name(theme_name).replace('/', "-")
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct TerminalExportUnsupportedError(pub String);

/// Every supported terminal on this OS, in display order. Register new exporters here.
pub fn all(environment: Arc<TerminalEnvironment>) -> Vec<Box<dyn TerminalExporter>> {
    if cfg!(target_os = "macos") {
        vec![
            Box::new(ITermExporter::new(environment.clone())),
            Box::new(GhosttyExporter::new(environment.clone())),
            Box::new(TerminalAppExporter::new(environment)),
        ]
    } else if cfg!(windows) {
        vec![Box::new(WindowsTerminalExporter::new(environment))]
    } else {
        vec![Box::new(GhosttyExporter::new(environment))]
    }
}

/// Used until the user picks another: iTerm2 on macOS, Windows Terminal on Windows, Ghostty elsewhere.
pub fn default_id() -> &'static str {
    if cfg!(target_os = "macos") {
        iterm::ID
    } else if cfg!(windows) {
        windows_terminal::ID
    } else {
        ghostty::ID
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::TerminalEnvironment;
    use crate::color::RgbColor;
    use crate::palette::TerminalColors;

    /// A fake machine for exporters: temp folders, a chosen set of installed apps, recorded "open"
    /// calls and canned Terminal profiles. Nothing touches real terminals.
    pub struct FakeMachine {
        pub temp: tempfile::TempDir,
        pub opened: Arc<Mutex<Vec<(PathBuf, PathBuf)>>>,
        pub profiles: Arc<Mutex<Option<Vec<String>>>>,
        pub environment: Arc<TerminalEnvironment>,
    }

    impl FakeMachine {
        pub fn new(installed: &[&str]) -> Self {
            let temp = tempfile::tempdir().unwrap();
            let installed: HashSet<String> = installed.iter().map(|s| s.to_string()).collect();
            let opened = Arc::new(Mutex::new(Vec::new()));
            let profiles = Arc::new(Mutex::new(None));
            let (opened_log, profile_list) = (opened.clone(), profiles.clone());
            let home = temp.path().join("home");
            let environment = Arc::new(TerminalEnvironment {
                config_dir: home.join(".config"),
                home,
                exports_dir: temp.path().join("data/terminal"),
                windows_terminal_fragments: temp.path().join("fragments/OmarchyThemes"),
                find_app: Box::new(move |id| {
                    installed.contains(id).then(|| PathBuf::from(format!("/Applications/{id}.app")))
                }),
                open: Box::new(move |file: &Path, app: &Path| {
                    opened_log.lock().unwrap().push((file.to_path_buf(), app.to_path_buf()));
                    Ok(())
                }),
                terminal_profiles: Box::new(move || profile_list.lock().unwrap().clone()),
                // A stand-in for NSKeyedArchiver: the color's hex, so tests can read it back.
                archive_color: Box::new(|c: RgbColor| Ok(c.hex().into_bytes())),
            });
            FakeMachine { temp, opened, profiles, environment }
        }
    }

    pub fn tokyo() -> TerminalColors {
        let c = |hex| RgbColor::parse(hex).unwrap();
        TerminalColors {
            background: c("#1a1b26"),
            foreground: c("#a9b1d6"),
            cursor: c("#c0caf5"),
            selection_background: c("#292e42"),
            ansi: (0..16).map(|i| RgbColor::new(i * 16, 0x40, 0x80)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::FakeMachine;
    use super::*;

    #[test]
    fn registry_lists_this_platforms_terminals_and_the_default_is_one_of_them() {
        let machine = FakeMachine::new(&[]);
        let exporters = all(machine.environment.clone());
        let ids: Vec<_> = exporters.iter().map(|e| e.id()).collect();

        if cfg!(target_os = "macos") {
            assert_eq!(ids, ["iterm2", "ghostty", "terminal"]);
            let names: Vec<_> = exporters.iter().map(|e| e.display_name()).collect();
            assert_eq!(names, ["iTerm2", "Ghostty", "Terminal"]);
            assert_eq!(default_id(), "iterm2");
        } else if cfg!(windows) {
            assert_eq!(ids, ["windows-terminal"]);
        } else {
            assert_eq!(ids, ["ghostty"]);
        }
        assert!(ids.contains(&default_id()));
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len());
    }

    #[test]
    fn schemes_use_the_same_name_everywhere() {
        let machine = FakeMachine::new(&[]);
        for exporter in [
            Box::new(ITermExporter::new(machine.environment.clone())) as Box<dyn TerminalExporter>,
            Box::new(GhosttyExporter::new(machine.environment.clone())),
            Box::new(TerminalAppExporter::new(machine.environment.clone())),
            Box::new(WindowsTerminalExporter::new(machine.environment.clone())),
        ] {
            assert_eq!(exporter.scheme_name("Tokyo Night"), "Tokyo Night (Omarchy)");
        }
        assert_eq!(scheme_file_name("A/B"), "A-B (Omarchy)");
    }

    #[test]
    fn installed_follows_the_app() {
        let machine = FakeMachine::new(&["com.googlecode.iterm2"]);
        assert!(ITermExporter::new(machine.environment.clone()).is_installed());
        assert!(!TerminalAppExporter::new(machine.environment.clone()).is_installed());
    }
}
