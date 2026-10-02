# From the command line to the apps

Building `omatheme` (the Rust CLI, `cli/`) turned up three things the apps should change now that a
second client can share their data folder. The CLI already does each one. Delete an item when both apps
have it, and this file when it's empty.

## 1. Tidy up only old interrupted downloads (macOS and Windows)

Both apps delete every `.staging-*` folder in `themes/` when they start (`ThemeLibrary` calls the
store's cleanup). If `omatheme download` is running at that moment, the app deletes its staging folder
from under it and the download fails. The same goes for one app instance and the CLI's cleanup, which
is why the CLI only touches folders older than six hours (`ThemeStore::clean_up(min_age)` in
`cli/src/store/theme_store.rs`, tested by `clean_up_leaves_recent_folders_alone`).

- macOS: `ThemeStore.cleanUpStaging()` in
  `macos/OmarchyThemesKit/Sources/OmarchyThemesKit/Storage/ThemeStore.swift` (called from
  `OmarchyThemesStores/ThemeLibrary.swift`).
- Windows: `ThemeStore.CleanUpStaging()` in `windows/src/OmarchyThemes.Core/Storage/ThemeStore.cs`
  (called from `OmarchyThemes.Stores/ThemeLibrary.cs`).

Skip folders whose modification time is recent (the CLI uses 6 hours), with a test.

## 2. Restore a copy a failed reinstall left aside (macOS)

The CLI's theme store works like the Windows app's: a reinstall renames the old copy aside to
`.old-<slug>-<32 hex digits>` and a removal renames the theme to `.removed-<slug>-<id>` before deleting
it. If the CLI is interrupted at the wrong moment on a Mac, it leaves such a folder. The Windows app's
cleanup puts an `.old-` copy back (or deletes it if the theme is there) and deletes `.removed-` folders;
the macOS app only deletes `.staging-` folders, and its list skips the others, so the theme would look
removed.

- macOS: extend `ThemeStore.cleanUpStaging()` like Windows' `CleanUpStaging()` (including its
  `SlugOfPrevious` name check), with the Windows tests' cases.

## 3. A dry run must not record a pretend desktop in the real data folder (macOS and Windows)

With `OMARCHY_THEMES_DRY_RUN=1` but no `OMARCHY_THEMES_DATA_DIR`, the first Apply saves the dry-run
backend's snapshot (`{"dryRun": "1"}`) to the real `original-desktop.json`. After that a real Apply
never saves the real desktop (a snapshot exists), and Restore "succeeds" without putting anything back
(neither backend finds its keys) and then forgets the snapshot, so the user's original desktop is lost.

The CLI keeps the real file read-only in that case: changes go to an overlay in memory
(`OverlaySnapshotStore` in `cli/src/theming/snapshot.rs`), and settings.json isn't written either
(`SettingsStore::in_memory`). With a test data folder, the dry run behaves as before.

- macOS: `AppServices.live()` in `macos/OmarchyThemesKit/Sources/OmarchyThemesStores/AppServices.swift`
  (`FileSnapshotStore` with `DryRunDesktopBackend`).
- Windows: `App.xaml.cs` in `windows/src/OmarchyThemes.App` (`FileSnapshotStore` registered with
  `DryRunDesktopBackend`).

Either use an in-memory snapshot store when dry-running against the real folder, or refuse a dry run
without a test data folder.
