//! The tool's identity, sent the same way as both apps' `AppInfo`.

/// "0.1.0", from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Sent with every request (GitHub rejects API requests without one).
pub fn user_agent() -> String {
    user_agent_for(VERSION)
}

pub fn user_agent_for(version: &str) -> String {
    format!("OmarchyThemes/{version} (+https://github.com/basecamp/omarchy)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_names_the_app_and_version_like_the_apps() {
        assert_eq!(user_agent_for("0.1.0"), "OmarchyThemes/0.1.0 (+https://github.com/basecamp/omarchy)");
        assert_eq!(user_agent(), user_agent_for(VERSION));
    }
}
