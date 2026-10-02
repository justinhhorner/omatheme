//! omatheme: the Omarchy Themes apps as a command-line tool.
//!
//! The library holds all the logic (catalog, GitHub resolution, palettes, the theme store, applying
//! themes, terminal colors) so it's tested without the network or the OS; `main.rs` is a thin
//! wrapper around [`cli::run`]. It shares the GUI apps' data folder and file format
//! (docs/data-format.md), so a theme downloaded here shows up in the app and vice versa.

pub mod app_info;
pub mod cancel;
pub mod catalog;
pub mod cli;
pub mod color;
pub mod github;
pub mod json;
pub mod net;
pub mod palette;
pub mod paths;
pub mod platform;
pub mod resolver;
pub mod store;
pub mod terminals;
pub mod theming;

#[cfg(test)]
pub(crate) mod test_support;
