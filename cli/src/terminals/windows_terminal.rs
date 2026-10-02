use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;

use super::{TerminalEnvironment, TerminalExporter};
use crate::json;
use crate::palette::TerminalColors;
use crate::paths::{InvalidSlugError, is_valid_slug};

pub const ID: &str = "windows-terminal";

/// Windows Terminal: a color scheme through Terminal's JSON fragment extensions, one file per
/// theme in the per-user fragments folder
/// (`%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\OmarchyThemes\<slug>.json`, the Windows
/// app's folder too). Terminal merges fragments into its settings, so the user's settings.json is
/// never edited, and removing the file removes the scheme. Terminal reads fragments when it starts.
pub struct WindowsTerminalExporter {
    environment: Arc<TerminalEnvironment>,
}

#[derive(Serialize)]
struct Fragment {
    schemes: Vec<Scheme>,
}

/// A Windows Terminal color scheme: a name and every color in the table are required.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Scheme {
    name: String,
    background: String,
    foreground: String,
    cursor_color: String,
    selection_background: String,
    black: String,
    red: String,
    green: String,
    yellow: String,
    blue: String,
    purple: String,
    cyan: String,
    white: String,
    bright_black: String,
    bright_red: String,
    bright_green: String,
    bright_yellow: String,
    bright_blue: String,
    bright_purple: String,
    bright_cyan: String,
    bright_white: String,
}

impl Scheme {
    fn new(name: String, c: &TerminalColors) -> Self {
        let a: Vec<String> = c.ansi.iter().map(|x| x.hex()).collect();
        Scheme {
            name,
            background: c.background.hex(),
            foreground: c.foreground.hex(),
            cursor_color: c.cursor.hex(),
            selection_background: c.selection_background.hex(),
            black: a[0].clone(),
            red: a[1].clone(),
            green: a[2].clone(),
            yellow: a[3].clone(),
            blue: a[4].clone(),
            purple: a[5].clone(),
            cyan: a[6].clone(),
            white: a[7].clone(),
            bright_black: a[8].clone(),
            bright_red: a[9].clone(),
            bright_green: a[10].clone(),
            bright_yellow: a[11].clone(),
            bright_blue: a[12].clone(),
            bright_purple: a[13].clone(),
            bright_cyan: a[14].clone(),
            bright_white: a[15].clone(),
        }
    }
}

impl WindowsTerminalExporter {
    pub fn new(environment: Arc<TerminalEnvironment>) -> Self {
        WindowsTerminalExporter { environment }
    }

    fn file(&self, slug: &str) -> Result<PathBuf, InvalidSlugError> {
        if !is_valid_slug(slug) {
            return Err(InvalidSlugError(slug.to_string()));
        }
        Ok(self.environment.windows_terminal_fragments.join(format!("{slug}.json")))
    }
}

