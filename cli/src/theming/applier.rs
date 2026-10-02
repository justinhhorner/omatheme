use std::sync::Mutex;

use anyhow::Result;

use super::backend::DesktopBackend;
use super::models::{ApplyRequest, ApplyResult, ApplyStep, DesktopCapabilities, StepOutcome, StepResult, WallpaperFit};
use super::snapshot::SnapshotStore;
use crate::cancel::{CancelToken, is_cancelled};

/// Applies a theme through a [`DesktopBackend`]. Before the first change it saves the current
/// desktop (so `restore` can undo everything), then runs each step independently: one failing step
/// doesn't stop the others, and the result reports exactly what happened per step.
pub struct ThemeApplier {
    backend: Box<dyn DesktopBackend>,
    snapshots: Box<dyn SnapshotStore>,
    gate: Mutex<()>,
}

#[derive(Debug, thiserror::Error)]
#[error("The wallpaper file is missing. Try downloading the theme again.")]
pub struct MissingWallpaperError;

impl ThemeApplier {
    pub fn new(backend: Box<dyn DesktopBackend>, snapshots: Box<dyn SnapshotStore>) -> Self {
        ThemeApplier { backend, snapshots, gate: Mutex::new(()) }
    }

    pub fn backend(&self) -> &dyn DesktopBackend {
        self.backend.as_ref()
    }

    pub fn capabilities(&self) -> DesktopCapabilities {
        self.backend.capabilities()
    }

    pub fn supported_fits(&self) -> Vec<WallpaperFit> {
        self.backend.supported_fits()
    }

    pub fn accent_color_note(&self) -> Option<&'static str> {
        self.backend.accent_color_note()
    }

    /// True when a snapshot was saved, including one that can't be read: restore then says why.
    pub fn has_original_snapshot(&self) -> bool {
        self.snapshots.load().map(|s| s.is_some()).unwrap_or(true)
    }

    pub fn apply(
        &self,
        request: &ApplyRequest,
        progress: &mut dyn FnMut(ApplyStep),
        cancel: &CancelToken,
    ) -> Result<ApplyResult> {
        cancel.check()?;
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let mut results = Vec::new();

        progress(ApplyStep::SaveOriginal);
        let saved = match self.snapshots.load() {
            Ok(saved) => saved,
            Err(error) => {
                // Saving now would replace the original with a desktop that may already be themed.
                return Ok(nothing_changed(format!(
                    "{error} Nothing was changed, so it isn't overwritten. To save your current desktop instead, delete that file."
                )));
            }
        };
        if saved.is_none() {
            match self.backend.capture().and_then(|snapshot| self.snapshots.save(&snapshot)) {
                Ok(()) => results.push(StepResult::new(ApplyStep::SaveOriginal, StepOutcome::Applied)),
                Err(error) if is_cancelled(&error) => return Err(error),
                // Without a snapshot it couldn't be undone, so change nothing.
                Err(error) => {
                    return Ok(nothing_changed(format!(
                        "Couldn't save your current desktop, so nothing was changed. {error}"
                    )));
                }
            }
        }

        let options = &request.options;
        let capabilities = self.backend.capabilities();

        results.push(self.run_step(
            ApplyStep::Wallpaper,
            options.wallpaper,
            capabilities.wallpaper,
            request.wallpaper.is_some(),
            progress,
            cancel,
            || {
                let path = request.wallpaper.as_ref().expect("checked");
                if !path.is_file() {
                    return Err(MissingWallpaperError.into());
                }
                self.backend.set_wallpaper(path, options.fit, request.background)
            },
        )?);

        results.push(self.run_step(
            ApplyStep::AppearanceMode,
            options.appearance_mode,
            capabilities.appearance_mode,
            true,
            progress,
            cancel,
            || self.backend.set_appearance_mode(request.mode),
        )?);

        results.push(self.run_step(
            ApplyStep::AccentColor,
            options.accent_color,
            capabilities.accent_color,
            request.accent.is_some(),
            progress,
            cancel,
            || self.backend.set_accent_color(request.accent.expect("checked")),
        )?);

        Ok(ApplyResult { steps: results })
    }

    #[allow(clippy::too_many_arguments)]
    fn run_step(
        &self,
        step: ApplyStep,
        enabled: bool,
        supported: bool,
        has_data: bool,
        progress: &mut dyn FnMut(ApplyStep),
        cancel: &CancelToken,
        action: impl FnOnce() -> Result<()>,
    ) -> Result<StepResult> {
        if !enabled {
            return Ok(StepResult::new(step, StepOutcome::SkippedByUser));
        }
        if !supported {
            return Ok(StepResult::new(step, StepOutcome::NotSupported));
        }
        if !has_data {
            return Ok(StepResult::new(step, StepOutcome::NoData));
        }
        cancel.check()?;
        progress(step);
        match action() {
            Ok(()) => Ok(StepResult::new(step, StepOutcome::Applied)),
            Err(error) if is_cancelled(&error) => Err(error),
            Err(error) => Ok(StepResult::failed(step, error.to_string())),
        }
    }

    /// Puts back the desktop saved before the first apply, then forgets the snapshot. Returns false
    /// if there was none. Fails with `SnapshotUnreadableError` (leaving the file alone) if it can't
    /// be read, and keeps the snapshot if restoring fails.
    pub fn restore_original(&self) -> Result<bool> {
        let _gate = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        let Some(snapshot) = self.snapshots.load()? else {
            return Ok(false);
        };
        self.backend.restore(&snapshot)?;
        self.snapshots.clear()?;
        Ok(true)
    }
}

