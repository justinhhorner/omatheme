use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::theming::WallpaperFit;

const ABOUT: &str = "Browse Omarchy's theme gallery and use a theme's look on this computer.";

const LONG_ABOUT: &str = "\
Browse the Omarchy theme gallery (https://omarchy.org/themes/) and the themes that ship with \
Omarchy, download them, and apply a theme's look to this computer: its wallpaper everywhere, \
light/dark and the accent color on Windows. It can also send a theme's colors to your terminal.

It shares its data folder with the Omarchy Themes app on the same computer, so themes downloaded \
in one show up in the other, and either can restore your original desktop.";

const CREDITS: &str = "\
Themes come from Omarchy (https://omarchy.org, https://github.com/basecamp/omarchy) and its \
community. omatheme isn't affiliated with Omarchy or 37signals.";

const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nThemes come from Omarchy (https://omarchy.org, https://github.com/basecamp/omarchy) and its community.",
    "\nomatheme isn't affiliated with Omarchy or 37signals."
);

#[derive(Debug, Parser)]
#[command(
    name = "omatheme",
    version,
    long_version = LONG_VERSION,
    about = ABOUT,
    long_about = LONG_ABOUT,
    after_help = CREDITS,
    propagate_version = true,
    max_term_width = 100
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Change nothing on the desktop or in terminals; show what would change (same as
    /// OMARCHY_THEMES_DRY_RUN=1)
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Use another data folder (same as OMARCHY_THEMES_DATA_DIR)
    #[arg(long, global = true, value_name = "PATH")]
    pub data_dir: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// List the themes: Omarchy's own first, then the community gallery
    #[command(visible_alias = "ls")]
    List(ListArgs),
    /// Show a theme's colors, terminal colors and wallpapers
    Show(ShowArgs),
    /// Download themes (wallpapers and colors) so they can be applied offline
    Download(DownloadArgs),
    /// Apply a theme to the desktop (downloading it first if needed)
    Apply(ApplyArgs),
    /// Show the theme on the desktop
    Current(JsonArgs),
    /// Switch to another wallpaper of the current theme
    Wallpaper(WallpaperArgs),
    /// Put back the desktop you had before the first apply
    Restore(RestoreArgs),
    /// Delete a downloaded theme
    #[command(visible_alias = "rm")]
    Remove(RemoveArgs),
    /// Send a theme's colors to a terminal app
    #[command(subcommand)]
    Terminal(TerminalCommand),
    /// Manage the download cache
    #[command(subcommand)]
    Cache(CacheCommand),
    /// Show where omatheme keeps its data, and whether an original desktop is saved
    Paths(JsonArgs),
    /// Browse, preview, download and apply themes in a full-screen terminal interface
    #[command(visible_alias = "ui")]
    Tui,
}

