use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use super::{TerminalEnvironment, TerminalExporter, scheme_file_name};
use crate::json;
use crate::palette::TerminalColors;

pub const ID: &str = "ghostty";
const BUNDLE_IDENTIFIER: &str = "com.mitchellh.ghostty";

/// The comment line that says which theme a file is for.
const SLUG_LINE_PREFIX: &str = "# omarchy-themes-slug: ";

/// Ghostty: a theme file in `$XDG_CONFIG_HOME/ghostty/themes/` (`~/.config/ghostty/themes/`),
/// named after the scheme. The user picks it with `theme = "<name>"` in their config; this never
/// edits the config itself.
pub struct GhosttyExporter {
    environment: Arc<TerminalEnvironment>,
}

/// Another theme with the same name already has the Ghostty theme file this one would use.
#[derive(Debug, thiserror::Error)]
#[error(
    "Ghostty already has a “{0}” theme, added for another theme with the same name. Remove that one first, then try again."
)]
pub struct GhosttyThemeNameTakenError(pub String);

impl GhosttyExporter {
    pub fn new(environment: Arc<TerminalEnvironment>) -> Self {
        GhosttyExporter { environment }
    }

    fn themes_dir(&self) -> PathBuf {
        self.environment.config_dir.join("ghostty/themes")
    }

    /// The file name is the name Ghostty's `theme =` refers to.
    fn file(&self, theme_name: &str) -> PathBuf {
        self.themes_dir().join(scheme_file_name(theme_name))
    }

    /// The theme's files. Each file is named after the theme and tagged with its slug, so a theme
    /// renamed upstream still finds its file under the old name, and two themes with the same name
    /// are told apart. An untagged file under the theme's name (written before files were tagged)
    /// counts too. Only "… (Omarchy)" files are read.
    fn files(&self, slug: &str, theme_name: &str) -> Vec<PathBuf> {
        let own_name = scheme_file_name(theme_name);
        let our_suffix = scheme_file_name("");
        let Ok(entries) = std::fs::read_dir(self.themes_dir()) else {
            return Vec::new();
        };
        entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !name.ends_with(&our_suffix) {
                    return None;
                }
                let path = entry.path();
                match slug_in(&path) {
                    Some(tagged) if tagged == slug => Some(path),
                    None if name == own_name => Some(path),
                    _ => None,
                }
            })
            .collect()
    }

    pub fn theme_file(name: &str, slug: &str, colors: &TerminalColors) -> String {
        let mut lines = vec![
            format!("# {name}, added by Omarchy Themes. Use it with: theme = \"{name}\""),
            format!("{SLUG_LINE_PREFIX}{slug}"),
            format!("background = {}", colors.background),
            format!("foreground = {}", colors.foreground),
            format!("cursor-color = {}", colors.cursor),
            format!("selection-background = {}", colors.selection_background),
            format!("selection-foreground = {}", colors.foreground),
        ];
        lines.extend(colors.ansi.iter().enumerate().map(|(i, c)| format!("palette = {i}={c}")));
        lines.join("\n") + "\n"
    }
}

/// The slug a theme file is tagged with, or None (missing, unreadable, untagged or not ours).
pub fn slug_in(file: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    text.lines().find_map(|line| line.strip_prefix(SLUG_LINE_PREFIX)).map(|s| s.trim_end().to_string())
}

