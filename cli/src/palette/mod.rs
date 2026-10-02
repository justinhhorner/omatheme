//! Theme palettes: colors.toml (both shapes) and alacritty.toml, and the terminal color mapping.

pub mod flat_toml;
mod model;
mod parsers;
mod terminal_colors;

pub use model::{AppearanceMode, NamedColor, Palette, PaletteParseError, PaletteSource};
pub use parsers::{parse_alacritty, parse_colors_toml};
pub use terminal_colors::{ANSI_NAMES, TerminalColors, swatch_name};