#[derive(Debug, Args)]
pub struct JsonArgs {
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Only themes whose name or repository contains TEXT
    #[arg(long, short, value_name = "TEXT")]
    pub search: Option<String>,
    /// Only downloaded themes
    #[arg(long, short)]
    pub downloaded: bool,
    /// Only the themes that ship with Omarchy
    #[arg(long = "default", conflicts_with = "community")]
    pub default_only: bool,
    /// Only community themes from omarchy.org
    #[arg(long)]
    pub community: bool,
    /// Check omarchy.org and GitHub for changes now (otherwise a catalog younger than 12 hours is used)
    #[arg(long)]
    pub refresh: bool,
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct ShowArgs {
    /// A slug, a name, or the start of either
    pub theme: String,
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct DownloadArgs {
    /// Slugs, names, or the start of either
    #[arg(required = true)]
    pub themes: Vec<String>,
    /// Download again even if already downloaded
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ApplyArgs {
    /// A slug, a name, or the start of either
    pub theme: String,
    /// Which wallpaper: its number in `omatheme show`, or its file name (default: the one last
    /// applied for this theme, else the first)
    #[arg(long, short, value_name = "N|NAME", conflicts_with = "no_wallpaper")]
    pub wallpaper: Option<String>,
    /// How the wallpaper fills the screen (default: the app's saved choice, else fill)
    #[arg(long, value_enum)]
    pub fit: Option<WallpaperFit>,
    /// Leave the wallpaper alone
    #[arg(long)]
    pub no_wallpaper: bool,
    /// Switch light/dark to match the theme (Windows)
    #[arg(long, overrides_with = "no_mode")]
    pub mode: bool,
    /// Leave light/dark alone
    #[arg(long, overrides_with = "mode")]
    pub no_mode: bool,
    /// Set the accent color from the theme (Windows)
    #[arg(long, overrides_with = "no_accent")]
    pub accent: bool,
    /// Leave the accent color alone
    #[arg(long, overrides_with = "accent")]
    pub no_accent: bool,
    /// Don't ask for confirmation
    #[arg(long, short)]
    pub yes: bool,
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct WallpaperArgs {
    /// The wallpaper's number or file name, or next/prev
    #[arg(value_name = "N|NAME|next|prev")]
    pub which: String,
    /// How the wallpaper fills the screen (default: the app's saved choice, else fill)
    #[arg(long, value_enum)]
    pub fit: Option<WallpaperFit>,
    /// Don't ask for confirmation
    #[arg(long, short)]
    pub yes: bool,
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RestoreArgs {
    /// Don't ask for confirmation
    #[arg(long, short)]
    pub yes: bool,
    /// Print JSON for scripts
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RemoveArgs {
    /// A downloaded theme's slug, name, or the start of either
    pub theme: String,
    /// Don't ask for confirmation (asked when it's the theme on the desktop)
    #[arg(long, short)]
    pub yes: bool,
}

#[derive(Debug, Subcommand)]
pub enum TerminalCommand {
    /// List the terminal apps omatheme can send colors to
    Apps(JsonArgs),
    /// Add a theme's colors to a terminal as a color scheme or profile
    Add(TerminalArgs),
    /// Remove a theme's colors from a terminal
    Remove(TerminalArgs),
}

#[derive(Debug, Args)]
pub struct TerminalArgs {
    /// A slug, a name, or the start of either
    pub theme: String,
    /// Which terminal (see `omatheme terminal apps`; default: the app's choice, else this OS's default)
    #[arg(long, value_name = "ID")]
    pub app: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum CacheCommand {
    /// Delete cached pages, GitHub file lists and palettes (downloaded themes stay)
    Clear,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn arguments_are_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn help_and_version_credit_omarchy() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();
        assert!(help.contains("https://omarchy.org"));
        assert!(help.contains("https://github.com/basecamp/omarchy"));
        assert!(help.contains("isn't affiliated with Omarchy or 37signals"));
        assert!(command.render_long_version().contains("isn't affiliated with Omarchy or 37signals"));
    }

    #[test]
    fn flags_parse_as_documented() {
        let cli = Cli::try_parse_from([
            "omatheme",
            "apply",
            "tokyo",
            "--wallpaper",
            "2",
            "--fit",
            "center",
            "--no-mode",
            "-y",
        ])
        .unwrap();
        let Command::Apply(args) = cli.command else { panic!() };
        assert_eq!(args.wallpaper.as_deref(), Some("2"));
        assert_eq!(args.fit, Some(WallpaperFit::Center));
        assert!(args.no_mode && args.yes && !args.no_accent);

        assert!(Cli::try_parse_from(["omatheme", "apply", "x", "--wallpaper", "1", "--no-wallpaper"]).is_err());
        assert!(Cli::try_parse_from(["omatheme", "list", "--default", "--community"]).is_err());
        assert!(Cli::try_parse_from(["omatheme", "download"]).is_err());
        let cli = Cli::try_parse_from(["omatheme", "--dry-run", "terminal", "add", "x", "--app", "ghostty"]).unwrap();
        assert!(cli.dry_run);
    }
}