impl TerminalExporter for WindowsTerminalExporter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "Windows Terminal"
    }

    fn is_installed(&self) -> bool {
        (self.environment.find_app)(ID).is_some()
    }

    fn add_hint(&self) -> String {
        "Adds these colors to Windows Terminal as a color scheme. Your Terminal settings aren't changed.".into()
    }

    fn is_added(&self, slug: &str, _theme_name: &str) -> bool {
        self.file(slug).is_ok_and(|f| f.exists())
    }

    /// Writes (or replaces) the theme's scheme. UTF-8 without a BOM, as Terminal requires.
    fn add(&self, slug: &str, theme_name: &str, colors: &TerminalColors) -> Result<()> {
        let fragment = Fragment { schemes: vec![Scheme::new(self.scheme_name(theme_name), colors)] };
        json::write_json(&self.file(slug)?, &fragment)
    }

    fn remove(&self, slug: &str, _theme_name: &str) -> Result<()> {
        let file = self.file(slug)?;
        let dir = &self.environment.windows_terminal_fragments;
        if !dir.exists() {
            return Ok(());
        }
        json::remove_file_if_present(&file)?;
        if std::fs::read_dir(dir)?.next().is_none() {
            std::fs::remove_dir(dir)?;
        }
        Ok(())
    }

    fn location(&self, slug: &str, _theme_name: &str) -> Option<PathBuf> {
        self.file(slug).ok()
    }

    fn added_message(&self, scheme: &str) -> String {
        format!(
            "In Windows Terminal, open Settings and choose “{scheme}” as a profile's color scheme \
             (Profiles › Defaults › Appearance applies it to all of them). If Terminal is open, restart it first."
        )
    }

    fn removed_message(&self, scheme: &str) -> String {
        format!(
            "“{scheme}” is gone after Terminal restarts. A profile that used it goes back to Terminal's default colors."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::RgbColor;
    use crate::terminals::test_support::FakeMachine;

    const COLOR_KEYS: [&str; 16] = [
        "black",
        "red",
        "green",
        "yellow",
        "blue",
        "purple",
        "cyan",
        "white",
        "brightBlack",
        "brightRed",
        "brightGreen",
        "brightYellow",
        "brightBlue",
        "brightPurple",
        "brightCyan",
        "brightWhite",
    ];

    fn colors() -> TerminalColors {
        let c = |hex| RgbColor::parse(hex).unwrap();
        TerminalColors {
            background: c("#1a1b26"),
            foreground: c("#a9b1d6"),
            cursor: c("#c0caf5"),
            selection_background: c("#292e42"),
            ansi: (0..16).map(|i| RgbColor::new(i * 16, 0x20, 0x30)).collect(),
        }
    }

    #[test]
    fn adds_a_complete_named_scheme_as_a_fragment() {
        let machine = FakeMachine::new(&[]);
        let terminal = WindowsTerminalExporter::new(machine.environment.clone());

        terminal.add("omarchy.tokyo-night", "Tokyo Night", &colors()).unwrap();

        assert!(terminal.is_added("omarchy.tokyo-night", "Tokyo Night"));
        let bytes =
            std::fs::read(machine.environment.windows_terminal_fragments.join("omarchy.tokyo-night.json")).unwrap();
        assert!(!bytes.starts_with(b"\xEF\xBB\xBF"), "Terminal needs UTF-8 without a BOM");
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let schemes = json["schemes"].as_array().unwrap();
        assert_eq!(schemes.len(), 1);
        let scheme = &schemes[0];
        assert_eq!(scheme["name"], "Tokyo Night (Omarchy)");
        assert_eq!(scheme["background"], "#1a1b26");
        assert_eq!(scheme["foreground"], "#a9b1d6");
        assert_eq!(scheme["cursorColor"], "#c0caf5");
        assert_eq!(scheme["selectionBackground"], "#292e42");
        for (i, key) in COLOR_KEYS.iter().enumerate() {
            assert_eq!(scheme[key], RgbColor::new(i as u8 * 16, 0x20, 0x30).hex(), "{key}");
        }
        assert!(json.get("profiles").is_none(), "Only schemes; the user's profiles are left alone");
    }

    #[test]
    fn adding_again_replaces_and_removing_cleans_up() {
        let machine = FakeMachine::new(&[]);
        let terminal = WindowsTerminalExporter::new(machine.environment.clone());
        let dir = &machine.environment.windows_terminal_fragments;

        terminal.add("aetheria", "Aetheria", &colors()).unwrap();
        terminal.add("aetheria", "Aetheria", &colors()).unwrap();
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);

        terminal.remove("aetheria", "Aetheria").unwrap();
        assert!(!terminal.is_added("aetheria", "Aetheria"));
        assert!(!dir.exists(), "The empty app folder is removed too");
        terminal.remove("aetheria", "Aetheria").unwrap();
    }

    #[test]
    fn names_with_quotes_and_accents_stay_valid_json() {
        let machine = FakeMachine::new(&[]);
        let terminal = WindowsTerminalExporter::new(machine.environment.clone());

        terminal.add("rose-pine", "Rosé \"Pine\"", &colors()).unwrap();

        let json: serde_json::Value =
            json::read_json(&machine.environment.windows_terminal_fragments.join("rose-pine.json")).unwrap();
        assert_eq!(json["schemes"][0]["name"], "Rosé \"Pine\" (Omarchy)");
    }

    #[test]
    fn rejects_slugs_that_could_escape_the_folder() {
        let machine = FakeMachine::new(&[]);
        let terminal = WindowsTerminalExporter::new(machine.environment.clone());
        for slug in ["", "..", "../x", r"a\b"] {
            assert!(terminal.add(slug, "X", &colors()).is_err(), "{slug}");
        }
    }

    #[test]
    fn installed_follows_the_app() {
        assert!(WindowsTerminalExporter::new(FakeMachine::new(&[ID]).environment.clone()).is_installed());
        assert!(!WindowsTerminalExporter::new(FakeMachine::new(&[]).environment.clone()).is_installed());
    }
}
