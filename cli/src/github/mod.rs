//! GitHub repo links and the one-API-call-per-theme client.

mod client;
mod repo_ref;

pub use client::{GitHubClient, GitHubError, RepoTree, RepoTreeItem};
pub(crate) use repo_ref::percent_decode;
pub use repo_ref::{RepoRef, escape};
