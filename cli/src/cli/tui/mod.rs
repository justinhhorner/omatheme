//! `omatheme tui`: the gallery as a full-screen terminal interface. The state and keys live in
//! [`app`], drawing in [`view`]; this runtime carries out their effects. Network work (catalog,
//! lookups, previews, downloads) runs on background threads so the interface stays responsive;
//! applying runs on the main thread, as AppKit requires.

mod app;
mod view;

use std::io::IsTerminal;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::TimeDelta;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::widgets::ListState;

use self::app::{App, ApplyPlan, Effect, Environment, Lookup, Preview, PreviewSource, TerminalInfo};
use super::commands::desktop::{apply_and_record, choose_fit, restore_and_record};
use super::context::{Context, ThemeIndex};
use super::{Outcome, UsageError};
use crate::cancel::{CancelToken, is_cancelled};
use crate::net::CacheOptions;
use crate::palette::TerminalColors;
use crate::resolver::ThemeDetails;
use crate::store::{DownloadProgress, InstalledTheme};
use crate::theming::{ApplyOptions, ApplyRequest, SummaryKind, ThemeApplier, WallpaperFit};

/// Previews are decoded once at about this size, then scaled to the pane when drawn.
const PREVIEW_SIZE: (u32, u32) = (320, 180);

/// What background jobs send back.
enum Message {
    Catalog(Result<ThemeIndex, String>),
    Details(String, Result<ThemeDetails, String>),
    Preview(String, Option<image::RgbImage>),
    Progress(app::DownloadState),
    Downloaded {
        slug: String,
        name: String,
        result: Result<InstalledTheme, String>,
        cancelled: bool,
        then_apply: Option<ApplyPlan>,
    },
}

pub fn run(ctx: &Context) -> Result<Outcome> {
    if !ctx.out.stdin_tty || !std::io::stdout().is_terminal() {
        return Err(UsageError("omatheme tui needs a terminal. The other commands work in scripts.".into()).into());
    }

    let applier = ctx.applier();
    let terminals = ctx
        .terminal_exporters()
        .iter()
        .map(|e| TerminalInfo {
            id: e.id(),
            name: e.display_name(),
            installed: e.is_installed(),
            can_remove: e.can_remove(),
        })
        .collect();
    let env = Environment {
        os_name: crate::platform::os_name(),
        dry_run: ctx.dry_run,
        test_data_folder: ctx.custom_data_dir,
        capabilities: applier.as_ref().ok().map(|a| a.capabilities()),
        fits: applier.as_ref().map(|a| a.supported_fits()).unwrap_or_default(),
        unsupported: applier.as_ref().err().map(|e| e.to_string()),
        appearance_hint: crate::platform::appearance_settings_hint(),
        terminals,
        colors: std::env::var_os("NO_COLOR").is_none_or(|v| v.is_empty()),
    };
    let has_snapshot = applier.as_ref().is_ok_and(|a| a.has_original_snapshot());
    let now = Instant::now();
    let mut app = App::new(env, ctx.settings().load(), has_snapshot, now);
    app.set_installed(ctx.store().list(), now);
    ctx.clean_up_store();

    let (tx, rx) = mpsc::channel();
    let mut runtime = Runtime { ctx, tx, rx, applier: applier.ok(), download: None };
    runtime.perform(&mut app, Effect::LoadCatalog { refresh: false });

    let mut terminal = ratatui::init();
    let result = runtime.event_loop(&mut terminal, &mut app);
    if let Some((cancel, handle)) = runtime.download.take() {
        // Let the download stop between chunks and remove its staging folder.
        cancel.cancel();
        let _ = terminal.draw(|frame| {
            app.say(SummaryKind::Info, "Stopping the download…");
            view::draw(frame, &app, &mut ListState::default());
        });
        let _ = handle.join();
    }
    ratatui::restore();
    result.map(|_| Outcome::Success)
}

struct Runtime<'a> {
    ctx: &'a Context,
    tx: Sender<Message>,
    rx: Receiver<Message>,
    applier: Option<ThemeApplier>,
    download: Option<(CancelToken, JoinHandle<()>)>,
}

