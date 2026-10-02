//! Color math for Windows accent settings, platform-neutral so it's tested everywhere. Windows
//! stores colors as DWORDs in ABGR (DWM AccentColor) or ARGB (ColorizationColor) order, and the
//! Explorer AccentPalette as 8 RGBA entries.

use crate::color::RgbColor;

/// Lightness offsets for Light3..Light1 (towards white) and Dark1..Dark3 (towards black).
const STEPS: [f64; 3] = [0.7, 0.45, 0.2];

pub fn to_abgr(c: RgbColor, alpha: u8) -> u32 {
    (u32::from(alpha) << 24) | (u32::from(c.b) << 16) | (u32::from(c.g) << 8) | u32::from(c.r)
}

pub fn to_argb(c: RgbColor, alpha: u8) -> u32 {
    (u32::from(alpha) << 24) | (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

pub fn from_abgr(value: u32) -> RgbColor {
    RgbColor::new(value as u8, (value >> 8) as u8, (value >> 16) as u8)
}

/// Keeps a theme accent usable as a Windows accent. Near-black or near-white colors (common for
/// monochrome themes) would make Start and taskbar highlights unreadable.
pub fn normalize_accent(accent: RgbColor) -> RgbColor {
    let (h, s, l) = accent.to_hsl();
    let clamped = l.clamp(0.25, 0.75);
    if clamped == l { accent } else { RgbColor::from_hsl(h, s, clamped) }
}

/// Light3, Light2, Light1, Accent, Dark1, Dark2, Dark3: the seven shades Windows derives.
pub fn shades(accent: RgbColor) -> Vec<RgbColor> {
    let (h, s, l) = accent.to_hsl();
    let mut shades: Vec<RgbColor> = STEPS.iter().map(|k| RgbColor::from_hsl(h, s, l + (1.0 - l) * k)).collect();
    shades.push(accent);
    shades.extend(STEPS.iter().rev().map(|k| RgbColor::from_hsl(h, s, l * (1.0 - k))));
    shades
}

/// The 32-byte REG_BINARY for HKCU\…\Explorer\Accent\AccentPalette (8 × RGBA; the 8th is a neutral).
pub fn accent_palette_bytes(accent: RgbColor) -> Vec<u8> {
    let mut colors = shades(accent);
    colors.push(RgbColor::new(0x76, 0x76, 0x76));
    colors.iter().flat_map(|c| [c.r, c.g, c.b, 0x00]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::c;

    const ACCENT: RgbColor = RgbColor::new(0x12, 0x34, 0x56);

    #[test]
    fn packs_abgr_and_argb() {
        assert_eq!(to_abgr(ACCENT, 0xFF), 0xFF56_3412);
        assert_eq!(to_argb(ACCENT, 0xC4), 0xC412_3456);
        assert_eq!(from_abgr(to_abgr(ACCENT, 0xFF)), ACCENT);
    }

    #[test]
    fn shades_run_from_lightest_to_darkest_around_the_accent() {
        let accent = c("#0078d4");
        let shades = shades(accent);

        assert_eq!(shades.len(), 7);
        assert_eq!(shades[3], accent);
        let lightness: Vec<f64> = shades.iter().map(|s| s.to_hsl().2).collect();
        assert!(lightness.windows(2).all(|w| w[0] >= w[1]), "{lightness:?}");
    }

    #[test]
    fn accent_palette_is_32_bytes_of_rgba_with_accent_in_slot_3() {
        let bytes = accent_palette_bytes(c("#0078d4"));
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[12..16], [0x00, 0x78, 0xd4, 0x00]);
    }

    #[test]
    fn normalizes_unusable_accents_into_a_readable_lightness_range() {
        for hex in ["#000000", "#050505", "#ffffff", "#fdf6e3"] {
            let (_, _, l) = normalize_accent(c(hex)).to_hsl();
            assert!((0.24..=0.76).contains(&l), "{hex}: {l}");
        }
    }

    #[test]
    fn leaves_reasonable_accents_alone() {
        assert_eq!(normalize_accent(c("#7aa2f7")), c("#7aa2f7"));
    }
}
