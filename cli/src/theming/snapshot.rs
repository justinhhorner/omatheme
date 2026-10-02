use std::sync::Mutex;

use anyhow::Result;

use super::models::DesktopSnapshot;
use crate::json;
use crate::paths::AppPaths;

/// Where the snapshot of the original desktop is kept between the first apply and restore.
pub trait SnapshotStore: Send + Sync {
    /// The saved desktop, or None if none was saved. Fails with [`SnapshotUnreadableError`] if one
    /// was saved but can't be read.
    fn load(&self) -> Result<Option<DesktopSnapshot>>;
    fn save(&self, snapshot: &DesktopSnapshot) -> Result<()>;
    fn clear(&self) -> Result<()>;
}

/// The saved original desktop exists but can't be read. It's the only way back to the user's own
/// desktop, so it must not be treated as missing (and replaced by the themed desktop).
#[derive(Debug, thiserror::Error)]
#[error("The saved copy of your original desktop can't be read ({0} in the data folder).")]
pub struct SnapshotUnreadableError(pub String);

/// original-desktop.json in the data folder (docs/data-format.md).
pub struct FileSnapshotStore {
    paths: AppPaths,
}

impl FileSnapshotStore {
    pub fn new(paths: AppPaths) -> Self {
        FileSnapshotStore { paths }
    }
}

impl SnapshotStore for FileSnapshotStore {
    fn load(&self) -> Result<Option<DesktopSnapshot>> {
        let path = self.paths.snapshot_file();
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(unreadable(&path).into()),
        };
        json::from_slice(&bytes).map(Some).map_err(|_| unreadable(&path).into())
    }

    fn save(&self, snapshot: &DesktopSnapshot) -> Result<()> {
        json::write_json(&self.paths.snapshot_file(), snapshot)
    }

    fn clear(&self) -> Result<()> {
        Ok(json::remove_file_if_present(&self.paths.snapshot_file())?)
    }
}

fn unreadable(path: &std::path::Path) -> SnapshotUnreadableError {
    SnapshotUnreadableError(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// Reads another store but keeps changes in memory: a dry run against the real data folder must
/// not record a pretend desktop as the user's original (or forget the real one). An unreadable
/// snapshot stays unreadable, as in the file store.
pub struct OverlaySnapshotStore<S: SnapshotStore> {
    base: S,
    overlay: Mutex<Option<Option<DesktopSnapshot>>>,
}

impl<S: SnapshotStore> OverlaySnapshotStore<S> {
    pub fn new(base: S) -> Self {
        OverlaySnapshotStore { base, overlay: Mutex::new(None) }
    }
}

impl<S: SnapshotStore> SnapshotStore for OverlaySnapshotStore<S> {
    fn load(&self) -> Result<Option<DesktopSnapshot>> {
        match self.overlay.lock().unwrap().clone() {
            Some(changed) => Ok(changed),
            None => self.base.load(),
        }
    }

    fn save(&self, snapshot: &DesktopSnapshot) -> Result<()> {
        *self.overlay.lock().unwrap() = Some(Some(snapshot.clone()));
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        *self.overlay.lock().unwrap() = Some(None);
        Ok(())
    }
}

/// A snapshot kept in memory only (tests).
pub struct MemorySnapshotStore {
    state: Mutex<Option<DesktopSnapshot>>,
}

impl MemorySnapshotStore {
    pub fn new(initial: Option<DesktopSnapshot>) -> Self {
        MemorySnapshotStore { state: Mutex::new(initial) }
    }
}

impl SnapshotStore for MemorySnapshotStore {
    fn load(&self) -> Result<Option<DesktopSnapshot>> {
        Ok(self.state.lock().unwrap().clone())
    }

    fn save(&self, snapshot: &DesktopSnapshot) -> Result<()> {
        *self.state.lock().unwrap() = Some(snapshot.clone());
        Ok(())
    }

    fn clear(&self) -> Result<()> {
        *self.state.lock().unwrap() = None;
        Ok(())
    }
}
