use std::collections::HashSet;

use scraper::{ElementRef, Html, Selector};
use serde::Serialize;

use super::default_themes;
use crate::github::RepoRef;

pub const DEFAULT_PAGE_URL: &str = "https://omarchy.org/themes/";

/// One theme: a card from omarchy.org/themes, or one of Omarchy's default themes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// Stable local id (screenshot file stem, else derived from the repo name; `omarchy.<folder>`
    /// for default themes).
    pub slug: String,
    pub name: String,
    pub repo_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screenshot_url: Option<String>,
}

impl CatalogEntry {
    /// One of the themes that ship with Omarchy.
    pub fn is_default_theme(&self) -> bool {
        default_themes::is_default(&self.slug)
    }

    /// "owner/repo" for community themes; default themes all live in Omarchy's repo.
    pub fn repo_display(&self) -> String {
        if self.is_default_theme() {
            return "Included with Omarchy".to_string();
        }
        RepoRef::parse(&self.repo_url).map(|r| r.full_name()).unwrap_or_else(|| self.repo_url.clone())
    }
}

/// Parses the omarchy.org/themes gallery. The live page (an Astro build) renders each theme as
/// `<li><a href="https://github.com/…"><img src="/assets/themes/x.webp"><span>Name</span></a></li>`.
/// Rather than depend on classes or exact nesting, it looks for GitHub repo links that have a
/// screenshot, either inside the link or in the same `li`/`figure`. That also covers a
/// `<figure><img><figcaption><a>` layout.
pub fn parse(html: &str, page_url: &str) -> Vec<CatalogEntry> {
    let document = Html::parse_document(html);
    let anchors = Selector::parse("a[href]").expect("valid selector");
    let images = Selector::parse("img").expect("valid selector");
    let captions = Selector::parse("figcaption").expect("valid selector");

    let mut entries = Vec::new();
    let mut seen_repos = HashSet::new();
    let mut seen_slugs = HashSet::new();

    for anchor in document.select(&anchors) {
        let Some(repo) = anchor.value().attr("href").and_then(RepoRef::parse) else {
            continue;
        };

        let card = closest_card(anchor);
        // Text links such as "Share your theme" are not theme cards.
        let Some(img) = anchor.select(&images).next().or_else(|| card.and_then(|c| c.select(&images).next())) else {
            continue;
        };

        let repo_url = repo.html_url();
        if !seen_repos.insert(repo_url.to_lowercase()) {
            continue;
        }

        let screenshot = resolve(img.value().attr("src"), page_url);
        let caption = card.and_then(|c| c.select(&captions).next()).map(text_of);
        let alt = img.value().attr("alt").map(strip_screenshot_suffix);
        let name = [caption, Some(text_of(anchor)), alt]
            .into_iter()
            .flatten()
            .map(|s| collapse_whitespace(&s))
            .find(|s| !s.is_empty())
            .unwrap_or_else(|| repo.name.clone());

        let slug = unique(slug_for(screenshot.as_deref(), &repo), &mut seen_slugs);
        entries.push(CatalogEntry { slug, name, repo_url, screenshot_url: screenshot });
    }
    entries
}

