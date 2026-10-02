//! The TUI's state and key handling, without a terminal: keys change the state and return the
//! [`Effect`]s the runtime should carry out (network work, applying, files). Tested on its own.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::crossterm;

use super::super::commands::catalog::matches_search;
use super::super::commands::desktop::{choose_fit, preferred_wallpaper};
use super::super::context::{ThemeIndex, lacks_named_extras};
use crate::catalog::CatalogEntry;
use crate::palette::{AppearanceMode, Palette};
use crate::resolver::ThemeDetails;
use crate::store::{AppSettings, InstalledTheme};
use crate::theming::{ApplyOptions, DesktopCapabilities, SummaryKind, WallpaperFit};

/// How long the selection must rest before free lookups (screenshots, default themes) start, so
/// scrolling through the list doesn't start a request per row.
pub const DWELL: Duration = Duration::from_millis(250);

/// What the desktop allows here, and everything else the TUI shows that doesn't change.
#[derive(Debug, Clone)]
pub struct Environment {
    pub os_name: &'static str,
    pub dry_run: bool,
    pub test_data_folder: bool,
    /// None where applying isn't supported; the reason is in `unsupported`.
    pub capabilities: Option<DesktopCapabilities>,
    pub fits: Vec<WallpaperFit>,
    pub unsupported: Option<String>,
    /// Where the user changes what this OS doesn't let apps change.
    pub appearance_hint: Option<&'static str>,
    pub terminals: Vec<TerminalInfo>,
    /// Truecolor swatches and previews (off with NO_COLOR).
    pub colors: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub installed: bool,
    pub can_remove: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Downloaded,
    Included,
    Community,
}