impl Runtime<'_> {
    fn event_loop(&mut self, terminal: &mut DefaultTerminal, app: &mut App) -> Result<()> {
        let mut list_state = ListState::default();
        loop {
            terminal.draw(|frame| view::draw(frame, app, &mut list_state))?;

            let mut effects = Vec::new();
            if event::poll(Duration::from_millis(100))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                effects = app.handle_key(key, Instant::now());
            }
            app.tick = app.tick.wrapping_add(1);

            while let Ok(message) = self.rx.try_recv() {
                self.receive(app, message);
            }

            let now = Instant::now();
            if let Some(entry) = app.wanted_details(now) {
                effects.push(Effect::LoadDetails(entry));
            }
            if let Some((key, source)) = app.wanted_preview(now) {
                effects.push(Effect::LoadPreview { key, source });
            }
            for effect in effects {
                if effect == Effect::Quit {
                    return Ok(());
                }
                self.perform(app, effect);
            }
        }
    }

    fn spawn(&self, job: impl FnOnce(Sender<Message>) + Send + 'static) -> JoinHandle<()> {
        let tx = self.tx.clone();
        std::thread::spawn(move || job(tx))
    }

    fn perform(&mut self, app: &mut App, effect: Effect) {
        let ctx = self.ctx;
        match effect {
            Effect::Quit => {}
            Effect::LoadCatalog { refresh } => {
                app.catalog_loading = true;
                let loader = ctx.loader();
                self.spawn(move |tx| {
                    let _ = tx.send(Message::Catalog(loader.theme_index(refresh).map_err(|e| format!("{e:#}"))));
                });
            }
            Effect::LoadDetails(entry) => {
                app.lookups.insert(entry.slug.clone(), Lookup::Loading);
                let resolver = ctx.resolver();
                self.spawn(move |tx| {
                    let result = resolver.resolve(&entry).map_err(|e| format!("{e:#}"));
                    let _ = tx.send(Message::Details(entry.slug, result));
                });
            }
            Effect::LoadPreview { key, source } => {
                app.previews.insert(key.clone(), Preview::Loading);
                let cache = ctx.cache().clone();
                self.spawn(move |tx| {
                    let image = match source {
                        PreviewSource::File(path) => image::open(path).ok(),
                        PreviewSource::Url(url) => cache
                            .get(&url, &CacheOptions { max_age: TimeDelta::days(7), ..Default::default() })
                            .ok()
                            .and_then(|r| image::load_from_memory(&r.body).ok()),
                    };
                    let image = image.map(|i| i.thumbnail(PREVIEW_SIZE.0, PREVIEW_SIZE.1).into_rgb8());
                    let _ = tx.send(Message::Preview(key, image));
                });
            }
            Effect::Download { entry, then_apply } => {
                if self.download.is_some() {
                    return;
                }
                app.download = Some(app::DownloadState {
                    slug: entry.slug.clone(),
                    name: entry.name.clone(),
                    file_index: 0,
                    files: 0,
                    bytes: 0,
                    total: None,
                });
                let (resolver, store, cancel) = (ctx.resolver(), ctx.store(), CancelToken::new());
                let token = cancel.clone();
                let handle = self.spawn(move |tx| {
                    let result = resolver.resolve(&entry).and_then(|details| {
                        let sizes: Vec<Option<u64>> = details.wallpapers.iter().map(|w| w.size).collect();
                        let total = sizes.iter().copied().sum::<Option<u64>>();
                        let files = sizes.len();
                        let mut last = Instant::now() - Duration::from_secs(1);
                        let mut last_file = usize::MAX;
                        let mut report = |p: DownloadProgress| {
                            // At most ~20 updates a second, and every file change.
                            if p.file_index == last_file && last.elapsed() < Duration::from_millis(50) {
                                return;
                            }
                            (last, last_file) = (Instant::now(), p.file_index);
                            let before: u64 = sizes.iter().take(p.file_index).map(|s| s.unwrap_or(0)).sum();
                            let _ = tx.send(Message::Progress(app::DownloadState {
                                slug: entry.slug.clone(),
                                name: entry.name.clone(),
                                file_index: p.file_index,
                                files,
                                bytes: before + if p.file_index < files { p.bytes_received } else { 0 },
                                total,
                            }));
                        };
                        store.install(&details, &mut report, &token)
                    });
                    let cancelled = result.as_ref().err().is_some_and(is_cancelled);
                    let _ = tx.send(Message::Downloaded {
                        slug: entry.slug.clone(),
                        name: entry.name.clone(),
                        result: result.map_err(|e| format!("{e:#}")),
                        cancelled,
                        then_apply,
                    });
                });
                self.download = Some((cancel, handle));
            }
            Effect::Apply(plan) => self.apply(app, &plan.slug, plan.wallpaper.as_deref(), plan.options),
            Effect::SetWallpaper { slug, file } => {
                let fits = self.applier.as_ref().map(|a| a.supported_fits()).unwrap_or_default();
                let fit = choose_fit(None, app.settings.apply_defaults.fit, &fits).unwrap_or(WallpaperFit::Fill);
                self.apply(app, &slug, Some(&file), ApplyOptions::wallpaper_only(fit));
                if app.message.as_ref().is_some_and(|m| m.kind == SummaryKind::Success) {
                    let dry_run = if ctx.dry_run { " (Dry run: nothing changed.)" } else { "" };
                    app.say(SummaryKind::Success, format!("Wallpaper set to {file}.{dry_run}"));
                }
            }
            Effect::Restore => {
                let Some(applier) = &self.applier else { return };
                match restore_and_record(ctx, applier) {
                    Ok((true, settings_error)) => {
                        let mut text = "Original desktop restored. Your previous wallpaper is back.".to_string();
                        if ctx.dry_run {
                            text += " (Dry run: nothing changed.)";
                        }
                        if let Some(error) = settings_error {
                            text += &format!(" settings.json couldn't be saved: {error:#}");
                        }
                        app.say(SummaryKind::Success, text);
                    }
                    Ok((false, _)) => app.say(SummaryKind::Info, "Nothing to restore: no saved desktop was found."),
                    Err(error) => app.say(SummaryKind::Error, format!("Couldn't restore your desktop: {error:#}")),
                }
                app.set_settings(ctx.settings().load());
                app.has_snapshot = applier.has_original_snapshot();
            }
            Effect::Remove { slug } => {
                let name = app.installed_theme(&slug).map(|t| t.name.clone()).unwrap_or_else(|| slug.clone());
                match ctx.store().remove(&slug) {
                    Ok(()) => app.say(SummaryKind::Info, format!("Removed the download of {name}.")),
                    Err(error) => app.say(SummaryKind::Error, format!("Couldn't remove {name}: {error:#}")),
                }
                app.set_installed(ctx.store().list(), Instant::now());
            }
            Effect::TerminalAdd { slug, app: id } => self.terminal(app, &slug, id, true),
            Effect::TerminalRemove { slug, app: id } => self.terminal(app, &slug, id, false),
        }
    }

    fn receive(&mut self, app: &mut App, message: Message) {
        let now = Instant::now();
        match message {
            Message::Catalog(Ok(index)) => app.set_index(index, now),
            Message::Catalog(Err(error)) => {
                app.catalog_loading = false;
                app.catalog_error = Some(error.clone());
                app.say(SummaryKind::Error, error);
            }
            Message::Details(slug, Ok(details)) => {
                if let Some(reason) = &details.stale_reason {
                    app.say(SummaryKind::Warning, format!("Showing what was saved from GitHub earlier. {reason}"));
                }
                app.lookups.insert(slug, Lookup::Loaded(Box::new(details)));
            }
            Message::Details(slug, Err(error)) => {
                app.lookups.insert(slug, Lookup::Failed(error));
            }
            Message::Preview(key, image) => {
                app.previews.insert(key, image.map_or(Preview::Failed, Preview::Ready));
            }
            Message::Progress(state) => {
                if app.download.as_ref().is_some_and(|d| d.slug == state.slug) {
                    app.download = Some(state);
                }
            }
            Message::Downloaded { slug, name, result, cancelled, then_apply } => {
                app.download = None;
                if let Some((_, handle)) = self.download.take() {
                    let _ = handle.join();
                }
                app.set_installed(self.ctx.store().list(), now);
                match result {
                    Ok(theme) => {
                        let count = theme.wallpapers.len();
                        let wallpapers =
                            if count == 1 { "1 wallpaper".to_string() } else { format!("{count} wallpapers") };
                        app.say(SummaryKind::Success, format!("Downloaded {name}: {wallpapers}."));
                        if let Some(plan) = then_apply.filter(|p| p.slug == slug) {
                            self.apply(app, &plan.slug, plan.wallpaper.as_deref(), plan.options);
                        }
                    }
                    Err(_) if cancelled => app.say(SummaryKind::Info, "Download cancelled. Nothing was saved."),
                    Err(error) => app.say(SummaryKind::Error, format!("Couldn't download {name}: {error}")),
                }
            }
        }
    }

    /// Applies on the main thread and records it in settings.json, as `omatheme apply` does.
    fn apply(&mut self, app: &mut App, slug: &str, wallpaper: Option<&str>, options: ApplyOptions) {
        let ctx = self.ctx;
        let Some(applier) = &self.applier else { return };
        let Some(theme) = ctx.store().get(slug) else {
            app.say(SummaryKind::Error, "That theme isn't downloaded any more.");
            return;
        };
        let request = ApplyRequest::from_theme(&theme, wallpaper, options);
        match apply_and_record(ctx, applier, &theme, &request) {
            Ok(applied) => {
                let mut text = format!("{}. {}", applied.summary.title, applied.summary.message);
                if ctx.dry_run {
                    text += " (Dry run: nothing changed.)";
                }
                if let Some(error) = applied.settings_error {
                    text += &format!(" settings.json couldn't be saved: {error:#}");
                }
                app.say(applied.summary.kind, text);
            }
            Err(error) => app.say(SummaryKind::Error, format!("Couldn't apply {}: {error:#}", theme.name)),
        }
        app.set_settings(ctx.settings().load());
        app.has_snapshot = applier.has_original_snapshot();
    }

    fn terminal(&mut self, app: &mut App, slug: &str, id: &str, add: bool) {
        let ctx = self.ctx;
        let exporters = ctx.terminal_exporters();
        let Some(exporter) = exporters.iter().find(|e| e.id() == id) else { return };
        let name = app
            .installed_theme(slug)
            .map(|t| t.name.clone())
            .or_else(|| app.entries.iter().find(|e| e.slug == slug).map(|e| e.name.clone()))
            .unwrap_or_else(|| slug.to_string());
        let scheme = exporter.scheme_name(&name);
        let dry_run = if ctx.dry_run { " (Dry run: written in the data folder, nothing opened.)" } else { "" };

        if add {
            let Some(palette) = app.palette(slug).cloned() else { return };
            match exporter.add(slug, &name, &TerminalColors::from_palette(&palette)) {
                Ok(()) => app.say(
                    SummaryKind::Success,
                    format!("Added to {}. {}{dry_run}", exporter.display_name(), exporter.added_message(&scheme)),
                ),
                Err(error) => {
                    app.say(SummaryKind::Error, format!("Couldn't add to {}: {error:#}", exporter.display_name()))
                }
            }
        } else if !exporter.can_remove() {
            app.say(SummaryKind::Info, exporter.remove_instructions(&scheme));
        } else if !exporter.is_added(slug, &name) {
            app.say(SummaryKind::Info, format!("{} doesn't have “{scheme}”.", exporter.display_name()));
        } else {
            match exporter.remove(slug, &name) {
                Ok(()) => app.say(
                    SummaryKind::Info,
                    format!("Removed from {}. {}", exporter.display_name(), exporter.removed_message(&scheme)),
                ),
                Err(error) => app
                    .say(SummaryKind::Error, format!("Couldn't remove it from {}: {error:#}", exporter.display_name())),
            }
        }
    }
}
