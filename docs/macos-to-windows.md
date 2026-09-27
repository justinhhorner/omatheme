# macOS → Windows: changes to bring across

Written on the Mac (Sep 2026) after building the macOS app. It lists what the macOS app does that the
Windows app doesn't yet, so a session on a Windows machine can implement it. Items are in priority order.
Port behaviour, not code (see CLAUDE.md), and keep each app's own conventions: WinUI/Fluent on Windows.

The usual rule applies: never apply a theme or restore the desktop on the real machine without asking. Test
apply logic against fakes, and in the UI only open dialogs and cancel them.

When an item is done, delete it from this file (and this file once it's empty), and update README/CLAUDE.md.

---

## 0. First: build and check the Windows edits made blind on the Mac

The default-themes feature (item 1) was added to both platforms. Windows **Core** was built and tested on the
Mac (`dotnet test tests/OmarchyThemes.Core.Tests`: 129 passing). The **WinUI app** can't be built on macOS
(the XAML compiler is Windows-only), so these edits have been reviewed but never compiled or run:

| File | Change |
|---|---|
| `src/OmarchyThemes.App/App.xaml.cs` | `CatalogService` now gets the `GitHubClient` (`github:` argument) so it can list default themes. |
| `src/OmarchyThemes.App/ViewModels/GalleryViewModel.cs` | Subtitle says "N themes" (was "community themes"). `EnsureLoadedAsync` also refreshes at startup when the cached catalog has no default themes (otherwise upgrading users wait up to 12 hours to see them). |
| `src/OmarchyThemes.App/ViewModels/ItemViewModels.cs` | `ThemeCardViewModel.RepoDisplay = entry.RepoDisplay` (Core now says "Included with Omarchy" for default themes); removed the unused `OmarchyThemes.Core.GitHub` using. |
| `src/OmarchyThemes.App/ViewModels/ThemeDetailViewModel.cs` | `RepoDisplay => Entry.RepoDisplay`. |
| `src/OmarchyThemes.App/Views/WelcomePage.xaml`, `AboutPage.xaml` | Wording mentions the themes that ship with Omarchy, not only the community gallery. |

Check: `dotnet build OmarchyThemes.sln`, `dotnet test` (Core 129 + backend 22), then run the app. The gallery
should list 22 default themes (Catppuccin … White) before the ~146 community ones; opening Tokyo Night shows
8 wallpapers, and the header link reads "Included with Omarchy" and opens
`github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night`.

## 1. Default themes: finish the gallery UI

**Done in Core on both platforms** (`Core/Catalog/DefaultThemes.cs`, `CatalogService`, shared fixture
`fixtures/omarchy-tree.json`, `DefaultThemesTests`). What the macOS UI adds on top, which Windows lacks:

- **Two sections** with headers: "Included with Omarchy" (count, "The themes Omarchy ships with") and
  "Community" (count, "From omarchy.org/themes"). Search and the "downloaded only" filter apply to both, and
  an empty section is hidden. On Windows the list is flat (default themes first). Group with `ItemsView`
  plus a header per group, or two `ItemsView`s in the page's `ScrollViewer`, sharing the card template.
  macOS: `macos/App/Views/GalleryView.swift` (`grid`, `GallerySectionHeader`).
- **A notice when they fail to load.** `ThemeCatalog.DefaultThemesError` is set when GitHub failed and
  nothing was cached; the community gallery still loads. macOS shows a warning banner ("Omarchy's themes are
  missing" + the error) only when no default themes are shown. On Windows add an `InfoBar` next to the
  existing stale-catalog notice in `GalleryViewModel.RefreshAsync`. macOS: `AppModel.performRefresh`
  (`defaultThemesNotice`).

## 2. Large wallpaper preview on the theme page

macOS (`macos/App/Views/ThemeDetailView.swift`: `WallpaperPreview`, `wallpaperSection`,
`WallpaperThumbnail`) shows a wallpaper large, over the theme page inside the main window (not a new window):

- Opened by a **Preview** button next to the "Wallpapers" heading (previews the selected wallpaper), a small
  expand button that appears on a thumbnail on hover, or double-clicking a thumbnail. VoiceOver gets a
  "Preview" action per thumbnail.
- A dark backdrop covers the page. The image is fitted (full resolution, downsampled to ~2560 px), with a
  spinner while a remote (not yet downloaded) image loads.
- A control bar underneath: previous/next buttons (also ← / →, wrapping around), the file name and "3 of 8",
  **Use This Wallpaper** (selects it for Apply; shows "Selected" when it already is), and a close button
  (also Esc). A click anywhere outside the bar closes it.

Windows suggestion: a full-page overlay `Grid` in `ThemeDetailPage` (a `Popup`/`TeachingTip` is too small;
a `ContentDialog` has fixed chrome). Give it a SmokeGrid-style dim layer (`SmokeFillColorDefaultBrush`), an
`Image` with `Stretch="Uniform"` decoded at ~2560 px (`BitmapImage.DecodePixelWidth`), and a bottom
`CommandBar` or `StackPanel` with the controls. Handle `KeyboardAccelerator`s for Left/Right/Escape, and
move focus into the overlay when it opens and back to the thumbnail when it closes. Add a Preview button to
the wallpaper section header, a hover button on each item in `WallpaperGrid`, and `DoubleTapped` on items.
Selecting in the preview should update `SelectedWallpaper`. Give icon-only buttons an
`AutomationProperties.Name` (see CLAUDE.md).

## 3. "View on GitHub" button with the GitHub mark

macOS has a toolbar button **View on GitHub** with GitHub's logo, a standard-width button with a little
horizontal padding, tooltip "Open this theme's repository on GitHub". Windows today has a `HyperlinkButton`
in the header with a generic link glyph (`&#xE8A7;`) plus "Open on GitHub" in the ⋯ flyout.

The mark is `macos/App/Assets.xcassets/GitHubMark.imageset/github-mark.svg` (Octicons `mark-github`, MIT;
use it only to link to GitHub). Its `d` path data works unchanged as XAML path markup, so on Windows use a
`PathIcon` (`Data="…"`) at 16×16 in a `Button`/`AppBarButton` labelled "View on GitHub". Where it goes is a
Fluent call: next to the primary actions in the header, or in the `TitleBar`/command area.

## 4. Downloads that survive navigating away

On macOS, downloads (and theme lookups) run in tasks owned by the app-wide model, keyed by theme. Leaving the
page doesn't cancel or orphan them, coming back shows the live progress, and pressing Download again joins
the running download instead of starting a second one. Concurrent lookups of the same theme share one GitHub
request. macOS: `macos/App/Model/AppModel.swift` (`download`, `downloads`, `DownloadState`, `details(for:)`).

On Windows, `ThemeDetailViewModel` is **transient** (`App.xaml.cs`: `AddTransient<ThemeDetailViewModel>`)
and `ThemeDetailPage` isn't navigation-cached, so a download belongs to the page. **Check first:** start a
download, navigate back, reopen the theme. If progress is lost or a second download can start, move the
download (task, progress, cancellation) into a singleton service keyed by slug (e.g. a `DownloadService`
next to `ThemeDetailsService`) and have the view model observe it. Also make `ThemeDetailsService`
share an in-flight `Task<ThemeDetails>` per slug rather than resolving twice.

## 5. Theme background around Fit/Center wallpapers

The macOS backend sets the desktop fill color to the theme's `background` color, so Fit and Center wallpapers
are framed in the theme's color instead of black. In the Swift Kit, `DesktopBackend.setWallpaper` takes a
`fillColor`, and `ApplyRequest` carries `background` from the palette.

Windows equivalent: `IDesktopWallpaper::SetBackgroundColor` (COLORREF). It's system-wide, so it must be
**snapshotted and restored** (`GetBackgroundColor`) with the rest of the desktop. Add it to
`IDesktopBackend.SetWallpaperAsync` (or a new capability), `WindowsDesktopBackend`, the snapshot values, and
the backend tests (assert the exact color written and restored).

## 6. Test switches for safe UI checks

The macOS app reads two environment variables, used for every UI check there (`macos/App/Model/AppModel.swift`,
`DryRunDesktopBackend.swift`):

- `OMARCHY_THEMES_DRY_RUN=1` swaps in a backend that reports the same capabilities as the real one but
  changes nothing, and shows a "Dry run" badge in the window. Apply and Restore can then be exercised end to
  end without touching the desktop, which is a much stronger guarantee than "only open dialogs and cancel".
- `OMARCHY_THEMES_DATA_DIR=<path>` uses another data folder instead of `%LOCALAPPDATA%\OmarchyThemes`
  (fresh first-launch state, no risk to real downloads or the saved original desktop).

Windows: read both in `App.ConfigureServices` (`AppPaths` root, and register a dry-run `IDesktopBackend`),
show a small badge in the shell, and add both to README/CLAUDE.md.

## 7. Error logging

macOS logs catalog, default-theme and download failures with `os.Logger` (subsystem
`com.justinhhorner.OmarchyThemes`). That's how a bug where SwiftUI cancelled the first catalog request was
found: the UI only said "Couldn't load themes". Windows has no logging. Add `Microsoft.Extensions.Logging`
(Debug + EventLog/EventSource, or a small rolling file in the data folder) at the same points
(`GalleryViewModel.RefreshAsync`, `ThemeDetailViewModel.ResolveAsync`/`DownloadAsync`, `ApplyService`),
logging the exception type and message.

## 8. Opt-in live checks

macOS has read-only tests against the real services, off by default
(`macos/OmarchyThemesKit/Tests/OmarchyThemesMacTests/LiveChecks.swift`, run with
`OMATHEME_LIVE=1 swift test --filter LiveChecks`). They parse omarchy.org, list and resolve a few default and
community themes, download one wallpaper checking byte-level progress, and read the current wallpaper. On the
Mac they caught a real bug that the fixtures couldn't: download progress wasn't in bytes. Windows did the same
checks by hand. Consider a `[Trait("Category", "Live")]` xUnit class skipped unless `OMATHEME_LIVE=1`, keeping
to a handful of GitHub calls (60/hour unauthenticated).

---

## macOS-only: don't port

- **Light/dark and accent are unsupported on macOS**, shown disabled in the Apply sheet with an "Open
  Appearance Settings" link. Windows supports both.
- **`ApplySummary` on macOS dropped the "sign out" accent note.** It's Windows-specific; keep it on Windows.
- **Swift-only internals:** the hand-written strict TOML reader (Windows uses Tomlyn), `HTTPTransport`
  instead of a URLProtocol fake, `AsyncGate` (Windows has `SemaphoreSlim`), the SwiftUI task-cancellation
  fix and the thumbnail aspect-ratio fix.
- **WebP/BMP → PNG before setting the wallpaper:** Windows already converts with WIC.
- **Credits:** both apps link `github.com/basecamp/omarchy`, which now redirects to `omacom/omarchy`. Update
  both together if you change it.