impl Filter {
    pub fn next(self) -> Self {
        match self {
            Filter::All => Filter::Downloaded,
            Filter::Downloaded => Filter::Included,
            Filter::Included => Filter::Community,
            Filter::Community => Filter::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "all themes",
            Filter::Downloaded => "downloaded",
            Filter::Included => "included with Omarchy",
            Filter::Community => "community",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Wallpapers,
}

/// A theme's details from GitHub, for themes that aren't downloaded.
#[derive(Clone)]
pub enum Lookup {
    Loading,
    Loaded(Box<ThemeDetails>),
    Failed(String),
}

/// A decoded, downscaled image for the preview pane.
#[derive(Clone)]
pub enum Preview {
    Loading,
    Ready(image::RgbImage),
    Failed,
}

/// Where a preview comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewSource {
    File(std::path::PathBuf),
    Url(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyPlan {
    pub slug: String,
    pub wallpaper: Option<String>,
    pub options: ApplyOptions,
}

/// Work for the runtime.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    LoadCatalog {
        refresh: bool,
    },
    LoadDetails(CatalogEntry),
    LoadPreview {
        key: String,
        source: PreviewSource,
    },
    /// Downloads the theme, then applies `then_apply` if given.
    Download {
        entry: CatalogEntry,
        then_apply: Option<ApplyPlan>,
    },
    Apply(ApplyPlan),
    SetWallpaper {
        slug: String,
        file: String,
    },
    Restore,
    Remove {
        slug: String,
    },
    TerminalAdd {
        slug: String,
        app: &'static str,
    },
    TerminalRemove {
        slug: String,
        app: &'static str,
    },
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ApplyDialog {
    pub slug: String,
    pub name: String,
    pub downloaded: bool,
    pub wallpaper: Option<String>,
    /// The theme has wallpapers (unknown counts as yes until it's downloaded).
    pub has_wallpapers: bool,
    pub mode: Option<AppearanceMode>,
    pub accent: Option<crate::color::RgbColor>,
    pub options: ApplyOptions,
    /// 0 wallpaper, 1 light/dark, 2 accent.
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pending {
    SetWallpaper { slug: String, file: String },
    Restore,
    Remove { slug: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmDialog {
    pub title: String,
    pub lines: Vec<String>,
    pub pending: Pending,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalDialog {
    pub slug: String,
    pub name: String,
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dialog {
    Apply(ApplyDialog),
    Confirm(ConfirmDialog),
    Terminal(TerminalDialog),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub kind: SummaryKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadState {
    pub slug: String,
    pub name: String,
    pub file_index: usize,
    pub files: usize,
    pub bytes: u64,
    pub total: Option<u64>,
}

pub struct App {
    pub env: Environment,
    pub entries: Vec<CatalogEntry>,
    pub installed: HashMap<String, InstalledTheme>,
    pub lookups: HashMap<String, Lookup>,
    pub previews: HashMap<String, Preview>,
    pub settings: AppSettings,
    pub has_snapshot: bool,
    pub catalog_loading: bool,
    pub catalog_error: Option<String>,
    pub notices: Vec<String>,
    pub filter: Filter,
    pub search: String,
    pub searching: bool,
    /// Indices into `entries`: default themes first, then community, as filtered.
    pub visible: Vec<usize>,
    /// Index into `visible`.
    pub selected: usize,
    pub focus: Focus,
    pub wallpaper_row: usize,
    pub dialog: Option<Dialog>,
    pub message: Option<Message>,
    pub download: Option<DownloadState>,
    pub selected_at: Instant,
    /// The last theme selected, kept while a search matches nothing.
    last_slug: Option<String>,
    /// A catalog arrived (the first one selects the theme on the desktop).
    catalog_seen: bool,
    pub tick: usize,
}

impl App {
    pub fn new(env: Environment, settings: AppSettings, has_snapshot: bool, now: Instant) -> Self {
        App {
            env,
            entries: Vec::new(),
            installed: HashMap::new(),
            lookups: HashMap::new(),
            previews: HashMap::new(),
            settings,
            has_snapshot,
            catalog_loading: true,
            catalog_error: None,
            notices: Vec::new(),
            filter: Filter::All,
            search: String::new(),
            searching: false,
            visible: Vec::new(),
            selected: 0,
            focus: Focus::List,
            wallpaper_row: 0,
            dialog: None,
            message: None,
            download: None,
            selected_at: now,
            last_slug: None,
            catalog_seen: false,
            tick: 0,
        }
    }

    // MARK: Data from the runtime

    pub fn set_index(&mut self, index: ThemeIndex, now: Instant) {
        // The first time, start on the theme on the desktop.
        let keep = if self.catalog_seen {
            self.selected_entry().map(|e| e.slug.clone())
        } else {
            // Else the top of the catalog, not whichever download was listed before it arrived.
            self.last_slug = None;
            self.settings.last_applied_slug.clone()
        };
        self.catalog_seen = true;
        self.entries = index.entries;
        self.installed = index.installed.into_iter().map(|t| (t.slug.clone(), t)).collect();
        self.notices = index.notices;
        self.catalog_loading = false;
        self.catalog_error = None;
        self.refilter(keep, now);
    }

    pub fn set_installed(&mut self, themes: Vec<InstalledTheme>, now: Instant) {
        let keep = self.selected_entry().map(|e| e.slug.clone()).or_else(|| self.settings.last_applied_slug.clone());
        for theme in &themes {
            if !self.entries.iter().any(|e| e.slug == theme.slug) {
                self.entries.push(super::super::context::entry_for(theme));
            }
        }
        self.installed = themes.into_iter().map(|t| (t.slug.clone(), t)).collect();
        self.refilter(keep, now);
    }

    pub fn set_settings(&mut self, settings: AppSettings) {
        self.settings = settings;
    }

    pub fn say(&mut self, kind: SummaryKind, text: impl Into<String>) {
        self.message = Some(Message { kind, text: text.into() });
    }

    // MARK: Derived state

    pub fn selected_entry(&self) -> Option<&CatalogEntry> {
        self.visible.get(self.selected).map(|&i| &self.entries[i])
    }

    pub fn installed_theme(&self, slug: &str) -> Option<&InstalledTheme> {
        self.installed.get(slug)
    }

    pub fn current_slug(&self) -> Option<&str> {
        self.settings.last_applied_slug.as_deref()
    }

    pub fn details(&self, slug: &str) -> Option<&ThemeDetails> {
        match self.lookups.get(slug) {
            Some(Lookup::Loaded(details)) => Some(details),
            _ => None,
        }
    }

    /// The selected theme's palette: from disk, else from GitHub once looked up.
    pub fn palette(&self, slug: &str) -> Option<&Palette> {
        let saved = self.installed.get(slug).and_then(|t| t.palette.as_ref());
        let looked_up = self.details(slug).and_then(|d| d.palette.as_ref());
        match saved {
            // Older downloads lack Omarchy's named extras; a fresh lookup has them.
            Some(saved) if lacks_named_extras(saved) => looked_up.or(Some(saved)),
            Some(saved) => Some(saved),
            None => looked_up,
        }
    }

    pub fn mode(&self, slug: &str) -> Option<AppearanceMode> {
        self.installed.get(slug).map(|t| t.mode).or_else(|| self.details(slug).map(|d| d.mode))
    }

    /// The selected theme's wallpapers: file names, with sizes when known.
    pub fn wallpapers(&self, slug: &str) -> Option<Vec<(String, Option<u64>)>> {
        if let Some(theme) = self.installed.get(slug) {
            return Some(
                theme
                    .wallpapers
                    .iter()
                    .map(|w| (w.clone(), std::fs::metadata(theme.wallpaper_path(w)).ok().map(|m| m.len())))
                    .collect(),
            );
        }
        self.details(slug).map(|d| d.wallpapers.iter().map(|w| (w.file_name().to_string(), w.size)).collect())
    }

    /// The wallpaper under the cursor in the details pane.
    pub fn wallpaper_at_cursor(&self, slug: &str) -> Option<String> {
        self.wallpapers(slug).and_then(|w| w.get(self.wallpaper_row).map(|(name, _)| name.clone()))
    }

    fn refilter(&mut self, keep: Option<String>, now: Instant) {
        let query = self.search.trim().to_lowercase();
        let mut visible: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| query.is_empty() || matches_search(e, &query))
            .filter(|(_, e)| match self.filter {
                Filter::All => true,
                Filter::Downloaded => self.installed.contains_key(&e.slug),
                Filter::Included => e.is_default_theme(),
                Filter::Community => !e.is_default_theme(),
            })
            .map(|(i, _)| i)
            .collect();
        // Default themes first, keeping catalog order within each section.
        visible.sort_by_key(|&i| !self.entries[i].is_default_theme());
        self.visible = visible;
        let keep = keep.or_else(|| self.last_slug.clone());
        let position = keep.and_then(|slug| self.visible.iter().position(|&i| self.entries[i].slug == slug));
        self.select(position.unwrap_or(0), now);
    }

    fn select(&mut self, index: usize, now: Instant) {
        let before = self.selected_entry().map(|e| e.slug.clone());
        self.selected = index.min(self.visible.len().saturating_sub(1));
        if self.selected_entry().map(|e| e.slug.clone()) != before {
            self.selected_at = now;
            self.wallpaper_row = self.preferred_wallpaper_row();
            self.focus = Focus::List;
        }
        if let Some(entry) = self.selected_entry() {
            self.last_slug = Some(entry.slug.clone());
        }
    }

    /// The wallpaper on the desktop if this is the current theme, else the first.
    fn preferred_wallpaper_row(&self) -> usize {
        let Some(entry) = self.selected_entry() else { return 0 };
        let Some(theme) = self.installed.get(&entry.slug) else { return 0 };
        preferred_wallpaper(theme, &self.settings)
            .and_then(|w| theme.wallpapers.iter().position(|x| *x == w))
            .unwrap_or(0)
    }

    fn rested(&self, now: Instant) -> bool {
        now.duration_since(self.selected_at) >= DWELL
    }

    /// Free lookups worth starting now: a default theme's details (its repo tree is shared and
    /// cached, its files aren't API-metered) once the selection rests.
    pub fn wanted_details(&self, now: Instant) -> Option<CatalogEntry> {
        let entry = self.selected_entry()?;
        let wanted = entry.is_default_theme() && !self.installed.contains_key(&entry.slug);
        (wanted && self.rested(now) && !self.lookups.contains_key(&entry.slug)).then(|| entry.clone())
    }

    /// The preview the details pane shows: the wallpaper under the cursor when browsing a
    /// downloaded theme's wallpapers, else the theme's screenshot.
    pub fn preview_key(&self) -> Option<(String, PreviewSource)> {
        let entry = self.selected_entry()?;
        let theme = self.installed.get(&entry.slug);
        if self.focus == Focus::Wallpapers
            && let Some(theme) = theme
            && let Some(file) = theme.wallpapers.get(self.wallpaper_row)
        {
            return Some((format!("wallpaper:{}/{file}", entry.slug), PreviewSource::File(theme.wallpaper_path(file))));
        }
        let source = match theme.and_then(|t| t.screenshot_path()).filter(|p| p.exists()) {
            Some(path) => PreviewSource::File(path),
            None => PreviewSource::Url(entry.screenshot_url.clone()?),
        };
        Some((format!("screenshot:{}", entry.slug), source))
    }

    /// The preview to load now, if any (after the selection rests).
    pub fn wanted_preview(&self, now: Instant) -> Option<(String, PreviewSource)> {
        if !self.env.colors || !self.rested(now) {
            return None;
        }
        self.preview_key().filter(|(key, _)| !self.previews.contains_key(key))
    }

    // MARK: Keys

    pub fn handle_key(&mut self, key: KeyEvent, now: Instant) -> Vec<Effect> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return vec![Effect::Quit];
        }
        if self.dialog.is_some() {
            return self.dialog_key(key);
        }
        if self.searching {
            return self.search_key(key, now);
        }
        self.message = None;
        let page = 10;
        match key.code {
            KeyCode::Char('q') => return vec![Effect::Quit],
            KeyCode::Char('?') => self.dialog = Some(Dialog::Help),
            KeyCode::Char('/') => {
                self.searching = true;
                self.focus = Focus::List;
            }
            KeyCode::Esc if self.focus == Focus::Wallpapers => self.focus = Focus::List,
            KeyCode::Esc if !self.search.is_empty() => {
                self.search.clear();
                let keep = self.selected_entry().map(|e| e.slug.clone());
                self.refilter(keep, now);
            }
            KeyCode::Char('o') => {
                self.filter = self.filter.next();
                let keep = self.selected_entry().map(|e| e.slug.clone());
                self.refilter(keep, now);
                self.say(SummaryKind::Info, format!("Showing {}.", self.filter.label()));
            }
            KeyCode::Char('R') => {
                self.catalog_loading = true;
                return vec![Effect::LoadCatalog { refresh: true }];
            }
            KeyCode::Up | KeyCode::Char('k') => self.move_cursor(-1, now),
            KeyCode::Down | KeyCode::Char('j') => self.move_cursor(1, now),
            KeyCode::PageUp => self.move_cursor(-page, now),
            KeyCode::PageDown => self.move_cursor(page, now),
            KeyCode::Home | KeyCode::Char('g') => self.move_to(0, now),
            KeyCode::End | KeyCode::Char('G') => self.move_to(usize::MAX, now),
            KeyCode::Left | KeyCode::Char('h') => self.focus = Focus::List,
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Enter if self.focus == Focus::List => {
                return self.open_details();
            }
            KeyCode::Enter => return self.wallpaper_enter(),
            KeyCode::Char('d') => return self.download(),
            KeyCode::Char('a') => return self.open_apply(),
            KeyCode::Char('w') => return self.ask_set_wallpaper(),
            KeyCode::Char('t') => return self.open_terminal(),
            KeyCode::Char('x') => self.ask_remove(),
            KeyCode::Char('r') => self.ask_restore(),
            _ => {}
        }
        Vec::new()
    }

    fn search_key(&mut self, key: KeyEvent, now: Instant) -> Vec<Effect> {
        let keep = self.selected_entry().map(|e| e.slug.clone());
        match key.code {
            KeyCode::Esc => {
                self.searching = false;
                self.search.clear();
            }
            KeyCode::Enter => self.searching = false,
            KeyCode::Backspace => {
                self.search.pop();
            }
            KeyCode::Up => {
                self.move_cursor(-1, now);
                return Vec::new();
            }
            KeyCode::Down => {
                self.move_cursor(1, now);
                return Vec::new();
            }
            KeyCode::Char(c) => self.search.push(c),
            _ => return Vec::new(),
        }
        self.refilter(keep, now);
        Vec::new()
    }

    fn move_cursor(&mut self, delta: isize, now: Instant) {
        if self.focus == Focus::Wallpapers {
            let count = self.selected_entry().and_then(|e| self.wallpapers(&e.slug)).map_or(0, |w| w.len());
            if count > 0 {
                self.wallpaper_row = self.wallpaper_row.saturating_add_signed(delta).min(count - 1);
            }
            return;
        }
        self.select(self.selected.saturating_add_signed(delta), now);
    }

    fn move_to(&mut self, index: usize, now: Instant) {
        if self.focus == Focus::Wallpapers {
            let count = self.selected_entry().and_then(|e| self.wallpapers(&e.slug)).map_or(0, |w| w.len());
            self.wallpaper_row = index.min(count.saturating_sub(1));
        } else {
            self.select(index, now);
        }
    }

    /// Moves into the wallpapers, looking the theme up first if it isn't downloaded.
    fn open_details(&mut self) -> Vec<Effect> {
        let Some(entry) = self.selected_entry().cloned() else { return Vec::new() };
        if let Some(wallpapers) = self.wallpapers(&entry.slug) {
            if !wallpapers.is_empty() {
                self.focus = Focus::Wallpapers;
            }
            return Vec::new();
        }
        self.lookup(&entry)
    }

    fn lookup(&mut self, entry: &CatalogEntry) -> Vec<Effect> {
        match self.lookups.get(&entry.slug) {
            Some(Lookup::Loading) => Vec::new(),
            Some(Lookup::Loaded(_)) => Vec::new(),
            Some(Lookup::Failed(_)) | None => {
                self.lookups.insert(entry.slug.clone(), Lookup::Loading);
                vec![Effect::LoadDetails(entry.clone())]
            }
        }
    }

    /// Enter on a wallpaper: switch to it if this theme is on the desktop, else apply with it.
    fn wallpaper_enter(&mut self) -> Vec<Effect> {
        match self.selected_entry() {
            Some(entry)
                if self.current_slug() == Some(entry.slug.as_str()) && self.installed.contains_key(&entry.slug) =>
            {
                self.ask_set_wallpaper()
            }
            _ => self.open_apply(),
        }
    }

    fn download(&mut self) -> Vec<Effect> {
        let Some(entry) = self.selected_entry().cloned() else { return Vec::new() };
        if self.installed.contains_key(&entry.slug) {
            self.say(SummaryKind::Info, format!("{} is already downloaded.", entry.name));
            return Vec::new();
        }
        if let Some(download) = &self.download {
            self.say(SummaryKind::Info, format!("Wait for {} to finish downloading.", download.name));
            return Vec::new();
        }
        vec![Effect::Download { entry, then_apply: None }]
    }

    fn open_apply(&mut self) -> Vec<Effect> {
        let Some(entry) = self.selected_entry().cloned() else { return Vec::new() };
        if self.env.capabilities.is_none() {
            let reason = self.env.unsupported.clone().unwrap_or_else(|| "Applying isn't supported here.".into());
            self.say(SummaryKind::Error, reason);
            return Vec::new();
        }
        if self.download.is_some() {
            self.say(SummaryKind::Info, "Wait for the download to finish.");
            return Vec::new();
        }
        let installed = self.installed.get(&entry.slug);
        let wallpapers = self.wallpapers(&entry.slug);
        let wallpaper = if self.focus == Focus::Wallpapers {
            self.wallpaper_at_cursor(&entry.slug)
        } else {
            match installed {
                Some(theme) => preferred_wallpaper(theme, &self.settings),
                None => wallpapers.as_ref().and_then(|w| w.first().map(|(name, _)| name.clone())),
            }
        };
        let defaults = &self.settings.apply_defaults;
        let fit = choose_fit(None, defaults.fit, &self.env.fits).unwrap_or(WallpaperFit::Fill);
        let options = ApplyOptions { fit, ..defaults.clone() };
        self.dialog = Some(Dialog::Apply(ApplyDialog {
            slug: entry.slug.clone(),
            name: installed.map(|t| t.name.clone()).unwrap_or_else(|| entry.name.clone()),
            downloaded: installed.is_some(),
            has_wallpapers: wallpapers.as_ref().is_none_or(|w| !w.is_empty()),
            wallpaper,
            mode: self.mode(&entry.slug),
            accent: self.palette(&entry.slug).map(|p| p.accent),
            options,
            row: 0,
        }));
        Vec::new()
    }

    fn ask_set_wallpaper(&mut self) -> Vec<Effect> {
        let Some(entry) = self.selected_entry().cloned() else { return Vec::new() };
        if self.env.capabilities.is_none() {
            let reason = self.env.unsupported.clone().unwrap_or_default();
            self.say(SummaryKind::Error, reason);
            return Vec::new();
        }
        if self.current_slug() != Some(entry.slug.as_str()) || !self.installed.contains_key(&entry.slug) {
            self.say(SummaryKind::Info, "That isn't the theme on the desktop. Press a to apply it.");
            return Vec::new();
        }
        let Some(file) = self.wallpaper_at_cursor(&entry.slug) else { return Vec::new() };
        if self.settings.last_applied_wallpaper.as_deref() == Some(file.as_str()) {
            self.say(SummaryKind::Info, format!("{file} is already on the desktop."));
            return Vec::new();
        }
        self.dialog = Some(Dialog::Confirm(ConfirmDialog {
            title: "Change the wallpaper".into(),
            lines: vec![format!("Set {file} as the wallpaper?"), "Only the wallpaper changes.".into()],
            pending: Pending::SetWallpaper { slug: entry.slug, file },
        }));
        Vec::new()
    }

    fn open_terminal(&mut self) -> Vec<Effect> {
        let Some(entry) = self.selected_entry().cloned() else { return Vec::new() };
        if self.env.terminals.is_empty() {
            return Vec::new();
        }
        let default = self.default_terminal_row();
        let name = self.installed.get(&entry.slug).map(|t| t.name.clone()).unwrap_or_else(|| entry.name.clone());
        self.dialog = Some(Dialog::Terminal(TerminalDialog { slug: entry.slug.clone(), name, row: default }));
        // Look the colors up if they aren't on disk (or are from before the named extras).
        let saved = self.installed.get(&entry.slug).and_then(|t| t.palette.as_ref());
        if saved.is_none_or(lacks_named_extras) { self.lookup(&entry) } else { Vec::new() }
    }

    /// The terminal the macOS app picked (`terminalApp`), else this OS's default.
    pub fn default_terminal_row(&self) -> usize {
        let terminals = &self.env.terminals;
        let saved = self.settings.terminal_app.as_deref();
        terminals
            .iter()
            .position(|t| Some(t.id) == saved)
            .or_else(|| terminals.iter().position(|t| t.id == crate::terminals::default_id()))
            .unwrap_or(0)
    }

    fn ask_remove(&mut self) {
        let Some(entry) = self.selected_entry().cloned() else { return };
        let Some(theme) = self.installed.get(&entry.slug) else {
            self.say(SummaryKind::Info, format!("{} isn't downloaded.", entry.name));
            return;
        };
        let mut lines = vec![format!("Delete the downloaded copy of {}?", theme.name)];
        if self.current_slug() == Some(entry.slug.as_str()) {
            lines.push("It's the theme on your desktop: this deletes the wallpaper your desktop shows.".into());
        }
        self.dialog = Some(Dialog::Confirm(ConfirmDialog {
            title: "Remove download".into(),
            lines,
            pending: Pending::Remove { slug: entry.slug },
        }));
    }

    fn ask_restore(&mut self) {
        if self.env.capabilities.is_none() {
            let reason = self.env.unsupported.clone().unwrap_or_default();
            self.say(SummaryKind::Error, reason);
            return;
        }
        if !self.has_snapshot {
            self.say(SummaryKind::Info, "Nothing to restore: no saved desktop was found.");
            return;
        }
        self.dialog = Some(Dialog::Confirm(ConfirmDialog {
            title: "Restore my original desktop".into(),
            lines: vec!["Put back the desktop you had before the first apply?".into()],
            pending: Pending::Restore,
        }));
    }

    fn dialog_key(&mut self, key: KeyEvent) -> Vec<Effect> {
        let Some(dialog) = self.dialog.take() else { return Vec::new() };
        let close = matches!(key.code, KeyCode::Esc | KeyCode::Char('q'));
        match dialog {
            Dialog::Help => Vec::new(),
            _ if close => Vec::new(),
            Dialog::Confirm(confirm) => match key.code {
                KeyCode::Enter | KeyCode::Char('y') => vec![match confirm.pending {
                    Pending::SetWallpaper { slug, file } => Effect::SetWallpaper { slug, file },
                    Pending::Restore => Effect::Restore,
                    Pending::Remove { slug } => Effect::Remove { slug },
                }],
                KeyCode::Char('n') => Vec::new(),
                _ => {
                    self.dialog = Some(Dialog::Confirm(confirm));
                    Vec::new()
                }
            },
            Dialog::Apply(mut apply) => {
                let effects = self.apply_dialog_key(&mut apply, key);
                if effects.is_empty() {
                    self.dialog = Some(Dialog::Apply(apply));
                }
                effects
            }
            Dialog::Terminal(mut terminal) => {
                let count = self.env.terminals.len();
                let app = self.env.terminals[terminal.row.min(count - 1)].clone();
                let effect = match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        terminal.row = terminal.row.saturating_sub(1);
                        None
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        terminal.row = (terminal.row + 1).min(count - 1);
                        None
                    }
                    KeyCode::Enter | KeyCode::Char('a') if !app.installed => {
                        self.say(SummaryKind::Error, format!("Install {} to use these colors in it.", app.name));
                        None
                    }
                    KeyCode::Enter | KeyCode::Char('a') if self.palette(&terminal.slug).is_none() => {
                        self.say(SummaryKind::Info, "The theme's colors are still loading.");
                        None
                    }
                    KeyCode::Enter | KeyCode::Char('a') => {
                        Some(Effect::TerminalAdd { slug: terminal.slug.clone(), app: app.id })
                    }
                    KeyCode::Char('x') | KeyCode::Delete | KeyCode::Backspace => {
                        Some(Effect::TerminalRemove { slug: terminal.slug.clone(), app: app.id })
                    }
                    _ => None,
                };
                match effect {
                    Some(effect) => vec![effect],
                    None => {
                        self.dialog = Some(Dialog::Terminal(terminal));
                        Vec::new()
                    }
                }
            }
        }
    }

    /// Whether the dialog's row can be changed (the OS supports it and the theme has data).
    pub fn apply_row_enabled(&self, dialog: &ApplyDialog, row: usize) -> bool {
        let Some(capabilities) = self.env.capabilities else { return false };
        match row {
            0 => capabilities.wallpaper && dialog.has_wallpapers,
            1 => capabilities.appearance_mode,
            _ => capabilities.accent_color && (dialog.accent.is_some() || !dialog.downloaded),
        }
    }

    fn apply_dialog_key(&mut self, dialog: &mut ApplyDialog, key: KeyEvent) -> Vec<Effect> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => dialog.row = dialog.row.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => dialog.row = (dialog.row + 1).min(2),
            KeyCode::Char(' ') if self.apply_row_enabled(dialog, dialog.row) => {
                let flag = match dialog.row {
                    0 => &mut dialog.options.wallpaper,
                    1 => &mut dialog.options.appearance_mode,
                    _ => &mut dialog.options.accent_color,
                };
                *flag = !*flag;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('f') if dialog.row == 0 && !self.env.fits.is_empty() => {
                let fits = &self.env.fits;
                let i = fits.iter().position(|f| *f == dialog.options.fit).unwrap_or(0);
                let next =
                    if key.code == KeyCode::Left { (i + fits.len() - 1) % fits.len() } else { (i + 1) % fits.len() };
                dialog.options.fit = fits[next];
            }
            KeyCode::Enter | KeyCode::Char('y') => {
                let plan = ApplyPlan {
                    slug: dialog.slug.clone(),
                    wallpaper: dialog.wallpaper.clone(),
                    options: dialog.options.clone(),
                };
                if dialog.downloaded {
                    return vec![Effect::Apply(plan)];
                }
                let Some(entry) = self.entries.iter().find(|e| e.slug == dialog.slug).cloned() else {
                    return Vec::new();
                };
                return vec![Effect::Download { entry, then_apply: Some(plan) }];
            }
            _ => {}
        }
        Vec::new()
    }

    /// Downloaded themes' slugs (for the list markers).
    pub fn downloaded(&self) -> HashSet<&str> {
        self.installed.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::color::RgbColor;
    use crate::palette::PaletteSource;

    pub fn entry(slug: &str, name: &str) -> CatalogEntry {
        CatalogEntry {
            slug: slug.into(),
            name: name.into(),
            repo_url: format!("https://github.com/someone/{slug}"),
            screenshot_url: Some(format!("https://omarchy.org/assets/themes/{slug}.webp")),
        }
    }

    pub fn installed(slug: &str, name: &str, wallpapers: &[&str]) -> InstalledTheme {
        let c = |hex| RgbColor::parse(hex).unwrap();
        InstalledTheme {
            slug: slug.into(),
            name: name.into(),
            repo_url: format!("https://github.com/someone/{slug}"),
            palette: Some(Palette {
                muted: Some(c("#414868")),
                ..Palette::new(c("#1a1b26"), c("#a9b1d6"), c("#7aa2f7"), PaletteSource::ColorsToml)
            }),
            mode: AppearanceMode::Dark,
            wallpapers: wallpapers.iter().map(|w| w.to_string()).collect(),
            screenshot_file: None,
            downloaded_at: chrono::Utc::now(),
            directory: PathBuf::from(format!("/themes/{slug}")),
        }
    }

    pub fn env(capabilities: Option<DesktopCapabilities>) -> Environment {
        Environment {
            os_name: "macOS",
            dry_run: false,
            test_data_folder: false,
            capabilities,
            fits: vec![WallpaperFit::Fill, WallpaperFit::Fit, WallpaperFit::Stretch, WallpaperFit::Center],
            unsupported: capabilities.is_none().then(|| "Applying themes isn't supported on Linux.".to_string()),
            appearance_hint: Some("System Settings › Appearance"),
            terminals: vec![
                TerminalInfo { id: "iterm2", name: "iTerm2", installed: false, can_remove: true },
                TerminalInfo { id: "ghostty", name: "Ghostty", installed: true, can_remove: true },
            ],
            colors: true,
        }
    }

    /// Two default themes, two community themes; Tokyo Night downloaded and on the desktop.
    pub fn app() -> App {
        let now = Instant::now();
        let settings = AppSettings {
            last_applied_slug: Some("omarchy.tokyo-night".into()),
            last_applied_wallpaper: Some("2.png".into()),
            ..Default::default()
        };
        let mut app = App::new(env(Some(DesktopCapabilities::WALLPAPER)), settings, true, now);
        app.set_index(
            ThemeIndex {
                entries: vec![
                    entry("omarchy.catppuccin", "Catppuccin"),
                    entry("omarchy.tokyo-night", "Tokyo Night"),
                    entry("aetheria", "Aetheria"),
                    entry("vulkanite", "Vulkanite"),
                ],
                installed: vec![installed("omarchy.tokyo-night", "Tokyo Night", &["1.png", "2.png", "3.png"])],
                catalog: None,
                notices: Vec::new(),
            },
            now,
        );
        // The tests start at the top of the list.
        app.select(0, now);
        app
    }

    #[test]
    fn it_starts_on_the_theme_on_the_desktop() {
        let now = Instant::now();
        let settings = AppSettings { last_applied_slug: Some("vulkanite".into()), ..Default::default() };
        let mut app = App::new(env(Some(DesktopCapabilities::WALLPAPER)), settings, true, now);
        // Downloaded themes show first, then the catalog arrives.
        app.set_installed(vec![installed("omarchy.tokyo-night", "Tokyo Night", &["1.png"])], now);
        app.set_index(
            ThemeIndex {
                entries: vec![entry("omarchy.tokyo-night", "Tokyo Night"), entry("vulkanite", "Vulkanite")],
                installed: vec![],
                catalog: None,
                notices: vec![],
            },
            now,
        );
        assert_eq!(app.selected_entry().unwrap().slug, "vulkanite");

        // With no theme on the desktop, it starts at the top of the catalog.
        let mut app = App::new(env(Some(DesktopCapabilities::WALLPAPER)), AppSettings::default(), false, now);
        app.set_installed(vec![installed("vulkanite", "Vulkanite", &["1.png"])], now);
        app.set_index(
            ThemeIndex {
                entries: vec![entry("omarchy.tokyo-night", "Tokyo Night"), entry("vulkanite", "Vulkanite")],
                installed: vec![],
                catalog: None,
                notices: vec![],
            },
            now,
        );
        assert_eq!(app.selected_entry().unwrap().slug, "omarchy.tokyo-night");
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn press(app: &mut App, codes: &[KeyCode]) -> Vec<Effect> {
        codes.iter().flat_map(|c| app.handle_key(key(*c), Instant::now())).collect()
    }

    fn selected(app: &App) -> &str {
        &app.selected_entry().unwrap().slug
    }

    #[test]
    fn the_list_puts_default_themes_first_and_moves_within_bounds() {
        let mut app = app();
        assert_eq!(selected(&app), "omarchy.catppuccin");
        press(&mut app, &[KeyCode::Down, KeyCode::Down]);
        assert_eq!(selected(&app), "aetheria");
        press(&mut app, &[KeyCode::End, KeyCode::Down]);
        assert_eq!(selected(&app), "vulkanite");
        press(&mut app, &[KeyCode::Home, KeyCode::Up]);
        assert_eq!(selected(&app), "omarchy.catppuccin");
    }

    #[test]
    fn search_filters_as_you_type_and_escape_clears_it() {
        let mut app = app();
        press(&mut app, &[KeyCode::Char('/'), KeyCode::Char('v'), KeyCode::Char('u')]);
        assert!(app.searching);
        assert_eq!(app.visible.len(), 1);
        assert_eq!(selected(&app), "vulkanite");
        // Typing doesn't trigger commands while searching.
        assert!(press(&mut app, &[KeyCode::Char('q')]).is_empty());
        press(&mut app, &[KeyCode::Esc]);
        assert!(!app.searching);
        assert_eq!(app.visible.len(), 4);
        assert_eq!(selected(&app), "vulkanite");
    }

    #[test]
    fn the_filter_cycles_through_downloaded_included_and_community() {
        let mut app = app();
        press(&mut app, &[KeyCode::Char('o')]);
        assert_eq!(app.filter, Filter::Downloaded);
        assert_eq!(app.visible.len(), 1);
        press(&mut app, &[KeyCode::Char('o')]);
        assert_eq!(app.visible.len(), 2);
        press(&mut app, &[KeyCode::Char('o')]);
        assert!(app.visible.iter().all(|&i| !app.entries[i].is_default_theme()));
        press(&mut app, &[KeyCode::Char('o')]);
        assert_eq!(app.filter, Filter::All);
    }

    #[test]
    fn opening_a_theme_that_isnt_downloaded_looks_it_up_once() {
        let mut app = app();
        press(&mut app, &[KeyCode::End]);
        let effects = press(&mut app, &[KeyCode::Enter]);
        assert_eq!(effects, [Effect::LoadDetails(entry("vulkanite", "Vulkanite"))]);
        assert!(matches!(app.lookups.get("vulkanite"), Some(Lookup::Loading)));
        assert!(press(&mut app, &[KeyCode::Enter]).is_empty());
    }

    #[test]
    fn a_downloaded_theme_opens_on_the_wallpaper_on_the_desktop() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        assert_eq!(app.focus, Focus::Wallpapers);
        assert_eq!(app.wallpaper_at_cursor("omarchy.tokyo-night").as_deref(), Some("2.png"));
        press(&mut app, &[KeyCode::Down, KeyCode::Down]);
        assert_eq!(app.wallpaper_row, 2);
        press(&mut app, &[KeyCode::Esc]);
        assert_eq!(app.focus, Focus::List);
    }

    #[test]
    fn default_themes_are_looked_up_only_after_the_selection_rests() {
        let mut app = app();
        let now = app.selected_at;
        assert_eq!(app.wanted_details(now), None);
        assert_eq!(app.wanted_details(now + DWELL), Some(entry("omarchy.catppuccin", "Catppuccin")));
        // Community themes cost an API call, so they wait for Enter.
        press(&mut app, &[KeyCode::End]);
        assert_eq!(app.wanted_details(Instant::now() + DWELL), None);
    }

    #[test]
    fn previews_show_the_screenshot_then_the_wallpaper_under_the_cursor() {
        let mut app = app();
        let (key, source) = app.preview_key().unwrap();
        assert_eq!(key, "screenshot:omarchy.catppuccin");
        assert_eq!(source, PreviewSource::Url("https://omarchy.org/assets/themes/omarchy.catppuccin.webp".into()));
        assert_eq!(app.wanted_preview(app.selected_at), None);

        press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        let (key, source) = app.preview_key().unwrap();
        assert_eq!(key, "wallpaper:omarchy.tokyo-night/2.png");
        assert_eq!(source, PreviewSource::File(PathBuf::from("/themes/omarchy.tokyo-night/wallpapers/2.png")));

        app.env.colors = false;
        assert_eq!(app.wanted_preview(Instant::now() + DWELL), None);
    }

    #[test]
    fn the_apply_dialog_starts_from_the_saved_defaults_and_skips_what_the_os_cant_do() {
        let mut app = app();
        app.settings.apply_defaults = ApplyOptions::new(true, true, false, WallpaperFit::Tile);
        press(&mut app, &[KeyCode::Down, KeyCode::Char('a')]);
        let Some(Dialog::Apply(dialog)) = app.dialog.clone() else { panic!("no dialog") };
        assert_eq!(dialog.wallpaper.as_deref(), Some("2.png"));
        assert_eq!(dialog.options.fit, WallpaperFit::Fill); // Tile isn't offered on macOS
        assert!(app.apply_row_enabled(&dialog, 0));
        assert!(!app.apply_row_enabled(&dialog, 1)); // light/dark isn't supported here

        // Space on an unsupported row changes nothing; the fit cycles through what's offered.
        press(&mut app, &[KeyCode::Down, KeyCode::Char(' '), KeyCode::Up, KeyCode::Right, KeyCode::Right]);
        let Some(Dialog::Apply(dialog)) = app.dialog.clone() else { panic!() };
        assert!(dialog.options.appearance_mode);
        assert_eq!(dialog.options.fit, WallpaperFit::Stretch);
        press(&mut app, &[KeyCode::Char(' ')]);

        let effects = press(&mut app, &[KeyCode::Enter]);
        assert_eq!(
            effects,
            [Effect::Apply(ApplyPlan {
                slug: "omarchy.tokyo-night".into(),
                wallpaper: Some("2.png".into()),
                options: ApplyOptions::new(false, true, false, WallpaperFit::Stretch),
            })]
        );
        assert!(app.dialog.is_none());
    }

    #[test]
    fn applying_a_theme_that_isnt_downloaded_downloads_it_first() {
        let mut app = app();
        press(&mut app, &[KeyCode::End, KeyCode::Char('a')]);
        let effects = press(&mut app, &[KeyCode::Enter]);
        match effects.as_slice() {
            [Effect::Download { entry, then_apply: Some(plan) }] => {
                assert_eq!(entry.slug, "vulkanite");
                assert_eq!(plan.slug, "vulkanite");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn escape_cancels_a_dialog_without_effects() {
        let mut app = app();
        press(&mut app, &[KeyCode::Char('a')]);
        assert!(press(&mut app, &[KeyCode::Esc]).is_empty());
        assert!(app.dialog.is_none());
        press(&mut app, &[KeyCode::Char('r')]);
        assert!(matches!(app.dialog, Some(Dialog::Confirm(_))));
        assert!(press(&mut app, &[KeyCode::Char('n')]).is_empty());
        assert!(app.dialog.is_none());
    }

    #[test]
    fn where_applying_isnt_supported_the_desktop_keys_say_why() {
        let now = Instant::now();
        let mut app = App::new(env(None), AppSettings::default(), false, now);
        app.set_index(
            ThemeIndex {
                entries: vec![entry("aetheria", "Aetheria")],
                installed: vec![],
                catalog: None,
                notices: vec![],
            },
            now,
        );
        for key in ['a', 'r'] {
            assert!(press(&mut app, &[KeyCode::Char(key)]).is_empty());
            assert!(app.dialog.is_none());
            assert!(app.message.as_ref().unwrap().text.contains("isn't supported"));
        }
    }

    #[test]
    fn switching_the_wallpaper_is_for_the_theme_on_the_desktop_and_asks_first() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        // The wallpaper already on the desktop.
        assert!(press(&mut app, &[KeyCode::Char('w')]).is_empty());
        assert!(app.message.as_ref().unwrap().text.contains("already on the desktop"));

        press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        assert!(matches!(app.dialog, Some(Dialog::Confirm(_))));
        let effects = press(&mut app, &[KeyCode::Char('y')]);
        assert_eq!(effects, [Effect::SetWallpaper { slug: "omarchy.tokyo-night".into(), file: "3.png".into() }]);

        // Another theme: `w` explains, Enter opens Apply.
        press(&mut app, &[KeyCode::Esc, KeyCode::Up, KeyCode::Char('w')]);
        assert!(app.dialog.is_none());
    }

    #[test]
    fn remove_and_restore_ask_first() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down, KeyCode::Char('x')]);
        let Some(Dialog::Confirm(confirm)) = app.dialog.clone() else { panic!() };
        assert!(confirm.lines.iter().any(|l| l.contains("theme on your desktop")));
        assert_eq!(press(&mut app, &[KeyCode::Enter]), [Effect::Remove { slug: "omarchy.tokyo-night".into() }]);

        press(&mut app, &[KeyCode::Char('r')]);
        assert_eq!(press(&mut app, &[KeyCode::Enter]), [Effect::Restore]);

        app.has_snapshot = false;
        press(&mut app, &[KeyCode::Char('r')]);
        assert!(app.dialog.is_none());

        // Not downloaded: nothing to remove.
        press(&mut app, &[KeyCode::End, KeyCode::Char('x')]);
        assert!(app.dialog.is_none());
    }

    #[test]
    fn the_terminal_dialog_adds_to_installed_terminals_only() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down, KeyCode::Char('t')]);
        let Some(Dialog::Terminal(dialog)) = app.dialog.clone() else { panic!() };
        assert_eq!(dialog.name, "Tokyo Night");
        // iTerm2 (the default) isn't installed here.
        assert!(press(&mut app, &[KeyCode::Enter]).is_empty());
        assert!(app.message.as_ref().unwrap().text.contains("Install iTerm2"));
        let effects = press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        assert_eq!(effects, [Effect::TerminalAdd { slug: "omarchy.tokyo-night".into(), app: "ghostty" }]);
        press(&mut app, &[KeyCode::Char('t')]);
        let effects = press(&mut app, &[KeyCode::Down, KeyCode::Char('x')]);
        assert_eq!(effects, [Effect::TerminalRemove { slug: "omarchy.tokyo-night".into(), app: "ghostty" }]);
    }

    #[test]
    fn the_terminal_dialog_looks_up_colors_that_arent_on_disk() {
        let mut app = app();
        press(&mut app, &[KeyCode::End]);
        let effects = press(&mut app, &[KeyCode::Char('t')]);
        assert_eq!(effects, [Effect::LoadDetails(entry("vulkanite", "Vulkanite"))]);
        let effects = press(&mut app, &[KeyCode::Down, KeyCode::Enter]);
        assert!(effects.is_empty());
        assert!(app.message.as_ref().unwrap().text.contains("still loading"));
    }

    #[test]
    fn downloading_skips_downloaded_themes_and_runs_one_at_a_time() {
        let mut app = app();
        press(&mut app, &[KeyCode::Down]);
        assert!(press(&mut app, &[KeyCode::Char('d')]).is_empty());
        press(&mut app, &[KeyCode::End]);
        assert_eq!(
            press(&mut app, &[KeyCode::Char('d')]),
            [Effect::Download { entry: entry("vulkanite", "Vulkanite"), then_apply: None }]
        );
        app.download = Some(DownloadState {
            slug: "vulkanite".into(),
            name: "Vulkanite".into(),
            file_index: 0,
            files: 3,
            bytes: 0,
            total: None,
        });
        press(&mut app, &[KeyCode::Up]);
        assert!(press(&mut app, &[KeyCode::Char('d')]).is_empty());
    }

    #[test]
    fn quitting_and_refreshing() {
        let mut app = app();
        assert_eq!(press(&mut app, &[KeyCode::Char('R')]), [Effect::LoadCatalog { refresh: true }]);
        assert_eq!(press(&mut app, &[KeyCode::Char('q')]), [Effect::Quit]);
        assert_eq!(
            app.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), Instant::now()),
            [Effect::Quit]
        );
    }

    #[test]
    fn the_selection_survives_a_catalog_reload() {
        let mut app = app();
        press(&mut app, &[KeyCode::End]);
        let index = ThemeIndex {
            entries: vec![entry("new-theme", "New"), entry("vulkanite", "Vulkanite")],
            installed: vec![],
            catalog: None,
            notices: vec![],
        };
        app.set_index(index, Instant::now());
        assert_eq!(selected(&app), "vulkanite");
    }
}