/// The original desktop couldn't be secured, so no step ran.
fn nothing_changed(message: String) -> ApplyResult {
    let mut steps = vec![StepResult::failed(ApplyStep::SaveOriginal, message)];
    steps.extend(
        [ApplyStep::Wallpaper, ApplyStep::AppearanceMode, ApplyStep::AccentColor]
            .map(|s| StepResult::new(s, StepOutcome::NotAttempted)),
    );
    ApplyResult { steps }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::color::c;
    use crate::palette::AppearanceMode;
    use crate::paths::AppPaths;
    use crate::store::InstalledTheme;
    use crate::test_support::{FakeDesktopBackend, FakeFailure};
    use crate::theming::snapshot::{
        FileSnapshotStore, MemorySnapshotStore, OverlaySnapshotStore, SnapshotUnreadableError,
    };
    use crate::theming::{ApplyOptions, WallpaperFit};

    struct Harness {
        dir: tempfile::TempDir,
        backend: Arc<FakeDesktopBackend>,
        snapshots: Arc<MemorySnapshotStore>,
        applier: ThemeApplier,
        wallpaper: PathBuf,
    }

    fn harness() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(FakeDesktopBackend::default());
        let snapshots = Arc::new(MemorySnapshotStore::new(None));
        let applier = ThemeApplier::new(Box::new(backend.clone()), Box::new(snapshots.clone()));
        let wallpaper = dir.path().join("wall.png");
        std::fs::write(&wallpaper, [1, 2, 3]).unwrap();
        Harness { dir, backend, snapshots, applier, wallpaper }
    }

    impl Harness {
        fn request(&self, options: ApplyOptions) -> ApplyRequest {
            ApplyRequest {
                theme_name: "Tokyo".into(),
                wallpaper: Some(self.wallpaper.clone()),
                mode: AppearanceMode::Light,
                accent: Some(c("#7aa2f7")),
                background: Some(c("#1a1b26")),
                options,
            }
        }

        fn apply(&self, request: &ApplyRequest) -> ApplyResult {
            self.applier.apply(request, &mut |_| {}, &CancelToken::new()).unwrap()
        }
    }

    fn options(wallpaper: bool, mode: bool, accent: bool, fit: WallpaperFit) -> ApplyOptions {
        ApplyOptions::new(wallpaper, mode, accent, fit)
    }

    #[test]
    fn saves_original_desktop_then_applies_every_aspect_in_order() {
        let h = harness();
        let result = h.apply(&h.request(ApplyOptions::default()));

        assert_eq!(h.backend.calls(), ["capture", "wallpaper:wall.png:fill:#1a1b26", "mode:light", "accent:#7aa2f7"]);
        assert!(result.steps.iter().all(|s| s.outcome == StepOutcome::Applied));
        assert_eq!(h.snapshots.load().unwrap(), Some(FakeDesktopBackend::snapshot()));
    }

    #[test]
    fn only_the_first_apply_takes_a_snapshot() {
        let h = harness();
        h.apply(&h.request(ApplyOptions::default()));
        h.backend.clear_calls();

        let second = h.apply(&h.request(ApplyOptions::default()));

        assert!(!h.backend.calls().contains(&"capture".to_string()));
        assert!(second.result_for(ApplyStep::SaveOriginal).is_none());
    }

    #[test]
    fn changes_nothing_if_the_original_desktop_cannot_be_saved() {
        let h = harness();
        h.backend.fail_on("capture");

        let result = h.apply(&h.request(ApplyOptions::default()));

        assert_eq!(h.backend.calls(), ["capture"]);
        assert_eq!(result.result_for(ApplyStep::SaveOriginal).unwrap().outcome, StepOutcome::Failed);
        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::NotAttempted);
        assert!(!result.any_applied());
        assert_eq!(h.snapshots.load().unwrap(), None);
    }

    #[test]
    fn respects_user_choices() {
        let h = harness();
        let result = h.apply(&h.request(options(true, false, false, WallpaperFit::Center)));

        assert_eq!(h.backend.calls(), ["capture", "wallpaper:wall.png:center:#1a1b26"]);
        assert_eq!(result.result_for(ApplyStep::AppearanceMode).unwrap().outcome, StepOutcome::SkippedByUser);
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::SkippedByUser);
        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::Applied);
    }

    #[test]
    fn skips_what_the_platform_cannot_do() {
        let h = harness();
        h.backend.set_capabilities(DesktopCapabilities::WALLPAPER);

        let result = h.apply(&h.request(ApplyOptions::default()));

        assert_eq!(h.backend.calls(), ["capture", "wallpaper:wall.png:fill:#1a1b26"]);
        assert_eq!(result.result_for(ApplyStep::AppearanceMode).unwrap().outcome, StepOutcome::NotSupported);
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::NotSupported);
        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::Applied);
    }

    #[test]
    fn skips_aspects_the_theme_has_no_data_for() {
        let h = harness();
        let mut request = h.request(ApplyOptions::default());
        request.wallpaper = None;
        request.accent = None;

        let result = h.apply(&request);

        assert_eq!(h.backend.calls(), ["capture", "mode:light"]);
        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::NoData);
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::NoData);
    }

    #[test]
    fn one_failing_step_does_not_stop_the_others() {
        let h = harness();
        h.backend.fail_on("mode");

        let result = h.apply(&h.request(ApplyOptions::default()));

        assert!(h.backend.calls().contains(&"accent:#7aa2f7".to_string()));
        let mode = result.result_for(ApplyStep::AppearanceMode).unwrap();
        assert_eq!(mode.outcome, StepOutcome::Failed);
        assert_eq!(mode.error.as_deref(), Some("mode exploded"));
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::Applied);
    }

    #[test]
    fn missing_wallpaper_file_fails_that_step_without_calling_the_os() {
        let h = harness();
        let mut request = h.request(ApplyOptions::default());
        request.wallpaper = Some(h.dir.path().join("gone.png"));

        let result = h.apply(&request);

        assert!(!h.backend.calls().iter().any(|c| c.starts_with("wallpaper")));
        let wallpaper = result.result_for(ApplyStep::Wallpaper).unwrap();
        assert_eq!(wallpaper.outcome, StepOutcome::Failed);
        assert_eq!(wallpaper.error, Some(MissingWallpaperError.to_string()));
        assert_eq!(result.result_for(ApplyStep::AccentColor).unwrap().outcome, StepOutcome::Applied);
    }

    #[test]
    fn cancellation_propagates() {
        let h = harness();
        let cancel = CancelToken::new();
        cancel.cancel();

        let error = h.applier.apply(&h.request(ApplyOptions::default()), &mut |_| {}, &cancel).err().unwrap();

        assert!(is_cancelled(&error));
        assert!(h.backend.calls().is_empty());
    }

    #[test]
    fn reports_progress_for_each_step_it_runs() {
        let h = harness();
        let mut steps = Vec::new();

        h.applier
            .apply(
                &h.request(options(true, true, false, WallpaperFit::Fill)),
                &mut |s| steps.push(s),
                &CancelToken::new(),
            )
            .unwrap();

        assert_eq!(steps, [ApplyStep::SaveOriginal, ApplyStep::Wallpaper, ApplyStep::AppearanceMode]);
    }

    #[test]
    fn concurrent_applies_do_not_interleave() {
        let h = harness();
        let request = h.request(options(true, false, false, WallpaperFit::Fill));

        std::thread::scope(|scope| {
            for _ in 0..5 {
                scope.spawn(|| h.applier.apply(&request, &mut |_| {}, &CancelToken::new()).unwrap());
            }
        });

        // Exactly one snapshot, however the applies were scheduled.
        let calls = h.backend.calls();
        assert_eq!(calls.iter().filter(|c| *c == "capture").count(), 1);
        assert_eq!(calls.len(), 6);
    }

    #[test]
    fn restore_puts_back_the_snapshot_once() {
        let h = harness();
        assert!(!h.applier.restore_original().unwrap());

        h.apply(&h.request(ApplyOptions::default()));
        assert!(h.applier.has_original_snapshot());

        assert!(h.applier.restore_original().unwrap());
        assert_eq!(h.backend.restored(), Some(FakeDesktopBackend::snapshot()));
        assert!(!h.applier.has_original_snapshot());
        assert!(!h.applier.restore_original().unwrap());
    }

    #[test]
    fn failed_restore_keeps_the_snapshot() {
        let h = harness();
        h.apply(&h.request(ApplyOptions::default()));
        h.backend.fail_on("restore");

        assert!(h.applier.restore_original().err().unwrap().is::<FakeFailure>());
        assert!(h.applier.has_original_snapshot());
    }

    #[test]
    fn request_from_installed_theme_uses_chosen_or_first_wallpaper() {
        let h = harness();
        let theme = InstalledTheme {
            slug: "t".into(),
            name: "T".into(),
            repo_url: "https://github.com/o/t".into(),
            palette: None,
            mode: AppearanceMode::Dark,
            wallpapers: vec!["1.png".into(), "2.png".into()],
            screenshot_file: None,
            downloaded_at: chrono::Utc::now(),
            directory: h.dir.path().to_path_buf(),
        };
        let file = |r: ApplyRequest| r.wallpaper.map(|p| p.file_name().unwrap().to_string_lossy().into_owned());

        assert_eq!(
            file(ApplyRequest::from_theme(&theme, Some("2.png"), ApplyOptions::default())).as_deref(),
            Some("2.png")
        );
        assert_eq!(
            file(ApplyRequest::from_theme(&theme, Some("missing.png"), ApplyOptions::default())).as_deref(),
            Some("1.png")
        );
        assert_eq!(file(ApplyRequest::from_theme(&theme, None, ApplyOptions::default())).as_deref(), Some("1.png"));
        assert_eq!(ApplyRequest::from_theme(&theme, None, ApplyOptions::default()).accent, None);

        let empty = InstalledTheme { wallpapers: Vec::new(), ..theme };
        assert_eq!(ApplyRequest::from_theme(&empty, None, ApplyOptions::default()).wallpaper, None);
    }

    #[test]
    fn file_snapshot_store_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileSnapshotStore::new(AppPaths::new(dir.path()));
        let snapshot = FakeDesktopBackend::snapshot();

        store.save(&snapshot).unwrap();
        assert_eq!(store.load().unwrap(), Some(snapshot));

        store.clear().unwrap();
        assert_eq!(store.load().unwrap(), None);
        store.clear().unwrap();
    }

    /// Mirrors the apps' "never replaces a saved desktop it cannot read".
    #[test]
    fn never_replaces_a_saved_desktop_it_cannot_read() {
        let h = harness();
        let paths = AppPaths::new(h.dir.path());
        std::fs::write(paths.snapshot_file(), "{ not json").unwrap();
        let applier = ThemeApplier::new(Box::new(h.backend.clone()), Box::new(FileSnapshotStore::new(paths.clone())));

        let result = applier.apply(&h.request(ApplyOptions::default()), &mut |_| {}, &CancelToken::new()).unwrap();

        assert!(h.backend.calls().is_empty());
        let save = result.result_for(ApplyStep::SaveOriginal).unwrap();
        assert_eq!(save.outcome, StepOutcome::Failed);
        assert!(save.error.as_ref().unwrap().contains("can't be read"));
        assert_eq!(result.result_for(ApplyStep::Wallpaper).unwrap().outcome, StepOutcome::NotAttempted);
        assert_eq!(std::fs::read_to_string(paths.snapshot_file()).unwrap(), "{ not json");

        // Restore is still offered, and says why it can't.
        assert!(applier.has_original_snapshot());
        assert!(applier.restore_original().err().unwrap().is::<SnapshotUnreadableError>());
        assert!(h.backend.calls().is_empty());
    }

    #[test]
    fn an_overlay_store_reads_the_file_but_never_writes_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = AppPaths::new(dir.path());
        let file = FileSnapshotStore::new(paths.clone());
        file.save(&FakeDesktopBackend::snapshot()).unwrap();
        let overlay = OverlaySnapshotStore::new(FileSnapshotStore::new(paths.clone()));

        assert_eq!(overlay.load().unwrap(), Some(FakeDesktopBackend::snapshot()));
        overlay.clear().unwrap();
        assert_eq!(overlay.load().unwrap(), None);
        assert_eq!(file.load().unwrap(), Some(FakeDesktopBackend::snapshot()));

        std::fs::write(paths.snapshot_file(), "{ not json").unwrap();
        assert!(OverlaySnapshotStore::new(file).load().is_err());
    }
}
