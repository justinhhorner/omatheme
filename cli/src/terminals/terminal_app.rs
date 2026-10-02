use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use base64::Engine;

use super::{TerminalEnvironment, TerminalExportUnsupportedError, TerminalExporter, scheme_file_name};
use crate::color::RgbColor;
use crate::json;
use crate::palette::{ANSI_NAMES, TerminalColors};

pub const ID: &str = "terminal";
const BUNDLE_IDENTIFIER: &str = "com.apple.Terminal";

/// Terminal.app has no folder of add-on profiles. The exporter writes a `.terminal` profile (in the
/// data folder) and opens it with Terminal, which imports it as a profile and opens a window with
/// it. Taking it back out would mean editing Terminal's settings, which exporters never do, so
/// removal is left to the user (Terminal › Settings › Profiles).
pub struct TerminalAppExporter {
    environment: Arc<TerminalEnvironment>,
}

impl TerminalAppExporter {
    pub fn new(environment: Arc<TerminalEnvironment>) -> Self {
        TerminalAppExporter { environment }
    }

    fn file(&self, theme_name: &str) -> PathBuf {
        self.environment.exports_dir.join(format!("{}.terminal", scheme_file_name(theme_name)))
    }

    /// Terminal's keys for the 16 ANSI colors, in order.
    pub fn ansi_keys() -> Vec<String> {
        ANSI_NAMES
            .iter()
            .map(|n| format!("ANSI{n}Color"))
            .chain(ANSI_NAMES.iter().map(|n| format!("ANSIBright{n}Color")))
            .collect()
    }

    /// A `.terminal` file: an XML property list whose colors are keyed-archived NSColors.
    pub fn profile(&self, name: &str, colors: &TerminalColors) -> Result<String> {
        let archive = |c: RgbColor| (self.environment.archive_color)(c);
        let mut entries: Vec<(String, String)> = vec![
            ("BackgroundColor".into(), data(&archive(colors.background)?)),
            ("CursorColor".into(), data(&archive(colors.cursor)?)),
            ("ProfileCurrentVersion".into(), "<real>2.07</real>".into()),
            ("SelectionColor".into(), data(&archive(colors.selection_background)?)),
            ("TextBoldColor".into(), data(&archive(colors.foreground)?)),
            ("TextColor".into(), data(&archive(colors.foreground)?)),
            ("name".into(), format!("<string>{}</string>", xml_escape(name))),
            ("type".into(), "<string>Window Settings</string>".into()),
        ];
        for (key, color) in Self::ansi_keys().into_iter().zip(&colors.ansi) {
            entries.push((key, data(&archive(*color)?)));
        }
        entries.sort();

        let mut plist = String::from(concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n<dict>\n"
        ));
        for (key, value) in entries {
            plist += &format!("\t<key>{}</key>\n\t{value}\n", xml_escape(&key));
        }
        plist += "</dict>\n</plist>\n";
        Ok(plist)
    }
}

