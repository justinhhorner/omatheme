//! Opaque sRGB colors, serialized as "#rrggbb".

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RgbColor {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        RgbColor { r, g, b }
    }

    /// Accepts "#rrggbb", "rrggbb", "0xrrggbb", "#rgb" and "#rrggbbaa" (alpha ignored).
    pub fn parse(text: &str) -> Option<Self> {
        let mut s = text.trim();
        if let Some(rest) = s.strip_prefix('#') {
            s = rest;
        } else if s.len() >= 2 && s[..2].eq_ignore_ascii_case("0x") {
            s = &s[2..];
        }
        if !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let hex6: String = match s.len() {
            3 => s.chars().flat_map(|c| [c, c]).collect(),
            6 | 8 => s[..6].to_string(),
            _ => return None,
        };
        let v = u32::from_str_radix(&hex6, 16).ok()?;
        Some(RgbColor::new((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Components in 0...1.
    pub fn unit_components(&self) -> (f64, f64, f64) {
        (f64::from(self.r) / 255.0, f64::from(self.g) / 255.0, f64::from(self.b) / 255.0)
    }

    /// WCAG relative luminance, 0 (black) to 1 (white).
    pub fn relative_luminance(&self) -> f64 {
        fn linear(channel: u8) -> f64 {
            let c = f64::from(channel) / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        }
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// True for colors that read as a light background.
    pub fn is_light(&self) -> bool {
        self.relative_luminance() > 0.4
    }

    /// Hue in degrees [0, 360), saturation and lightness in [0, 1].
    pub fn to_hsl(&self) -> (f64, f64, f64) {
        let (r, g, b) = self.unit_components();
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;
        if max == min {
            return (0.0, 0.0, l);
        }
        let d = max - min;
        let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
        let h = if max == r {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        (h * 60.0, s, l)
    }

    pub fn from_hsl(h: f64, s: f64, l: f64) -> Self {
        let s = s.clamp(0.0, 1.0);
        let l = l.clamp(0.0, 1.0);
        if s == 0.0 {
            let v = to_byte(l);
            return RgbColor::new(v, v, v);
        }
        let h = ((h % 360.0) + 360.0) % 360.0 / 360.0;
        let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
        let p = 2.0 * l - q;
        RgbColor::new(
            to_byte(hue_to_rgb(p, q, h + 1.0 / 3.0)),
            to_byte(hue_to_rgb(p, q, h)),
            to_byte(hue_to_rgb(p, q, h - 1.0 / 3.0)),
        )
    }
}

fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }
    if t < 1.0 / 6.0 {
        return p + (q - p) * 6.0 * t;
    }
    if t < 1.0 / 2.0 {
        return q;
    }
    if t < 2.0 / 3.0 {
        return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
    }
    p
}

/// Rounds halves to even, like .NET's `Math.Round`, so the CLI computes the same bytes as the apps.
fn to_byte(unit: f64) -> u8 {
    (unit.clamp(0.0, 1.0) * 255.0).round_ties_even() as u8
}

impl fmt::Display for RgbColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl Serialize for RgbColor {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for RgbColor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        RgbColor::parse(&text).ok_or_else(|| serde::de::Error::custom(format!("Invalid color '{text}'.")))
    }
}

#[cfg(test)]
pub(crate) fn c(hex: &str) -> RgbColor {
    RgbColor::parse(hex).unwrap_or_else(|| panic!("bad test color {hex}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_forms() {
        for (text, r, g, b) in [
            ("#7aa2f7", 0x7a, 0xa2, 0xf7),
            ("7AA2F7", 0x7a, 0xa2, 0xf7),
            ("0x7aa2f7", 0x7a, 0xa2, 0xf7),
            ("#fff", 0xff, 0xff, 0xff),
            ("#7aa2f7cc", 0x7a, 0xa2, 0xf7),
            ("  #000000 ", 0, 0, 0),
        ] {
            assert_eq!(RgbColor::parse(text), Some(RgbColor::new(r, g, b)), "{text}");
        }
    }

    #[test]
    fn rejects_non_colors() {
        for text in ["", "CellForeground", "#12345", "#gggggg", "+fffff", "#-12345"] {
            assert_eq!(RgbColor::parse(text), None, "{text}");
        }
    }

    #[test]
    fn serializes_as_hex_string() {
        let json = serde_json::to_string(&serde_json::json!({ "c": c("#7AA2F7") })).unwrap();
        assert!(json.contains("\"#7aa2f7\""));
        assert_eq!(serde_json::from_str::<RgbColor>("\"#7aa2f7\"").unwrap(), c("#7aa2f7"));
        assert!(serde_json::from_str::<RgbColor>("\"nope\"").is_err());
    }

    #[test]
    fn light_and_dark_backgrounds_are_classified() {
        assert!(!c("#1a1b26").is_light());
        assert!(c("#fdf6e3").is_light());
        assert!(c("#e1e2e7").is_light());
    }

    #[test]
    fn hsl_round_trips() {
        for hex in ["#7aa2f7", "#000000", "#ffffff", "#ff0000", "#1a1b26", "#c0caf5"] {
            let (h, s, l) = c(hex).to_hsl();
            assert_eq!(RgbColor::from_hsl(h, s, l), c(hex), "{hex}");
        }
    }
}