impl TerminalExporter for GhosttyExporter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "Ghostty"
    }

    /// The app, or a Ghostty config folder (for installs the OS doesn't know about).
    fn is_installed(&self) -> bool {
        (self.environment.find_app)(BUNDLE_IDENTIFIER).is_some() || self.environment.config_dir.join("ghostty").exists()
    }

    fn add_hint(&self) -> String {
        "Adds these colors to Ghostty as a theme. Your Ghostty config isn't changed.".into()
    }

    fn is_added(&self, slug: &str, theme_name: &str) -> bool {
        !self.files(slug, theme_name).is_empty()
    }

    fn add(&self, slug: &str, theme_name: &str, colors: &TerminalColors) -> Result<()> {
        let file = self.file(theme_name);
        let scheme = self.scheme_name(theme_name);
        if let Some(owner) = slug_in(&file)
            && owner != slug
        {
            return Err(GhosttyThemeNameTakenError(scheme).into());
        }
        // A theme renamed since it was added: its file under the old name goes.
        for old in self.files(slug, theme_name).into_iter().filter(|f| *f != file) {
            json::remove_file_if_present(&old)?;
        }
        json::write_atomic(&file, Self::theme_file(&scheme, slug, colors).as_bytes())
    }

    fn remove(&self, slug: &str, theme_name: &str) -> Result<()> {
        for file in self.files(slug, theme_name) {
            json::remove_file_if_present(&file)?;
        }
        Ok(())
    }

    fn location(&self, _slug: &str, theme_name: &str) -> Option<PathBuf> {
        Some(self.file(theme_name))
    }

    fn added_message(&self, scheme: &str) -> String {
        let reload = if cfg!(target_os = "macos") { "reload the config (⌘⇧,)" } else { "reload the config" };
        format!("To use it, set theme = \"{scheme}\" in your Ghostty config, then {reload} or restart Ghostty.")
    }

    fn removed_message(&self, scheme: &str) -> String {
        format!(
            "The “{scheme}” theme file was deleted. If your Ghostty config still says theme = \"{scheme}\", change it."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminals::test_support::{FakeMachine, tokyo};

    fn names(dir: &std::path::Path) -> Vec<String> {
        let mut names: Vec<_> =
            std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn add_writes_a_theme_file_named_after_the_scheme() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let ghostty = GhosttyExporter::new(machine.environment.clone());

        ghostty.add("omarchy.tokyo-night", "Tokyo Night", &tokyo()).unwrap();

        let file = machine.temp.path().join("home/.config/ghostty/themes/Tokyo Night (Omarchy)");
        let text = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<_> = text.lines().collect();
        for expected in [
            "background = #1a1b26",
            "foreground = #a9b1d6",
            "cursor-color = #c0caf5",
            "selection-background = #292e42",
            "palette = 0=#004080",
            "palette = 15=#f04080",
        ] {
            assert!(lines.contains(&expected), "missing {expected}");
        }
        assert_eq!(lines.iter().filter(|l| l.starts_with("palette = ")).count(), 16);
        assert!(ghostty.is_added("omarchy.tokyo-night", "Tokyo Night"));

        ghostty.remove("omarchy.tokyo-night", "Tokyo Night").unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn a_renamed_theme_finds_and_replaces_its_file_under_the_old_name() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let ghostty = GhosttyExporter::new(machine.environment.clone());
        let themes = machine.temp.path().join("home/.config/ghostty/themes");
        ghostty.add("nord", "Nord", &tokyo()).unwrap();

        assert!(ghostty.is_added("nord", "Nord Deep"));

        ghostty.add("nord", "Nord Deep", &tokyo()).unwrap();
        assert_eq!(names(&themes), ["Nord Deep (Omarchy)"]);

        ghostty.remove("nord", "Nord Deep").unwrap();
        assert!(names(&themes).is_empty());
    }

    #[test]
    fn another_theme_with_the_same_name_is_not_overwritten() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let ghostty = GhosttyExporter::new(machine.environment.clone());
        let themes = machine.temp.path().join("home/.config/ghostty/themes");
        ghostty.add("omarchy.nord", "Nord", &tokyo()).unwrap();

        assert!(!ghostty.is_added("nord", "Nord"));
        assert!(ghostty.add("nord", "Nord", &tokyo()).err().unwrap().is::<GhosttyThemeNameTakenError>());
        ghostty.remove("nord", "Nord").unwrap();

        assert_eq!(slug_in(&themes.join("Nord (Omarchy)")).as_deref(), Some("omarchy.nord"));
    }

    #[test]
    fn untagged_files_from_earlier_versions_still_count() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let ghostty = GhosttyExporter::new(machine.environment.clone());
        let themes = machine.temp.path().join("home/.config/ghostty/themes");
        json::write_atomic(
            &themes.join("Nord (Omarchy)"),
            b"# Nord (Omarchy), added by Omarchy Themes.\nbackground = #2e3440\n",
        )
        .unwrap();
        json::write_atomic(&themes.join("My Own"), b"background = #000000\n").unwrap();

        assert!(ghostty.is_added("nord", "Nord"));

        ghostty.remove("nord", "Nord").unwrap();
        assert_eq!(names(&themes), ["My Own"]);
    }

    #[test]
    fn a_config_folder_counts_as_installed() {
        let machine = FakeMachine::new(&[]);
        let ghostty = GhosttyExporter::new(machine.environment.clone());
        assert!(!ghostty.is_installed());

        std::fs::create_dir_all(machine.environment.config_dir.join("ghostty")).unwrap();
        assert!(ghostty.is_installed());
    }
}
