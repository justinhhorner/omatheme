use std::collections::HashMap;

use super::model::{AppearanceMode, NamedColor, Palette, PaletteParseError, PaletteSource};
use super::terminal_colors::{ANSI_NAMES, swatch_name};
use super::{flat_toml, model};
use crate::catalog::default_themes::uppercase_first;
use crate::color::RgbColor;

const NAMED_KEYS: [&str; 14] = [
    "red",
    "orange",
    "yellow",
    "green",
    "cyan",
    "blue",
    "magenta",
    "brown",
    "bright_red",
    "bright_yellow",
    "bright_green",
    "bright_cyan",
    "bright_blue",
    "bright_magenta",
];

fn lookup(values: &HashMap<String, String>) -> impl Fn(&str) -> Option<RgbColor> + '_ {
    move |key| values.get(&key.to_lowercase()).and_then(|v| RgbColor::parse(v))
}

/// Omarchy's colors.toml. Two shapes exist in the wild:
/// - terminal-style: accent, cursor, foreground, background, selection_*, color0..color15;
/// - named (newer Omarchy): mode, accent, background, foreground, red, bright_red, … .
///
/// Both are accepted, and a file mixing them is fine.
pub fn parse_colors_toml(text: &str) -> Result<Palette, PaletteParseError> {
    let values = flat_toml::parse(text);
    let get = lookup(&values);

    let background =
        get("background").ok_or_else(|| PaletteParseError("colors.toml has no valid 'background' color.".into()))?;
    let foreground =
        get("foreground").ok_or_else(|| PaletteParseError("colors.toml has no valid 'foreground' color.".into()))?;

    let mut swatches: Vec<NamedColor> =
        (0..16).filter_map(|i| get(&format!("color{i}")).map(|c| NamedColor::new(swatch_name(i), c))).collect();
    if swatches.is_empty() {
        swatches = NAMED_KEYS.iter().filter_map(|key| get(key).map(|c| NamedColor::new(humanize(key), c))).collect();
    }

    Ok(Palette {
        background,
        foreground,
        accent: get("accent").or_else(|| get("color4")).or_else(|| get("blue")).unwrap_or(foreground),
        cursor: get("cursor"),
        selection: get("selection_background").or_else(|| get("selection")),
        muted: get("muted"),
        bright_foreground: get("bright_foreground"),
        declared_mode: parse_mode(values.get("mode").map(String::as_str)),
        swatches,
        source: PaletteSource::ColorsToml,
    })
}

pub(crate) fn parse_mode(mode: Option<&str>) -> Option<model::AppearanceMode> {
    match mode?.trim().to_lowercase().as_str() {
        "light" => Some(AppearanceMode::Light),
        "dark" => Some(AppearanceMode::Dark),
        _ => None,
    }
}

/// "bright_red" → "Bright red".
fn humanize(key: &str) -> String {
    uppercase_first(&key.replace('_', " "))
}

