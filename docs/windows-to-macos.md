# Windows to macOS: changes to bring over

From the Windows code review follow-up (Sep 29, 2026). Each item names the Windows change to port
(behaviour, not code) and where it goes on macOS. Delete items as they're done, and this file when
it's empty (and its line in `docs/ARCHITECTURE.md`'s layout and `CLAUDE.md`).

1. **Don't overwrite an unreadable `original-desktop.json`.** Today `FileSnapshotStore.load()` returns
   nil for a corrupt file, so the next Apply captures the *themed* desktop over it, and
   `MacDesktopBackend.capture()` deletes `original-desktop/` (the private wallpaper copies) first. The
   user's real original is lost.
   - Windows: `SnapshotUnreadableException` in `windows/src/OmarchyThemes.Core/Theming/SnapshotStore.cs`
     (`FileSnapshotStore.Load` throws it when the file exists but can't be read); `ThemeApplier.ApplyAsync`
     turns it into a failed SaveOriginal step with nothing applied; `HasOriginalSnapshot` counts it as
     present so Restore can say why. Test: `ThemeApplierTests.Never_replaces_a_saved_desktop_it_cannot_read`.
   - Windows backend: `WindowsDesktopBackend.CaptureAsync` copies into `original-desktop.new/` and swaps
     it in only when every copy succeeded. Test: `A_failed_capture_keeps_the_previous_wallpaper_copies`.
   - macOS: `OmarchyThemesKit/Theming/SnapshotStore.swift` (make `load()` throw for unreadable),
     `ThemeApplier.swift`, and `OmarchyThemesMac/MacDesktopBackend.swift` `capture()`.

2. **Only `GITHUB_TOKEN`.** The user chose to drop `OMARCHY_THEMES_GITHUB_TOKEN` (both were the same
   token). Windows: `windows/src/OmarchyThemes.App/Services/AppEnvironment.cs`. macOS:
   `OmarchyThemesStores/AppServices.swift` (`live()` reads both).

3. **One User-Agent, built from the app version.** Windows: `AppInfo.UserAgent` in
   `windows/src/OmarchyThemes.Core/AppInfo.cs`, used by `GitHubClient` and the app's HTTP clients
   (`OmarchyThemes/0.1.0 (+https://github.com/basecamp/omarchy)`). macOS: `HTTPSessions.userAgent` in
   `OmarchyThemesKit/Net/HTTPTransport.swift` is a hard-coded `OmarchyThemes/0.1`.

4. **Low priority: the Apply sheet saves settings itself.** On Windows the dialog now returns the
   choices plus the "remember" flag, and `DesktopStore.ApplyAsync(…, rememberOptions)` saves them
   (test: `Choices_from_the_apply_dialog_can_become_the_defaults`). macOS: `App/Views/ApplySheet.swift:138`
   calls `preferences.update` from the view; `DesktopStore.apply` could take the flag instead.
