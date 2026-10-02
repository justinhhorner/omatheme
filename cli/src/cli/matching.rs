//! Turning a `<THEME>` argument into one theme: an exact slug, then an exact name (or a default
//! theme's folder name), then a unique prefix of any of them. Ambiguous input lists the candidates.

use crate::catalog::default_themes::SLUG_PREFIX;

#[derive(Debug, thiserror::Error)]
pub enum MatchError {
    #[error("No theme matches '{0}'. Try `omatheme list --search {0}`.")]
    NotFound(String),
    #[error("'{query}' matches {} themes: {}. Use a slug to pick one.", .candidates.len(), .candidates.join(", "))]
    Ambiguous { query: String, candidates: Vec<String> },
}

/// Something with a slug and a name.
pub trait Named {
    fn slug(&self) -> &str;
    fn name(&self) -> &str;
}

impl Named for crate::catalog::CatalogEntry {
    fn slug(&self) -> &str {
        &self.slug
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for crate::store::InstalledTheme {
    fn slug(&self) -> &str {
        &self.slug
    }
    fn name(&self) -> &str {
        &self.name
    }
}

pub fn find<'a, T: Named>(items: &'a [T], query: &str) -> Result<&'a T, MatchError> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Err(MatchError::NotFound(query.to_string()));
    }
    let folder = |item: &T| item.slug().strip_prefix(SLUG_PREFIX).map(str::to_lowercase);

    let rules: [&dyn Fn(&T) -> bool; 3] = [
        &|item| item.slug().to_lowercase() == q,
        &|item| item.name().to_lowercase() == q || folder(item).is_some_and(|f| f == q),
        &|item| {
            item.slug().to_lowercase().starts_with(&q)
                || item.name().to_lowercase().starts_with(&q)
                || folder(item).is_some_and(|f| f.starts_with(&q))
        },
    ];
    for rule in rules {
        let matches: Vec<&T> = items.iter().filter(|item| rule(item)).collect();
        match matches.as_slice() {
            [] => continue,
            [one] => return Ok(one),
            many => {
                return Err(MatchError::Ambiguous {
                    query: query.to_string(),
                    candidates: many.iter().map(|m| format!("{} ({})", m.slug(), m.name())).collect(),
                });
            }
        }
    }
    Err(MatchError::NotFound(query.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::CatalogEntry;

    fn entry(slug: &str, name: &str) -> CatalogEntry {
        CatalogEntry { slug: slug.into(), name: name.into(), repo_url: String::new(), screenshot_url: None }
    }

    fn catalog() -> Vec<CatalogEntry> {
        vec![
            entry("omarchy.tokyo-night", "Tokyo Night"),
            entry("omarchy.catppuccin", "Catppuccin"),
            entry("omarchy.catppuccin-latte", "Catppuccin Latte"),
            entry("tokyo-night", "Tokyo Night"),
            entry("aetheria", "Aetheria"),
            entry("arc-blueberry", "Arc Blueberry"),
        ]
    }

    fn slug(query: &str) -> Result<String, MatchError> {
        find(&catalog(), query).map(|e| e.slug.clone())
    }

    #[test]
    fn an_exact_slug_wins() {
        assert_eq!(slug("tokyo-night").unwrap(), "tokyo-night");
        assert_eq!(slug("OMARCHY.TOKYO-NIGHT").unwrap(), "omarchy.tokyo-night");
        assert_eq!(slug("omarchy.catppuccin").unwrap(), "omarchy.catppuccin");
    }

    #[test]
    fn then_an_exact_name_or_default_theme_folder() {
        assert_eq!(slug("aetheria").unwrap(), "aetheria");
        assert_eq!(slug("catppuccin latte").unwrap(), "omarchy.catppuccin-latte");
        assert_eq!(slug("catppuccin").unwrap(), "omarchy.catppuccin");
    }

    #[test]
    fn then_a_unique_prefix() {
        assert_eq!(slug("aeth").unwrap(), "aetheria");
        assert_eq!(slug("arc").unwrap(), "arc-blueberry");
        assert_eq!(slug("Arc Blue").unwrap(), "arc-blueberry");
    }

    #[test]
    fn ambiguous_input_lists_the_candidates() {
        // Two themes are called "Tokyo Night".
        match slug("Tokyo Night") {
            Err(MatchError::Ambiguous { candidates, .. }) => {
                assert_eq!(candidates, ["omarchy.tokyo-night (Tokyo Night)", "tokyo-night (Tokyo Night)"])
            }
            other => panic!("{other:?}"),
        }
        let error = slug("tok").err().unwrap();
        assert!(error.to_string().contains("matches 2 themes"), "{error}");
    }

    #[test]
    fn nothing_matching_says_how_to_search() {
        let error = slug("zzz").err().unwrap();
        assert!(matches!(error, MatchError::NotFound(_)));
        assert!(error.to_string().contains("omatheme list --search zzz"));
        assert!(slug("  ").is_err());
    }
}
