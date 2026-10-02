//! The themes that ship with Omarchy, in the themes/ folder of its repo. They aren't listed on
//! omarchy.org/themes, so they're discovered from the repo's file tree: the same cached API call
//! that resolving any one of them uses, so opening a default theme costs no extra request.

use std::collections::HashSet;

use super::CatalogEntry;
use crate::github::{GitHubClient, RepoRef, RepoTree};
use crate::paths::is_valid_slug;

/// Prefixes default-theme slugs. Community slugs never contain a ".", so the two can't collide,
/// and a downloaded default theme keeps its folder name.
pub const SLUG_PREFIX: &str = "omarchy.";

const FOLDER: &str = "themes/";

/// Omarchy's repo (basecamp/omarchy redirects here). Its default branch (HEAD) is the current
/// release line.
pub fn repo() -> RepoRef {
    RepoRef::new("omacom", "omarchy")
}

pub fn is_default(slug: &str) -> bool {
    slug.starts_with(SLUG_PREFIX)
}

/// One entry per folder directly under themes/, sorted by name.
pub fn entries(tree: &RepoTree) -> Vec<CatalogEntry> {
    let files: HashSet<String> = tree.files().map(|f| f.path.to_lowercase()).collect();
    let repo = repo();

    let mut folders: Vec<&str> = tree
        .items
        .iter()
        .filter(|i| !i.is_file)
        .filter_map(|i| i.path.strip_prefix(FOLDER))
        .filter(|name| !name.contains('/') && is_valid_slug(&format!("{SLUG_PREFIX}{name}")))
        .collect();
    folders.sort();

    folders
        .into_iter()
        .map(|name| {
            let theme = RepoRef::with(&repo.owner, &repo.name, Some("HEAD"), Some(&format!("{FOLDER}{name}")));
            let preview = files
                .contains(&format!("{FOLDER}{name}/preview.png").to_lowercase())
                .then(|| GitHubClient::raw_url(&theme, "preview.png"));
            CatalogEntry {
                slug: format!("{SLUG_PREFIX}{name}"),
                name: display_name(name),
                repo_url: theme.html_url(),
                screenshot_url: preview,
            }
        })
        .collect()
}

/// Omarchy's own naming (omarchy-theme-list): "retro-82" → "Retro 82".
pub fn display_name(folder: &str) -> String {
    folder.split('-').filter(|s| !s.is_empty()).map(uppercase_first).collect::<Vec<_>>().join(" ")
}

/// "tokyo night" → "Tokyo night": only the first character changes.
pub fn uppercase_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_follow_omarchy_theme_list() {
        for (folder, name) in [
            ("tokyo-night", "Tokyo Night"),
            ("retro-82", "Retro 82"),
            ("white", "White"),
            ("flexoki-light", "Flexoki Light"),
        ] {
            assert_eq!(display_name(folder), name);
        }
    }

    #[test]
    fn uppercase_first_only_changes_the_first_character() {
        for (text, expected) in [("tokyo night", "Tokyo night"), ("x", "X"), ("", ""), ("éclair", "Éclair")] {
            assert_eq!(uppercase_first(text), expected);
        }
    }

    #[test]
    fn community_entries_are_not_default_themes() {
        let entry = CatalogEntry {
            slug: "aetheria".into(),
            name: "Aetheria".into(),
            repo_url: "https://github.com/JJDizz1L/aetheria".into(),
            screenshot_url: None,
        };
        assert!(!entry.is_default_theme());
        assert_eq!(entry.repo_display(), "JJDizz1L/aetheria");
    }
}
