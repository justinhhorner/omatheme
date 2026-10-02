use std::collections::HashMap;

use serde::Serialize;

use super::model::Palette;
use crate::color::RgbColor;

pub const ANSI_NAMES: [&str; 8] = ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White"];

/// The palette swatch name for ANSI color `index` (0-15): "Red", …, "Bright red". The parsers name
/// swatches this way and [`TerminalColors::from_palette`] looks them up by it.
pub fn swatch_name(index: usize) -> String {
    if index < 8 { ANSI_NAMES[index].to_string() } else { format!("Bright {}", ANSI_NAMES[index - 8].to_lowercase()) }
}

/// A theme's colors as a terminal color scheme: background, foreground, cursor, selection and the
/// 16 ANSI colors. For Omarchy's named colors.toml this follows Omarchy's own terminal template
/// (default/themed/alacritty.toml.tpl): black = background, white = foreground, bright black =
/// muted, bright white and the cursor = bright_foreground. Themes that ship the 16 colors
/// (color0..15 or alacritty.toml) use them as-is. Missing bright colors fall back to their normal
/// ones. The same mapping as both apps' `TerminalColors`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalColors {
    pub background: RgbColor,
    pub foreground: RgbColor,
    pub cursor: RgbColor,
    pub selection_background: RgbColor,
    /// 16 colors: black, red, green, yellow, blue, magenta, cyan, white, then the bright ones.
    pub ansi: Vec<RgbColor>,
}

impl TerminalColors {
    pub fn from_palette(palette: &Palette) -> Self {
        let mut swatches: HashMap<String, RgbColor> = HashMap::new();
        for swatch in &palette.swatches {
            swatches.entry(swatch.name.to_lowercase()).or_insert(swatch.color);
        }
        let swatch = |name: &str| swatches.get(&name.to_lowercase()).copied();

        // A neutral between background and foreground, for themes with neither muted nor a bright black.
        let between = mix(palette.background, palette.foreground, 0.35);

        let normal: Vec<RgbColor> = ANSI_NAMES
            .iter()
            .map(|&name| match name {
                "Black" => swatch("Black").unwrap_or(palette.background),
                "White" => swatch("White").unwrap_or(palette.foreground),
                _ => swatch(name).unwrap_or(palette.foreground),
            })
            .collect();
        let bright: Vec<RgbColor> = ANSI_NAMES
            .iter()
            .enumerate()
            .map(|(i, &name)| match name {
                "Black" => swatch("Bright black").or(palette.muted).unwrap_or(between),
                "White" => swatch("Bright white").or(palette.bright_foreground).unwrap_or(normal[i]),
                _ => swatch(&format!("Bright {}", name.to_lowercase())).unwrap_or(normal[i]),
            })
            .collect();

        TerminalColors {
            background: palette.background,
            foreground: palette.foreground,
            cursor: palette.cursor.or(palette.bright_foreground).unwrap_or(palette.foreground),
            selection_background: palette.selection.unwrap_or_else(|| mix(palette.background, palette.foreground, 0.2)),
            ansi: normal.into_iter().chain(bright).collect(),
        }
    }
}

