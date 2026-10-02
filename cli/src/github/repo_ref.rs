/// A GitHub repository, optionally pinned to a branch/tag and a sub-folder (from links like
/// https://github.com/owner/repo/tree/main/themes/foo).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
    pub git_ref: Option<String>,
    pub sub_path: Option<String>,
}

const RESERVED_OWNERS: [&str; 10] =
    ["orgs", "settings", "sponsors", "topics", "marketplace", "explore", "features", "login", "search", "about"];

impl RepoRef {
    pub fn new(owner: &str, name: &str) -> Self {
        RepoRef { owner: owner.into(), name: name.into(), git_ref: None, sub_path: None }
    }

    pub fn with(owner: &str, name: &str, git_ref: Option<&str>, sub_path: Option<&str>) -> Self {
        RepoRef {
            owner: owner.into(),
            name: name.into(),
            git_ref: git_ref.map(Into::into),
            sub_path: sub_path.map(Into::into),
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Git ref used for API and raw URLs; HEAD resolves to the default branch.
    pub fn effective_ref(&self) -> &str {
        self.git_ref.as_deref().unwrap_or("HEAD")
    }

    pub fn html_url(&self) -> String {
        let mut url = format!("https://github.com/{}/{}", self.owner, self.name);
        if self.git_ref.is_some() || self.sub_path.is_some() {
            url += &format!("/tree/{}", encode_path(self.effective_ref()));
            if let Some(sub) = &self.sub_path {
                url += &format!("/{}", encode_path(sub));
            }
        }
        url
    }

    /// Maps a path relative to the theme root to a path relative to the repo root.
    pub fn repo_path(&self, theme_relative_path: &str) -> String {
        match &self.sub_path {
            Some(sub) => format!("{sub}/{theme_relative_path}"),
            None => theme_relative_path.to_string(),
        }
    }

    /// Parses a github.com repo link; None for anything else (profiles, /compare, /issues, other hosts).
    pub fn parse(url: &str) -> Option<Self> {
        let text = url.trim();
        let parsed = url::Url::parse(text).ok()?;
        if !matches!(parsed.scheme(), "https" | "http") {
            return None;
        }
        let host = parsed.host_str()?.to_ascii_lowercase();
        if host != "github.com" && host != "www.github.com" {
            return None;
        }

        // The raw path keeps "." and ".." segments (the url crate would resolve them), so
        // traversal attempts can be rejected.
        let after_scheme = &text[text.find("://")? + 3..];
        let path_start = after_scheme.find(['/', '?', '#']).unwrap_or(after_scheme.len());
        let path = &after_scheme[path_start..];
        let path = &path[..path.find(['?', '#']).unwrap_or(path.len())];
        let segments: Vec<String> = path.split('/').filter(|s| !s.is_empty()).map(percent_decode).collect();
        if segments.len() < 2 {
            return None;
        }

        let owner = &segments[0];
        let name = match segments[1].to_ascii_lowercase().strip_suffix(".git") {
            Some(_) => &segments[1][..segments[1].len() - 4],
            None => &segments[1],
        };
        if !is_valid_name(owner)
            || !is_valid_name(name)
            || RESERVED_OWNERS.contains(&owner.to_ascii_lowercase().as_str())
        {
            return None;
        }

        if segments.len() == 2 {
            return Some(RepoRef::new(owner, name));
        }

        // Only /tree/<ref>[/<sub/path>] is a repo view we understand; /compare, /issues etc. are not themes.
        if segments.len() < 4 || segments[2] != "tree" {
            return None;
        }
        let rest = &segments[4..];
        if rest.iter().any(|s| s == "." || s == "..") {
            return None;
        }
        let sub_path = if rest.is_empty() { None } else { Some(rest.join("/")) };
        Some(RepoRef::with(owner, name, Some(&segments[3]), sub_path.as_deref()))
    }
}

fn is_valid_name(s: &str) -> bool {
    (1..=100).contains(&s.chars().count())
        && s != "."
        && s != ".."
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

pub(crate) fn percent_decode(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
            out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| segment.to_string())
}

/// Percent-encodes everything except RFC 3986 unreserved characters (like .NET's EscapeDataString).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out += &format!("%{b:02X}");
        }
    }
    out
}

/// Escapes each segment of a "/"-separated path.
fn encode_path(path: &str) -> String {
    path.split('/').map(escape).collect::<Vec<_>>().join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_repo_links() {
        for (url, owner, name, git_ref, sub) in [
            ("https://github.com/JJDizz1L/aetheria", "JJDizz1L", "aetheria", None, None),
            ("https://github.com/owner/repo.git", "owner", "repo", None, None),
            ("https://www.github.com/owner/repo/", "owner", "repo", None, None),
            ("http://github.com/owner/repo", "owner", "repo", None, None),
            ("  https://github.com/owner/repo  ", "owner", "repo", None, None),
            ("https://github.com/owner/repo/tree/dev", "owner", "repo", Some("dev"), None),
            ("https://github.com/owner/mono/tree/main/themes/foo", "owner", "mono", Some("main"), Some("themes/foo")),
            ("https://github.com/owner/repo?tab=readme", "owner", "repo", None, None),
        ] {
            assert_eq!(RepoRef::parse(url), Some(RepoRef::with(owner, name, git_ref, sub)), "{url}");
        }
    }

    #[test]
    fn rejects_non_repo_links() {
        for url in [
            "",
            "not a url",
            "https://gitlab.com/owner/repo",
            "https://github.com/owner",
            "https://github.com/omacom/omarchy-site/compare",
            "https://github.com/owner/repo/issues/1",
            "https://github.com/owner/repo/tree",
            "https://github.com/orgs/basecamp",
            "https://github.com/owner/repo/tree/main/../../etc",
            "https://github.com/owner/repo/tree/main/%2e%2e/etc",
            "ftp://github.com/owner/repo",
        ] {
            assert_eq!(RepoRef::parse(url), None, "{url}");
        }
    }

    #[test]
    fn default_ref_is_head_and_paths_are_prefixed_with_sub_path() {
        let plain = RepoRef::new("o", "r");
        let nested = RepoRef::with("o", "r", Some("main"), Some("themes/foo"));

        assert_eq!(plain.effective_ref(), "HEAD");
        assert_eq!(plain.repo_path("colors.toml"), "colors.toml");
        assert_eq!(nested.repo_path("colors.toml"), "themes/foo/colors.toml");
        assert_eq!(plain.html_url(), "https://github.com/o/r");
        assert_eq!(nested.html_url(), "https://github.com/o/r/tree/main/themes/foo");
    }

    #[test]
    fn escape_keeps_only_unreserved_characters() {
        assert_eq!(escape("a b/c~d.e_f-g"), "a%20b%2Fc~d.e_f-g");
        assert_eq!(escape("é"), "%C3%A9");
    }
}