fn data(bytes: &[u8]) -> String {
    format!("<data>{}</data>", base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl TerminalExporter for TerminalAppExporter {
    fn id(&self) -> &'static str {
        ID
    }

    fn display_name(&self) -> &'static str {
        "Terminal"
    }

    fn is_installed(&self) -> bool {
        (self.environment.find_app)(BUNDLE_IDENTIFIER).is_some()
    }

    fn can_remove(&self) -> bool {
        false
    }

    fn add_hint(&self) -> String {
        "Opens these colors in Terminal, which adds them as a new profile. Your other profiles aren't changed.".into()
    }

    /// A read-only look at Terminal's own profile list.
    fn is_added(&self, _slug: &str, theme_name: &str) -> bool {
        let scheme = self.scheme_name(theme_name);
        (self.environment.terminal_profiles)().is_some_and(|profiles| profiles.contains(&scheme))
    }

    fn add(&self, _slug: &str, theme_name: &str, colors: &TerminalColors) -> Result<()> {
        let Some(app) = (self.environment.find_app)(BUNDLE_IDENTIFIER) else {
            return Err(TerminalExportUnsupportedError("Terminal isn't installed.".into()).into());
        };
        let file = self.file(theme_name);
        json::write_atomic(&file, self.profile(&self.scheme_name(theme_name), colors)?.as_bytes())?;
        (self.environment.open)(&file, &app)
    }

    fn remove(&self, _slug: &str, theme_name: &str) -> Result<()> {
        Err(TerminalExportUnsupportedError(self.remove_instructions(&self.scheme_name(theme_name))).into())
    }

    fn location(&self, _slug: &str, theme_name: &str) -> Option<PathBuf> {
        Some(self.file(theme_name))
    }

    fn added_message(&self, scheme: &str) -> String {
        format!(
            "Terminal opened a window with “{scheme}” and added it to its profiles. To use it for every new window, \
             open Terminal › Settings › Profiles, select it and click Default."
        )
    }

    fn removed_message(&self, _scheme: &str) -> String {
        String::new()
    }

    fn remove_instructions(&self, scheme: &str) -> String {
        format!(
            "Terminal profiles can only be removed in Terminal: open Terminal › Settings › Profiles, select “{scheme}” and click the − button."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminals::test_support::{FakeMachine, tokyo};

    #[test]
    fn add_writes_a_profile_and_opens_it_with_terminal() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let terminal = TerminalAppExporter::new(machine.environment.clone());

        terminal.add("omarchy.tokyo-night", "Tokyo Night", &tokyo()).unwrap();

        let (file, app) = machine.opened.lock().unwrap()[0].clone();
        assert_eq!(file.file_name().unwrap(), "Tokyo Night (Omarchy).terminal");
        assert_eq!(file.parent().unwrap().file_name().unwrap(), "terminal");
        assert_eq!(app.file_name().unwrap(), "com.apple.Terminal.app");

        let plist = std::fs::read_to_string(&file).unwrap();
        assert!(plist.contains("<key>name</key>\n\t<string>Tokyo Night (Omarchy)</string>"));
        assert!(plist.contains("<key>type</key>\n\t<string>Window Settings</string>"));
        for key in TerminalAppExporter::ansi_keys()
            .into_iter()
            .chain(["BackgroundColor", "TextColor", "CursorColor", "SelectionColor"].map(String::from))
        {
            assert!(plist.contains(&format!("<key>{key}</key>\n\t<data>")), "missing {key}");
        }
        // The fake archiver stores the hex, so the background's data decodes to it.
        let encoded = base64::engine::general_purpose::STANDARD.encode("#1a1b26");
        assert!(plist.contains(&format!("<key>BackgroundColor</key>\n\t<data>{encoded}</data>")));
    }

    #[test]
    fn is_added_reads_terminals_profiles_and_removal_is_manual() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let terminal = TerminalAppExporter::new(machine.environment.clone());
        assert!(!terminal.is_added("omarchy.tokyo-night", "Tokyo Night"));

        *machine.profiles.lock().unwrap() = Some(vec!["Basic".into(), "Tokyo Night (Omarchy)".into()]);

        assert!(terminal.is_added("omarchy.tokyo-night", "Tokyo Night"));
        assert!(!terminal.can_remove());
        assert!(
            terminal.remove("omarchy.tokyo-night", "Tokyo Night").err().unwrap().is::<TerminalExportUnsupportedError>()
        );
        assert!(terminal.remove_instructions("Tokyo Night (Omarchy)").contains("Settings › Profiles"));
    }

    #[test]
    fn add_fails_when_terminal_is_missing() {
        let machine = FakeMachine::new(&[]);
        let terminal = TerminalAppExporter::new(machine.environment.clone());
        assert!(terminal.add("t", "T", &tokyo()).err().unwrap().is::<TerminalExportUnsupportedError>());
        assert!(machine.opened.lock().unwrap().is_empty());
    }

    #[test]
    fn names_are_escaped_in_the_plist() {
        let machine = FakeMachine::new(&[BUNDLE_IDENTIFIER]);
        let plist =
            TerminalAppExporter::new(machine.environment.clone()).profile("R&D <Dark> (Omarchy)", &tokyo()).unwrap();
        assert!(plist.contains("<string>R&amp;D &lt;Dark&gt; (Omarchy)</string>"));
    }

    /// The real archiver: a keyed archive of an sRGB NSColor (read-only, no terminal involved).
    #[cfg(target_os = "macos")]
    #[test]
    fn the_real_archiver_makes_a_keyed_archive() {
        let bytes = crate::platform::mac_native::archive_color(RgbColor::new(0x1a, 0x1b, 0x26)).unwrap();
        assert!(bytes.starts_with(b"bplist00"));
        assert!(bytes.windows(b"NSColor".len()).any(|w| w == b"NSColor"));
    }
}
