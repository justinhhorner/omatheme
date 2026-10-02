use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Local, TimeDelta, TimeZone, Utc};
use serde::Deserialize;

use super::repo_ref::{RepoRef, escape};
use crate::app_info;
use crate::net::{CacheOptions, CachedResponse, HttpCache, HttpResponse, reason_phrase};

#[derive(Debug, Clone, thiserror::Error)]
pub enum GitHubError {
    /// The repository doesn't exist, was made private, or is empty.
    #[error("The GitHub repository {repo} couldn't be found. It may have been renamed, deleted or made private.")]
    NotFound { repo: String },
    /// The API rate limit (60 an hour without a token) is used up.
    #[error("{}", rate_limit_message(*.resets_at))]
    RateLimited { resets_at: Option<DateTime<Utc>> },
    #[error("{path} wasn't found in {repo}.")]
    FileNotFound { path: String, repo: String },
    #[error("GitHub returned an unreadable file list for {repo}.")]
    UnreadableTree { repo: String },
    #[error("GitHub returned {status} {} for {repo}.", reason_phrase(*.status))]
    Http { status: u16, repo: String },
}

fn rate_limit_message(resets_at: Option<DateTime<Utc>>) -> String {
    match resets_at {
        Some(at) => format!(
            "GitHub's rate limit was reached. It resets at {}. Setting GITHUB_TOKEN raises the limit.",
            at.with_timezone(&Local).format("%-I:%M %p")
        ),
        None => "GitHub's rate limit was reached. Try again in a few minutes, or set GITHUB_TOKEN.".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoTreeItem {
    /// Path relative to the theme root.
    pub path: String,
    pub is_file: bool,
    pub size: Option<u64>,
}

impl RepoTreeItem {
    pub fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

#[derive(Clone)]
pub struct RepoTree {
    /// Paths relative to the theme root (the repo's sub-folder if the link had one).
    pub items: Vec<RepoTreeItem>,
    /// True when served from cache because GitHub couldn't be reached.
    pub is_stale: bool,
    pub stale_reason: Option<Arc<anyhow::Error>>,
}

impl RepoTree {
    pub fn files(&self) -> impl Iterator<Item = &RepoTreeItem> {
        self.items.iter().filter(|i| i.is_file)
    }

    pub fn find_file(&self, path: &str) -> Option<&RepoTreeItem> {
        self.files().find(|i| i.path.eq_ignore_ascii_case(path))
    }
}

/// Minimal GitHub access for theme repos: exactly one REST call per repo (the recursive tree), and
/// file contents from raw.githubusercontent.com, which isn't API-metered. Everything goes through
/// the HTTP cache, so revisits are free and work offline.
#[derive(Clone)]
pub struct GitHubClient {
    cache: Arc<HttpCache>,
    token: Option<String>,
}

impl GitHubClient {
    /// Repo contents are re-checked at most this often (ETag revalidation after that).
    pub const FRESH_FOR_SECONDS: i64 = 3600;

    pub fn new(cache: Arc<HttpCache>, token: Option<&str>) -> Self {
        let token = token.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
        GitHubClient { cache, token }
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    pub fn tree(&self, repo: &RepoRef) -> Result<RepoTree> {
        let mut headers = vec![
            // GitHub rejects API requests without a User-Agent.
            ("User-Agent".to_string(), app_info::user_agent()),
            ("Accept".to_string(), "application/vnd.github+json".to_string()),
            ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
        ];
        if let Some(token) = &self.token {
            headers.push(("Authorization".to_string(), format!("Bearer {token}")));
        }
        let mapped_repo = repo.clone();
        let response = self.cache.get(
            &Self::tree_url(repo),
            &CacheOptions {
                max_age: TimeDelta::seconds(Self::FRESH_FOR_SECONDS),
                headers,
                map_error: Some(Box::new(move |r| Some(map_api_error(r, &mapped_repo).into()))),
                ..Default::default()
            },
        )?;
        Ok(Self::parse_tree(&response, repo)?)
    }

    /// The cached file list for `repo` without touching the network, or None.
    pub fn cached_tree(&self, repo: &RepoRef) -> Option<RepoTree> {
        let response = self.cache.cached_response(&Self::tree_url(repo))?;
        Self::parse_tree(&response, repo).ok()
    }

    pub fn tree_url(repo: &RepoRef) -> String {
        format!(
            "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
            escape(&repo.owner),
            escape(&repo.name),
            escape(repo.effective_ref())
        )
    }

    fn parse_tree(response: &CachedResponse, repo: &RepoRef) -> Result<RepoTree, GitHubError> {
        #[derive(Deserialize)]
        struct TreeResponse {
            tree: Option<Vec<Entry>>,
        }
        #[derive(Deserialize)]
        struct Entry {
            path: Option<String>,
            #[serde(rename = "type")]
            kind: Option<String>,
            size: Option<u64>,
        }

        let unreadable = || GitHubError::UnreadableTree { repo: repo.full_name() };
        let tree: TreeResponse = crate::json::from_slice(&response.body).map_err(|_| unreadable())?;
        let entries = tree.tree.ok_or_else(unreadable)?;

        let prefix = repo.sub_path.as_ref().map(|s| if s.ends_with('/') { s.clone() } else { format!("{s}/") });
        let items = entries
            .into_iter()
            .filter_map(|entry| {
                let mut path = entry.path?;
                if let Some(prefix) = &prefix {
                    path = path.strip_prefix(prefix.as_str())?.to_string();
                }
                Some(RepoTreeItem { path, is_file: entry.kind.as_deref() == Some("blob"), size: entry.size })
            })
            .collect();

        Ok(RepoTree { items, is_stale: response.is_stale, stale_reason: response.error.clone() })
    }

    /// raw.githubusercontent.com URL for a path relative to the theme root.
    pub fn raw_url(repo: &RepoRef, theme_relative_path: &str) -> String {
        let path = repo.repo_path(theme_relative_path).split('/').map(escape).collect::<Vec<_>>().join("/");
        format!(
            "https://raw.githubusercontent.com/{}/{}/{}/{}",
            escape(&repo.owner),
            escape(&repo.name),
            escape(repo.effective_ref()),
            path
        )
    }

    pub fn raw_text(&self, repo: &RepoRef, theme_relative_path: &str) -> Result<String> {
        let not_found = GitHubError::FileNotFound { path: theme_relative_path.to_string(), repo: repo.full_name() };
        let response = self.cache.get(
            &Self::raw_url(repo, theme_relative_path),
            &CacheOptions {
                max_age: TimeDelta::seconds(Self::FRESH_FOR_SECONDS),
                map_error: Some(Box::new(move |r| (r.status == 404).then(|| not_found.clone().into()))),
                ..Default::default()
            },
        )?;
        Ok(response.text())
    }
}

pub(crate) fn map_api_error(response: &HttpResponse, repo: &RepoRef) -> GitHubError {
    let status = response.status;
    if status == 429 || (status == 403 && response.header("x-ratelimit-remaining") == Some("0")) {
        let resets_at = response
            .header("x-ratelimit-reset")
            .and_then(|v| v.trim().parse::<i64>().ok())
            .and_then(|s| Utc.timestamp_opt(s, 0).single());
        return GitHubError::RateLimited { resets_at };
    }
    // 409 = empty repository.
    if status == 404 || status == 409 {
        return GitHubError::NotFound { repo: repo.full_name() };
    }
    GitHubError::Http { status, repo: repo.full_name() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_urls_escape_path_segments() {
        let url = GitHubClient::raw_url(&RepoRef::with("o", "r", None, Some("my themes")), "backgrounds/1 dark.png");
        assert_eq!(url, "https://raw.githubusercontent.com/o/r/HEAD/my%20themes/backgrounds/1%20dark.png");
    }

    #[test]
    fn rate_limit_message_names_the_reset_time() {
        let error = GitHubError::RateLimited { resets_at: Utc.timestamp_opt(1_790_482_111, 0).single() };
        assert!(error.to_string().starts_with("GitHub's rate limit was reached. It resets at "));
    }
}