/// Halves round to even, like .NET's `Math.Round`, so every client exports identical colors.
fn mix(a: RgbColor, b: RgbColor, t: f64) -> RgbColor {
    let channel = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round_ties_even() as u8;
    RgbColor::new(channel(a.r, b.r), channel(a.g, b.g), channel(a.b, b.b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::c;
    use crate::palette::{parse_alacritty, parse_colors_toml};
    use crate::test_support::fixture;

    /// Mirrors the apps' `TerminalColorsTests`, so every client exports the same colors.
    #[test]
    fn named_colors_toml_follows_omarchys_terminal_template() {
        let palette = parse_colors_toml(
            r##"
background = "#1a1b26"
foreground = "#a9b1d6"
muted = "#414868"
bright_foreground = "#c0caf5"
selection = "#292e42"
red = "#f7768e"
green = "#9ece6a"
yellow = "#e0af68"
blue = "#7aa2f7"
magenta = "#ad8ee6"
cyan = "#449dab"
orange = "#eb927b"
bright_red = "#ff7a93"
bright_blue = "#7da6ff"
"##,
        )
        .unwrap();
        assert_eq!(palette.muted, Some(c("#414868")));
        assert_eq!(palette.bright_foreground, Some(c("#c0caf5")));

        let t = TerminalColors::from_palette(&palette);

        assert_eq!(t.background, c("#1a1b26"));
        assert_eq!(t.foreground, c("#a9b1d6"));
        assert_eq!(t.cursor, c("#c0caf5")); // bright_foreground
        assert_eq!(t.selection_background, c("#292e42"));
        let expected: Vec<_> = [
            "#1a1b26", "#f7768e", "#9ece6a", "#e0af68", "#7aa2f7", "#ad8ee6", "#449dab", "#a9b1d6", "#414868",
            "#ff7a93", "#9ece6a", "#e0af68", "#7da6ff", "#ad8ee6", "#449dab", "#c0caf5",
        ]
        .into_iter()
        .map(c)
        .collect();
        assert_eq!(t.ansi, expected);
    }

    #[test]
    fn terminal_style_colors_toml_uses_its_16_colors_as_is() {
        let t = TerminalColors::from_palette(&parse_colors_toml(&fixture("colors-ansi.toml")).unwrap());

        assert_eq!(t.ansi.len(), 16);
        assert_eq!(t.ansi[0], c("#000000"));
        assert_eq!(t.ansi[1], c("#c8e967"));
        assert_eq!(t.ansi[8], c("#c53253"));
        assert_eq!(t.ansi[15], c("#11aeb3"));
        assert_eq!(t.cursor, c("#ff7f41"));
    }

    #[test]
    fn alacritty_uses_its_normal_and_bright_colors() {
        let t = TerminalColors::from_palette(&parse_alacritty(&fixture("alacritty.toml")).unwrap());

        assert_eq!(t.ansi[0], c("#1a0e0e"));
        assert_eq!(t.ansi[7], c("#f2e8e8"));
        assert_eq!(t.ansi[8], c("#2b1818")); // 0x-prefixed in the file
        assert_eq!(t.ansi[15], c("#fff1f1"));
        assert_eq!(t.cursor, c("#eaeaea"));
    }

    #[test]
    fn missing_colors_have_sensible_fallbacks() {
        let t = TerminalColors::from_palette(
            &parse_colors_toml("background = \"#000000\"\nforeground = \"#ffffff\"").unwrap(),
        );

        assert_eq!(t.ansi[0], c("#000000")); // black = background
        assert_eq!(t.ansi[1], c("#ffffff")); // no red: foreground
        assert_eq!(t.ansi[7], c("#ffffff")); // white = foreground
        assert_eq!(t.ansi[8], c("#595959")); // bright black: between background and foreground
        assert_eq!(t.ansi[15], c("#ffffff")); // bright white: white
        assert_eq!(t.cursor, c("#ffffff")); // cursor: foreground
        assert_eq!(t.selection_background, c("#333333"));
    }

    #[test]
    fn ansi_swatch_names_match_what_terminal_colors_looks_up() {
        let names: Vec<_> = (0..9).map(swatch_name).collect();
        assert_eq!(names, ["Black", "Red", "Green", "Yellow", "Blue", "Magenta", "Cyan", "White", "Bright black"]);
        assert_eq!(swatch_name(15), "Bright white");
    }

    #[test]
    fn mixing_rounds_halves_to_even_like_dotnet() {
        // 0 + (255 - 0) * 0.5 = 127.5 → 128 (even); 1 + (2 - 1) * 0.5 = 1.5 → 2; 2.5 → 2.
        assert_eq!(mix(c("#000000"), c("#ffffff"), 0.5), c("#808080"));
        assert_eq!(mix(RgbColor::new(1, 2, 0), RgbColor::new(2, 3, 5), 0.5), RgbColor::new(2, 2, 2));
    }
}