/// alacritty.toml, the fallback for themes that predate colors.toml.
pub fn parse_alacritty(text: &str) -> Result<Palette, PaletteParseError> {
    let values = flat_toml::parse(text);
    let get = lookup(&values);

    let background = get("colors.primary.background")
        .ok_or_else(|| PaletteParseError("alacritty.toml has no valid [colors.primary] background.".into()))?;
    let foreground = get("colors.primary.foreground")
        .ok_or_else(|| PaletteParseError("alacritty.toml has no valid [colors.primary] foreground.".into()))?;

    let swatches = (0..16)
        .filter_map(|i| {
            let group = if i < 8 { "normal" } else { "bright" };
            get(&format!("colors.{group}.{}", ANSI_NAMES[i % 8].to_lowercase()))
                .map(|c| NamedColor::new(swatch_name(i), c))
        })
        .collect();

    Ok(Palette {
        background,
        foreground,
        accent: get("colors.normal.blue").unwrap_or(foreground),
        cursor: get("colors.cursor.cursor"),
        selection: get("colors.selection.background"),
        muted: None,
        bright_foreground: None,
        declared_mode: None,
        swatches,
        source: PaletteSource::Alacritty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::c;
    use crate::test_support::fixture;

    #[test]
    fn parses_terminal_style_colors_toml() {
        let p = parse_colors_toml(&fixture("colors-ansi.toml")).unwrap();

        assert_eq!(p.background, c("#0e091d"));
        assert_eq!(p.foreground, c("#14b9b5"));
        assert_eq!(p.accent, c("#be3f50"));
        assert_eq!(p.cursor, Some(c("#ff7f41")));
        assert_eq!(p.selection, Some(c("#14b9b5")));
        assert_eq!(p.swatches.len(), 16);
        assert_eq!(p.swatches[0], NamedColor::new("Black", c("#000000")));
        assert_eq!(p.swatches[15].name, "Bright white");
        assert_eq!(p.declared_mode, None);
        assert_eq!(p.mode(), AppearanceMode::Dark);
        assert_eq!(p.source, PaletteSource::ColorsToml);
    }

    #[test]
    fn parses_named_colors_toml_with_explicit_mode() {
        let p = parse_colors_toml(&fixture("colors-named.toml")).unwrap();

        assert_eq!(p.declared_mode, Some(AppearanceMode::Light));
        assert_eq!(p.mode(), AppearanceMode::Light);
        assert_eq!(p.accent, c("#2e7de9"));
        assert_eq!(p.selection, Some(c("#b7c1e3")));
        let names: Vec<_> = p.swatches.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            ["Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Magenta", "Brown", "Bright red", "Bright blue"]
        );
    }

    #[test]
    fn accent_falls_back_to_color4_then_foreground() {
        let with_color4 =
            parse_colors_toml("background = \"#000000\"\nforeground = \"#ffffff\"\ncolor4 = \"#0000ff\"").unwrap();
        let bare = parse_colors_toml("background = \"#000000\"\nforeground = \"#eeeeee\"").unwrap();

        assert_eq!(with_color4.accent, c("#0000ff"));
        assert_eq!(bare.accent, c("#eeeeee"));
    }

    #[test]
    fn infers_light_mode_from_background_when_not_declared() {
        let p = parse_colors_toml("background = \"#fdf6e3\"\nforeground = \"#657b83\"").unwrap();
        assert_eq!(p.declared_mode, None);
        assert_eq!(p.mode(), AppearanceMode::Light);
    }

    #[test]
    fn falls_back_to_lenient_parsing_for_invalid_toml() {
        // Duplicate keys and a stray line make this invalid TOML, but the colors are still usable.
        let p = parse_colors_toml(
            "background = \"#101010\"\nforeground = \"#f0f0f0\"\nforeground = \"#e0e0e0\"\nthis line is not toml\naccent = '#ff0000'",
        )
        .unwrap();

        assert_eq!(p.background, c("#101010"));
        assert_eq!(p.foreground, c("#e0e0e0"));
        assert_eq!(p.accent, c("#ff0000"));
    }

    #[test]
    fn rejects_colors_toml_without_background_or_foreground() {
        for text in ["", "foreground = \"#ffffff\"", "background = \"not-a-color\"\nforeground = \"#ffffff\""] {
            assert!(parse_colors_toml(text).is_err(), "{text}");
        }
    }

    #[test]
    fn parses_alacritty_toml() {
        let p = parse_alacritty(&fixture("alacritty.toml")).unwrap();

        assert_eq!(p.background, c("#1b1112"));
        assert_eq!(p.foreground, c("#f2e8e8"));
        assert_eq!(p.accent, c("#d66b6b"));
        assert_eq!(p.cursor, Some(c("#eaeaea")));
        assert_eq!(p.selection, Some(c("#372223")));
        assert_eq!(p.swatches.len(), 16);
        assert_eq!(p.swatches[8], NamedColor::new("Bright black", c("#2b1818")));
        assert_eq!(p.source, PaletteSource::Alacritty);
    }

    #[test]
    fn rejects_alacritty_without_primary_colors() {
        assert!(parse_alacritty("[font]\nsize = 12").is_err());
    }
}
