//! The theme catalog: omarchy.org/themes plus Omarchy's default themes.

pub mod default_themes;
mod parser;
mod service;

pub use parser::{CatalogEntry, DEFAULT_PAGE_URL, parse};
pub use service::{CatalogFormatError, CatalogService, ThemeCatalog};
