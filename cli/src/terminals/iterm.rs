use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde_json::{Value, json};

use super::{TerminalEnvironment, TerminalExporter};
use crate::color::RgbColor;
use crate::json;
use crate::palette::TerminalColors;

pub const ID: &str = "iterm2";
const BUNDLE_IDENTIFIER: &str = "com.googlecode.iterm2";

/// iTerm2: a Dynamic Profile, one JSON file per theme in
/// `~/Library/Application Support/iTerm2/DynamicProfiles/`. iTerm2 watches that folder, so the
/// profile appears (and disappears on remove) without a restart.
pub struct ITermExporter {
    environment: Arc<TerminalEnvironment>,
}

impl ITermExporter {
    pub fn new(environment: Arc<TerminalEnvironment>) -> Self {
        ITermExporter { environment }
    }

    fn file(&self, slug: &str) -> PathBuf {
        self.environment
            .home
            .join("Library/Application Support/iTerm2/DynamicProfiles")
            .join(format!("omarchy-themes-{slug}.json"))
    }

    pub fn profile_json(slug: &str, name: &str, colors: &TerminalColors) -> Value {
        let mut profile = json!({
            "Name": name,
            // Stable, so adding a theme again updates its profile instead of making a second one.
            "Guid": format!("omarchy-themes-{slug}"),
            "Background Color": component(colors.background),
            "Foreground Color": component(colors.foreground),
            "Bold Color": component(colors.foreground),
            "Cursor Color": component(colors.cursor),
            "Cursor Text Color": component(colors.background),
            "Selection Color": component(colors.selection_background),
            "Selected Text Color": component(colors.foreground),
        });
        for (index, color) in colors.ansi.iter().enumerate() {
            profile[format!("Ansi {index} Color")] = component(*color);
        }
        json!({ "Profiles": [profile] })
    }
}

fn component(color: RgbColor) -> Value {
    let (r, g, b) = color.unit_components();
    json!({ "Red Component": r, "Green Component": g, "Blue Component": b, "Alpha Component": 1, "Color Space": "sRGB" })
}

impl TerminalExporter for ITermExporter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "iTerm2"
    }

    fn is_installed(&self) -> bool {
        (self.environment.find_app)(BUNDLE_IDENTIFIER).is_some()
    }

    fn add_hint(&self) -> String {
        "Adds these colors to iTerm2 as a new profile. Your iTerm2 settings aren't changed.".into()
    }

    fn is_added(&self, slug: &str, _theme_name: &str) -> bool {
        self.file(slug).exists()
    }

    fn add(&self, slug: &str, theme_name: &str, colors: &TerminalColors) -> Result<()> {
        json::write_json(&self.file(slug), &Self::profile_json(slug, &self.scheme_name(theme_name), colors))
    }

    fn remove(&self, slug: &str, _theme_name: &str) -> Result<()> {
        Ok(json::remove_file_if_present(&self.file(slug))?)
    }

    fn location(&self, slug: &str, _theme_name: &str) -> Option<PathBuf> {
        Some(self.file(slug))
    }

    fn added_message(&self, scheme: &str) -> String {
        format!(
            "“{scheme}” is now an iTerm2 profile. In iTerm2, open Settings › Profiles and pick it for a new window, \
             or choose Other Actions › Set as Default to use it everywhere."
        )
    }

    fn removed_message(&self, scheme: &str) -> String {
        format!("“{scheme}” is no longer one of iTerm2's profiles.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminals::test_support::{FakeMachine, tokyo};

    #[test]
    fn add_writes_a_dynamic_profile_and_remove_deletes_it() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let iterm = ITermExporter::new(machine.environment.clone());
        assert!(!iterm.is_added("omarchy.tokyo-night", "Tokyo Night"));

        iterm.add("omarchy.tokyo-night", "Tokyo Night", &tokyo()).unwrap();

        let file = machine
            .temp
            .path()
            .join("home/Library/Application Support/iTerm2/DynamicProfiles/omarchy-themes-omarchy.tokyo-night.json");
        assert!(file.exists());
        assert!(iterm.is_added("omarchy.tokyo-night", "Tokyo Night"));

        iterm.remove("omarchy.tokyo-night", "Tokyo Night").unwrap();
        assert!(!iterm.is_added("omarchy.tokyo-night", "Tokyo Night"));
        iterm.remove("omarchy.tokyo-night", "Tokyo Night").unwrap(); // already gone: no error
    }

    #[test]
    fn profile_has_every_color_as_srgb_components() {
        let json = ITermExporter::profile_json("tokyo", "Tokyo Night (Omarchy)", &tokyo());
        let profile = &json["Profiles"][0];

        assert_eq!(profile["Name"], "Tokyo Night (Omarchy)");
        assert_eq!(profile["Guid"], "omarchy-themes-tokyo");
        let keys = (0..16)
            .map(|i| format!("Ansi {i} Color"))
            .chain(["Background Color", "Foreground Color", "Cursor Color", "Selection Color"].map(String::from));
        for key in keys {
            assert!(profile.get(&key).is_some(), "missing {key}");
        }
        let background = &profile["Background Color"];
        assert_eq!(background["Color Space"], "sRGB");
        assert!((background["Red Component"].as_f64().unwrap() - 0x1a as f64 / 255.0).abs() < 0.0001);
        assert!((background["Blue Component"].as_f64().unwrap() - 0x26 as f64 / 255.0).abs() < 0.0001);
        assert!((profile["Ansi 5 Color"]["Red Component"].as_f64().unwrap() - 80.0 / 255.0).abs() < 0.0001);
    }
}