fn closest_card(element: ElementRef<'_>) -> Option<ElementRef<'_>> {
    std::iter::once(element)
        .chain(element.ancestors().filter_map(ElementRef::wrap))
        .find(|e| matches!(e.value().name().to_ascii_lowercase().as_str(), "li" | "figure" | "article"))
}

fn text_of(element: ElementRef<'_>) -> String {
    element.text().collect()
}

fn collapse_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_screenshot_suffix(alt: &str) -> String {
    const SUFFIX: &str = " theme screenshot";
    if alt.to_lowercase().ends_with(SUFFIX) && alt.len() >= SUFFIX.len() {
        alt[..alt.len() - SUFFIX.len()].to_string()
    } else {
        alt.to_string()
    }
}

fn resolve(src: Option<&str>, page_url: &str) -> Option<String> {
    let src = src?.trim();
    if src.is_empty() {
        return None;
    }
    let url = url::Url::parse(page_url).ok()?.join(src).ok()?;
    matches!(url.scheme(), "https" | "http").then(|| url.to_string())
}

pub(crate) fn slug_for(screenshot: Option<&str>, repo: &RepoRef) -> String {
    if let Some(url) = screenshot.and_then(|s| url::Url::parse(s).ok()) {
        let last = url.path_segments().and_then(|mut s| s.next_back()).unwrap_or_default();
        let decoded = crate::github::percent_decode(last);
        let stem = match decoded.rfind('.') {
            Some(i) if i > 0 => &decoded[..i],
            _ => &decoded,
        };
        let from_screenshot = sanitize(stem);
        if !from_screenshot.is_empty() {
            return from_screenshot;
        }
    }

    let name = match &repo.sub_path {
        Some(sub) => sub.rsplit('/').next().unwrap_or(sub).to_string(),
        None => repo.name.clone(),
    };
    let mut slug = sanitize(&name);
    if slug.starts_with("omarchy-") && slug.len() > 8 {
        slug = slug[8..].to_string();
    }
    if slug.ends_with("-theme") && slug.len() > 6 {
        slug.truncate(slug.len() - 6);
    }
    if slug.is_empty() { "theme".to_string() } else { slug }
}

fn sanitize(s: &str) -> String {
    let mapped: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    mapped.trim_matches('-').to_string()
}

fn unique(slug: String, seen: &mut HashSet<String>) -> String {
    let mut candidate = slug.clone();
    let mut i = 2;
    while !seen.insert(candidate.to_lowercase()) {
        candidate = format!("{slug}-{i}");
        i += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::fixture;

    fn live() -> Vec<CatalogEntry> {
        parse(&fixture("catalog-live-structure.html"), DEFAULT_PAGE_URL)
    }

    fn by_slug(slug: &str) -> CatalogEntry {
        live().into_iter().find(|e| e.slug == slug).unwrap()
    }

    #[test]
    fn parses_live_omarchy_org_card_structure() {
        let entries = live();
        let slugs: Vec<_> = entries.iter().map(|e| e.slug.as_str()).collect();
        assert_eq!(slugs, ["aetheria", "amberbyte", "arc-blueberry", "all-hallows-eve"]);

        let aetheria = &entries[0];
        assert_eq!(aetheria.name, "Aetheria");
        assert_eq!(aetheria.repo_url, "https://github.com/JJDizz1L/aetheria");
        assert_eq!(aetheria.screenshot_url.as_deref(), Some("https://omarchy.org/assets/themes/aetheria.webp"));
    }

    #[test]
    fn normalizes_repo_urls_names_and_relative_screenshots() {
        assert_eq!(by_slug("amberbyte").repo_url, "https://github.com/tahfizhabib/omarchy-amberbyte-theme");
        assert_eq!(by_slug("arc-blueberry").name, "Arc Blueberry");
        assert_eq!(
            by_slug("arc-blueberry").screenshot_url.as_deref(),
            Some("https://omarchy.org/assets/themes/arc-blueberry.webp")
        );
        let hallows = by_slug("all-hallows-eve");
        assert_eq!(hallows.name, "All Hallow's Eve");
        assert_eq!(hallows.repo_url, "https://github.com/someone/monorepo/tree/main/themes/all-hallows-eve");
        assert_eq!(hallows.screenshot_url.as_deref(), Some("https://cdn.example.com/shots/all-hallows-eve.png"));
        assert_eq!(hallows.repo_display(), "someone/monorepo");
    }

    #[test]
    fn skips_nav_links_non_github_links_cards_without_screenshots_and_duplicates() {
        let entries = live();
        assert!(!entries.iter().any(|e| e.repo_url.contains("basecamp/omarchy")));
        assert!(!entries.iter().any(|e| e.repo_url.contains("omarchy-site")));
        assert!(!entries.iter().any(|e| e.name == "Not GitHub"));
        assert!(!entries.iter().any(|e| e.name == "No Screenshot"));
        assert_eq!(entries.iter().filter(|e| e.repo_url.ends_with("/aetheria")).count(), 1);
    }

    #[test]
    fn supports_figure_layout() {
        let entries = parse(&fixture("catalog-figure.html"), DEFAULT_PAGE_URL);
        assert_eq!(entries.len(), 3);

        assert_eq!(entries[0].slug, "tokyo-night");
        assert_eq!(entries[0].name, "Tokyo Night");
        assert_eq!(entries[0].repo_url, "https://github.com/someone/omarchy-tokyo-night-theme");

        assert_eq!(entries[1].slug, "rose-pine");
        assert_eq!(entries[1].name, "Rosé Pine");

        // No screenshot URL: slug falls back to the repo name minus omarchy-/-theme.
        assert_eq!(entries[2].slug, "no-src");
        assert_eq!(entries[2].screenshot_url, None);
    }

    #[test]
    fn makes_slugs_unique() {
        let html = r#"
            <ul>
              <li><a href="https://github.com/a/one"><img src="/assets/themes/same.webp"><span>One</span></a></li>
              <li><a href="https://github.com/b/two"><img src="/assets/themes/same.webp"><span>Two</span></a></li>
            </ul>"#;
        let slugs: Vec<_> = parse(html, DEFAULT_PAGE_URL).into_iter().map(|e| e.slug).collect();
        assert_eq!(slugs, ["same", "same-2"]);
    }

    #[test]
    fn returns_empty_for_pages_without_theme_cards() {
        for html in ["", "<html><body><p>Maintenance</p></body></html>", "<ul><li><a href=\"https://github.com/x/y\""] {
            assert!(parse(html, DEFAULT_PAGE_URL).is_empty(), "{html}");
        }
    }
}
