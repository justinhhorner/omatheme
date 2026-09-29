# Windows code review: notes for the next Windows session

A review of `windows/` done on a Mac (Sep 28, 2026). On the Mac, Core and its tests build and run, the
Windows backend and its tests only compile (`-p:EnableWindowsTargeting=true`), and the WinUI app can't be
built at all (the XAML compiler is Windows-only). So:

- **Core** changes were made and tested on the Mac.
- **Platform.Windows** got a few very low-risk edits that compiled but haven't been run.
- **App** wasn't touched. Everything for it is below, to apply and compile on Windows.

Work through the sections in order. Line numbers refer to the tree as this review left it.

## 1. First: verify what the Mac couldn't

Run on Windows, from `windows/`:

```powershell
dotnet build OmarchyThemes.sln      # must succeed with 0 warnings (the App is the part not built on the Mac)
dotnet test                         # expect Core 161 passed + 3 skipped, Windows backend 31 passed + 2 skipped
```

Core went from 158 to 161 tests (3 new ones, listed below). The backend count is unchanged.

**Changes that only compiled (Platform.Windows):**

| File | Change | What checks it |
|---|---|---|
| `src/OmarchyThemes.Platform.Windows/WindowsTerminalSchemes.cs` `Add` | Writes through the new `JsonFile.WriteAtomic(path, value, options)` instead of its own temp-file-and-rename. Same serializer options, same temp name, same folder creation. One difference: the slug is now checked **before** the fragments folder is created, so an invalid slug no longer leaves an empty `Fragments\OmarchyThemes` folder behind. | All of `WindowsTerminalSchemesTests` (7), especially the no-BOM check and `Adding_again_replaces_and_removing_cleans_up`. |
| `src/OmarchyThemes.Platform.Windows/WindowsDesktopBackend.cs` | The `"reg:<key>\|<name>"` snapshot key, spelled out in both `CaptureAsync` and `RestoreAsync`, is now one `RegSnapshotKey(key, name)` helper. It produces the same string. | `Capture_then_restore_puts_back_registry_and_per_monitor_wallpapers`, `Snapshot_survives_json_round_trip`. If this PC has a real `%LOCALAPPDATA%\OmarchyThemes\original-desktop.json`, a **read-only** look at its `reg:…` keys confirms old snapshots still match. |
| `tests/OmarchyThemes.Platform.Windows.Tests/WindowsDesktopBackendTests.cs` | Dropped the one-line `AccentMathFromAbgr` wrapper; the test calls `AccentMath.FromAbgr` directly. | `Near_black_accents_are_lifted_before_writing`. |

**Core changes the App compiles against** (all covered by Core tests, but only a solution build proves
the App still compiles):

- `Theming/ThemeApplier.cs` was split. `ApplyOptions` and `ApplyRequest` moved to `ApplyRequest.cs`;
  `ApplyStep`, `StepOutcome`, `StepResult` and `ApplyResult` to `ApplyResult.cs`; `ISnapshotStore` and
  `FileSnapshotStore` to `SnapshotStore.cs`. Namespaces are unchanged.
- `TerminalColors.AnsiNames` (public, used only inside Core) was replaced by `Palettes/AnsiColors.cs`
  (`Names` and `SwatchName(index)`). The two palette parsers and `TerminalColors` had each spelled the
  names out, and they have to agree (the parsers name the swatches, `TerminalColors` looks them up).
- `JsonFile` gained `WriteAtomic(path, value, JsonSerializerOptions)`. The three atomic writers now share
  one private `ReplaceAtomic`.
- Smaller Core edits: `ThemeStore` uses `Directory.` instead of `System.IO.Directory.`;
  `ThemeDownloads.RunAsync` names its "finishing" status once; `Palette.Mode` had two `<summary>` tags,
  now merged into one.
- Tests: the three private `IProgress<T>` collectors are now one `ListProgress<T>` in
  `TestSupport/Fakes.cs`. New: `PaletteParserTests.Both_formats_name_the_16_ansi_swatches_the_same_way`,
  and `JsonFileTests` (2 tests: no BOM or leftover temp file with other options; byte writes replace).

Build warnings were 0 before and after, for `src/OmarchyThemes.Core` and for
`tests/OmarchyThemes.Platform.Windows.Tests` (with `EnableWindowsTargeting`).

## 2. Bugs and risks

Most severe first. "Core" items can be fixed and tested on either machine.

1. **Unexpected exceptions crash the app and log nothing.** `App.xaml.cs` has no `UnhandledException`
   handler, and several `async void` and command paths let ordinary I/O errors through:
   - `GalleryViewModel.RefreshAsync` (`ViewModels/GalleryViewModel.cs:191`) catches HTTP/timeout/format
     errors but not `IOException` or `UnauthorizedAccessException` from cache writes. It's reached from
     `GalleryPage.OnNavigatedTo` (async void).
   - `ThemeDetailViewModel.ResolveAsync` (`ViewModels/ThemeDetailViewModel.cs:176`) has the same gap. It's
     reached from `ThemeDetailPage.OnNavigatedTo`.
   - `ThemeDetailViewModel.Remove` (`:319`) and `DownloadedViewModel.Remove` (`:107`) call
     `ThemeStore.Remove`, which throws if a file is locked.
   - `SettingsViewModel.ClearCache` (`:137`) catches `IOException` only, not `UnauthorizedAccessException`.
     `OpenDataFolder` (`:124`) doesn't catch `Win32Exception`.
   - `ApplyService.ApplyAsync` (`Services/AppServices.cs:62`) runs `settings.Update` after a successful
     apply. If that write throws, the app crashes and the applied theme isn't recorded.

   **Fix:** in `App()`, log `UnhandledException` and `TaskScheduler.UnobservedTaskException` (with
   `LogCritical(e.Exception, …)`). Widen the catch filters above, or better, use one shared "expected
   error" check (see 4.3). Show failures in the page's InfoBar.
   `FileLoggerProvider` (`Services/FileLoggerProvider.cs:36`) logs only an exception's type and message.
   Log `exception.ToString()` at Error and above, so a crash leaves a stack trace.

2. **Core: cache writes can fail a fetch that succeeded.** `HttpCache.GetAsync` calls `Store`/`WriteMeta`
   with no guard, and `JsonFile.ReplaceAtomic` (`Storage/JsonFile.cs:54`) always uses the same temp
   name, `path + ".tmp"`. On Windows the write fails with an `IOException` when:
   - two requests for the same URL finish together. Omarchy's tree (`omacom/omarchy` `HEAD`) is shared:
     a background gallery refresh and opening a default theme can revalidate it at the same moment, and
     `File.Create` then hits a sharing violation;
   - Defender or the indexer holds a freshly written file while `File.Move` renames over it.

   Because of item 1, either case crashes the app.
   **Fix:** treat cache-write failures as non-fatal in `HttpCache` (catch `IOException` and
   `UnauthorizedAccessException` around `Store`/`WriteMeta`, then return the response anyway). Give
   `ReplaceAtomic` a unique temp name (`$"{path}.{Guid.NewGuid():N}.tmp"`) and delete the temp file when
   the write fails. Temp files aren't part of `docs/data-format.md`.

3. **Core: reinstalling a theme isn't atomic.** `ThemeStore.InstallAsync` (`Storage/ThemeStore.cs:146-148`)
   deletes the installed folder, then moves the staged one into place.
   - If the delete fails partway (a file is locked), the installed theme is left half-deleted: its
     `theme.json` can list wallpapers that no longer exist, and Apply then reports "The wallpaper file
     is missing".
   - If the move fails, the old copy is already gone.

   **Fix:** rename the old folder to `.old-<slug>-<guid>`, move the staged folder in, then delete the old
   one best-effort. If the move fails, rename the old folder back. `CleanUpStaging` should remove `.old-*`
   too. Add a `ThemeStoreTests` case with a fake that fails the move.

4. **Core + backend: an unreadable `original-desktop.json` counts as "no snapshot".** `JsonFile.TryRead`
   returns null for a corrupt file, so `ThemeApplier.ApplyAsync` (`Theming/ThemeApplier.cs:33`) captures
   again and overwrites it with the *themed* desktop. `WindowsDesktopBackend.CaptureAsync`
   (`WindowsDesktopBackend.cs:94`) also deletes `original-desktop/`, the private copies of the original
   wallpapers, before copying. The user's real original is lost, and Settings says "Nothing has been
   changed yet". Atomic writes make this unlikely, but this file is the only safety net.
   **Fix:** have `FileSnapshotStore` tell "missing" from "unreadable". For unreadable, fail the
   SaveOriginal step with a clear message (or back the file up before overwriting). Have the backend
   capture into a temp folder and swap it in only when the capture succeeds.

5. **Clear Cache doesn't clear the in-session theme details.** `SettingsViewModel.ClearCache` deletes
   `cache/`, but `ThemeDetailsService` (`Themes/ThemeDetailsService.cs:16`) keeps its memo, so "The
   catalog and theme details will be downloaded again when needed" isn't true until the app restarts. The
   macOS app clears both (`AppStores.clearCache()` → `library.clearDetailsCache()`, with a generation
   counter so a lookup in flight doesn't refill the cache).
   **Fix:** add `ThemeDetailsService.Clear()` with the same generation guard, test it in Core, and call it
   from `ClearCache`.

6. **Small wording problems** (user-visible text: check with the user before changing):
   - `SettingsViewModel.GitHubText` (`:80`) only mentions `GITHUB_TOKEN`, but
     `OMARCHY_THEMES_GITHUB_TOKEN` is checked first (`StorageInfo.GitHubToken`).
   - `CurrentThemeViewModel.Details` (`:68`) says "0 wallpapers" where `InstalledThemeViewModel.Details`
     (`ItemViewModels.cs:110`) says "No wallpaper".

7. **Fragile cast in `ShellPage`.** `(string)i.Tag == tag` (`Views/ShellPage.xaml.cs:34, 86`) throws if
   a nav item without a string `Tag` is ever added. Use `i.Tag as string == tag` (see 3.4).

8. **Inconsistent User-Agent.** API requests send `OmarchyThemes/1.0` (set per request in
   `GitHubClient.cs:36`, which overrides the client default). Everything else sends
   `OmarchyThemes/0.1 (+https://github.com/basecamp/omarchy)` (`AppServices.cs:16`). Neither comes from
   the assembly version (0.1.0). Low priority: use one value, built from the version.

## 3. App changes to make (WinUI, can't be built on the Mac)

No change here may alter what the user sees or the automation names, unless the item says so.

### 3.1 Services and startup

- **Split `Services/AppServices.cs`.** It holds four unrelated types. Make it `HttpClients.cs`,
  `ApplyService.cs` and `NavigationService.cs`, and move `StorageInfo.GitHubToken()` into
  `AppEnvironment`: read it once in `FromProcess`, like the macOS `AppServices.live` does. Then
  `StorageInfo` is only `DirectorySize`, and its summary ("Snapshot of local storage use") is accurate.
- **`App.ConfigureServices`** (`App.xaml.cs:33-73`): `CatalogService` is registered before the
  `GitHubClient` it needs. Order the registrations by layer (network, themes, desktop, view models).
  Resolution is lazy, so behaviour doesn't change. Add the exception logging from bug 1 here.
- **`DryRunDesktopBackend.Capabilities`** repeats `WindowsDesktopBackend.Capabilities`, and its summary
  says the two must match. Expose `public const DesktopCapabilities Supported` on `WindowsDesktopBackend`
  and use it in both, so they can't drift.

### 3.2 View models: remove duplication

- **The result InfoBar state is copied four times**: `IsResultOpen`, `ResultTitle`, `ResultMessage`,
  `ResultSeverity` and a `ShowResult` in `ThemeDetailViewModel`, `DownloadedViewModel`,
  `SettingsViewModel` and `CurrentThemeViewModel`. Extract it (this is the macOS `Banner`):

  ```csharp
  public sealed partial class ResultBar : ObservableObject
  {
      [ObservableProperty] public partial bool IsOpen { get; set; }
      [ObservableProperty] public partial string Title { get; set; } = "";
      [ObservableProperty] public partial string Message { get; set; } = "";
      [ObservableProperty] public partial InfoBarSeverity Severity { get; set; }

      public void Show(InfoBarSeverity severity, string title, string message)
      {
          Severity = severity;
          Title = title;
          Message = message;
          IsOpen = true;
      }

      public void Show(ApplySummary summary) => Show(Ui.Severity(summary.Kind), summary.Title, summary.Message);

      public void Close() => IsOpen = false;
  }
  ```

  Each view model gets `public ResultBar Result { get; } = new();`. Rebind the four InfoBars:
  `ThemeDetailPage.xaml:208-212`, `DownloadedPage.xaml:19-23`, `SettingsPage.xaml:25-30` and
  `GalleryPage.xaml:274-278` (`ViewModel.CurrentTheme.Result.*`). Keep `Mode=TwoWay` on `IsOpen`.
- **Palette summary chips**: `KeyColors(p).Take(3).Concat(Swatches(p).Take(8))` appears in both
  `ItemViewModels.cs:100-102` and `CurrentThemeViewModel.cs:53-55`. Add
  `ColorChipViewModel.Summary(Palette?)`.
- **Mode wording** is built in four places: `ItemViewModels.cs:107` ("Light"), `CurrentThemeViewModel.cs:67`
  and `ThemeDetailViewModel.cs:131` ("Light theme"), `ApplyDialog.xaml.cs:30` ("Switch Windows to light
  mode"). Add `Ui.ModeName(AppearanceMode)` ("Light"/"Dark") and build the rest from it.
- **The catalog entry for an installed theme** is built twice:
  `new CatalogEntry(theme.Slug, theme.Name, theme.RepoUrl, ScreenshotUrl: null)` in
  `GalleryViewModel.cs:81` and `DownloadedViewModel.cs:110-111`. `DownloadedViewModel` depends on
  `GalleryViewModel` only to call `Find`. Add `GalleryViewModel.EntryFor(InstalledTheme)` now; in 4.1 it
  moves to a catalog store and the dependency between the view models goes away.
- **Identical exception filters**: `GitHubException or ThemeResolveException or HttpRequestException or
  TaskCanceledException` appears in `ThemeDetailViewModel.cs:176` and `ThemeDetailViewModel.Terminal.cs:68`,
  and a similar list in `GalleryViewModel.cs:191`. Replace them with the shared check in 4.3.
- **Section tags**: `"Gallery"` is a string literal in both `DownloadedViewModel.BrowseThemes` (`:114`)
  and `ShellPage.Pages`. Add a `Sections` class with constants (the XAML `Tag`s stay literal).
- `ThemeDetailViewModel.Terminal.cs:61` compares against `"Bright black"`. Use `AnsiColors.SwatchName(8)`,
  the Core naming contract.

### 3.3 View models: readability

- `GalleryViewModel` constructor (`:36-41`): the fields are assigned in a scrambled order. Match the
  declaration order.
- `GalleryViewModel.Show` (`:226`) and `UpdateBadges` (`:238`) each call `_store.List()`, which reads
  every manifest from disk, so one refresh lists the themes twice. Pass the installed slugs into
  `UpdateBadges`.
- `ThemeDetailViewModel.LoadAsync` (`:146`): the comment about a running download sits above
  `RefreshTerminalState()`. Move it down one line, to the `_downloads.Get` check it describes.
- `ItemViewModels.cs:54`: the "For the large preview" doc comment is on the private `_preview` field.
  Put it on `Preview`, and order the field and property like `Thumbnail`'s.
- `Wallpapers.ToList().IndexOf(...)` / `.FindIndex(...)` (`ThemeDetailViewModel.cs:219, 235`,
  `ThemeDetailViewModel.Preview.cs:23`) copy the list on every call. A small `IndexOf` helper over
  `IReadOnlyList<T>` avoids that.

### 3.4 Views

- **One confirm dialog**: `DownloadedPage.xaml.cs:52-63` and `ThemeDetailPage.xaml.cs:42-53` build the
  same "Remove …?" `ContentDialog`. Add `Views/Dialogs.cs` with
  `static Task<bool> ConfirmRemoveAsync(XamlRoot root, string themeName)`, keeping the text exactly.
- **One preview handler**: `ExpandButton_Click` and `PreviewMenu_Click` (`ThemeDetailPage.Preview.cs:72-82`)
  are identical. Keep one (`PreviewItem_Click`) and point `ThemeDetailPage.xaml:301` and `:322` at it.
- **`ShellPage` nav lookup**: the same lookup is written out at `ShellPage.xaml.cs:32-34` and `:84-86`.
  Extract it, which also fixes bug 7:

  ```csharp
  private NavigationViewItem? FindNavItem(string tag) =>
      NavView.MenuItems.Concat(NavView.FooterMenuItems).OfType<NavigationViewItem>()
          .FirstOrDefault(i => i.Tag as string == tag);
  ```

- **Fit combo**: `ApplyDialog.xaml.cs:21-23, 80` and `SettingsPage.xaml.cs:13-15, 22` both fill the combo
  with enum names and parse them back. Add a small `Helpers/FitChoices` with `Fill(ComboBox, WallpaperFit)`
  and `WallpaperFit? Selected(ComboBox)`. The items shown stay the same.
- **`ApplyDialog.ShowAsync`** (`:84-100`) resolves services itself and saves the "remember" choice. A view
  shouldn't write settings: return the options together with the remember flag, and let the caller (or
  the desktop store in 4.1) save them. Low priority.

### 3.5 XAML formatting and comments

- `ThemeDetailPage.xaml`: the root `<Grid>` (`:47`, closed at `:515`) doesn't indent its children (the
  `ScrollViewer` at `:48-424` and the preview overlay at `:426-514`). Indent both by four spaces.
- Attributes are alphabetical everywhere (XAML Styler order) except `Tag="{x:Bind}"` in
  `ThemeDetailPage.xaml:295` and `:323` and `DownloadedPage.xaml:93`. Move each into place.
- Remove comments that only name the next block: `ThemeDetailPage.xaml:81` (`Header`), `:184`
  (`Progress + notifications`), `:230` (`Body`); `GalleryPage.xaml:107` (`Header: title + subtitle…`),
  `:407` (`Error with nothing to show`), `:425` (`Search / filter matched nothing`); `DownloadedPage.xaml:137`
  (`Empty state`). The bound `Visibility` already says what they say.
  Keep the rest: they explain why (card proportions, "a card is a Button" for UI Automation, the shared
  scroll area, the GitHub mark's license, the hover button and keyboard access, the preview overlay's
  keys and focus, TitleBar caption heights, test-switch badges, csproj build notes).
- Repeated text styling. Move these into `Styles/AppStyles.xaml` and use them:
  - `SecondaryCaptionTextStyle` (Caption + `TextFillColorSecondaryBrush`): `GalleryPage.xaml:49-52, 245-248,
    281-284`; `ThemeDetailPage.xaml:26-30, 398-402, 494-497`; `DownloadedPage.xaml:63`. `SettingsPage`'s
    page-local `RowDescriptionStyle` is the same plus `TextWrapping="Wrap"`, so move it too.
  - `AccentBadgeTextStyle` (Caption + `TextOnAccentFillColorPrimaryBrush`): `GalleryPage.xaml:60-63, 239-242`;
    `ThemeDetailPage.xaml:110-113`; `DownloadedPage.xaml:57-60`.
  - `TertiaryCaptionTextStyle` (Caption + `TextFillColorTertiaryBrush` + Wrap): `ApplyDialog.xaml:47-53, 60-65`;
    `ThemeDetailPage.xaml:414-418`.
- `SettingsPage.xaml:38-208` repeats one icon | header + description | control grid seven times. A small
  `Controls/SettingsRow` UserControl (`Glyph`, `Header`, `Description`, content for the control) would
  cut it to about seven short elements. Keep the `AutomationProperties.Name`s on the toggles and buttons.
  The toolkit's `SettingsCard` would be a new dependency; the rules say no.

## 4. Larger SOLID and abstraction recommendations

Most valuable first.

### 4.1 Move the app's logic out of the view models into testable stores (as on macOS)

**Problem.** The view models mix UI state with policy and I/O: apply bookkeeping, which wallpaper to use,
the catalog refresh rules, error wording, terminal export, cache clearing. They depend on concrete types
(`ThemeStore`, `WindowsTerminalSchemes`, `GalleryViewModel`), read `DateTimeOffset.Now` directly, and
there's no App test project, so none of it is tested. This breaks single responsibility and dependency
inversion, and it's where bugs 1 and 5 live. The macOS app just made the same move:
`macos/OmarchyThemesKit/Sources/OmarchyThemesStores/`, tested in `Tests/OmarchyThemesStoresTests/`.

**Shape.** Add a platform-neutral project, `src/OmarchyThemes.Stores` (net10.0, referencing Core), with
`tests/OmarchyThemes.Stores.Tests` reusing Core's fakes (`FakeHttpHandler`, `FakeDownloader`,
`FakeDesktopBackend`, `InMemorySnapshotStore`, `ManualTimeProvider`; move them to a shared test-support
project or link the file). Both build and run on the Mac, so this part can be done there. View models
become thin WinUI adapters that turn store state into bindable properties and `InfoBarSeverity`.

| Store (macOS equivalent) | Takes over from |
|---|---|
| `Preferences` (`Preferences.swift`) | Direct `SettingsStore.Load()` calls. Holds settings in memory and raises a change event, so `ApplyService.ActiveSlug` stops re-reading `settings.json` on every badge and binding. |
| `DesktopStore` (`DesktopStore.swift`) | `ApplyService`, plus the apply code in `DownloadedViewModel.ApplyAsync`, `CurrentThemeViewModel.SetWallpaperAsync` (wallpaper only, saved fit) and `SettingsViewModel.RestoreOriginalAsync`. Also `PreferredWallpaper(theme)`, which `DownloadedViewModel.cs:94` and `ThemeDetailViewModel.cs:217-220` compute separately. Returns results, not InfoBar state. |
| `CatalogStore` (`CatalogStore.swift`) | `GalleryViewModel`: cached load, the refresh policy (12 h, or no default themes yet), the stale and missing-default-themes notices (`DescribeDefaultThemesError`), `Find` / `EntryFor(InstalledTheme)`, and search + downloaded-only + section filtering as a pure function. Takes a `TimeProvider`. |
| `ThemeLibrary` (`ThemeLibrary.swift`) | Installed themes, Remove with a failure result (bug 1), Clear Cache including the details memo (bug 5), download-failure wording (`ThemeDetailViewModel.cs:262-267`). |
| `TerminalStore` (`TerminalStore.swift`) | `ThemeDetailViewModel.Terminal.cs`, including `PaletteForTerminalAsync` (look up again for palettes saved before `muted`/`bright_foreground` were read), behind 4.2. |

Tests to write mirror macOS `StoresTests`: catalog load and freshness, a shared refresh, offline and
missing-default-theme notices; apply, one apply at a time, switching the current wallpaper (wallpaper only,
saved fit), restore and failed restore; remove and clear cache; terminal add/remove, failures, and
refreshing old downloads' bright colors.

Keep WinUI/MVVM conventions: stores are plain C# with events (or `INotifyPropertyChanged`, as
`DownloadOperation` already does), view models stay `ObservableObject`s, and x:Bind stays on the view
models.

### 4.2 A terminal-scheme interface in Core

`ThemeDetailViewModel` depends on the concrete `WindowsTerminalSchemes` and on its static
`IsTerminalInstalled()`, read once into a static field (`ThemeDetailViewModel.Terminal.cs:15`), so neither
can be faked. Add an interface to `Core/Palettes` (or `Core/Terminals`):

```csharp
public interface ITerminalSchemes
{
    bool IsAvailable { get; }
    string SchemeName(string themeName);
    bool IsAdded(string slug);
    void Add(string slug, string themeName, TerminalColors colors);
    void Remove(string slug);
}
```

`WindowsTerminalSchemes` implements it, with `IsAvailable` computed at construction (and injectable). This
matches the macOS `TerminalExporter` protocol, and a second Windows terminal (e.g. WezTerm) would be one
new class.

### 4.3 One place that decides which errors are expected, and what to tell the user

Catch lists differ from place to place: `ThemeDownloads.cs:138`, `CatalogService.cs:76`,
`GalleryViewModel.cs:191` and `ThemeDetailViewModel.cs:176`, `ThemeDetailViewModel.Terminal.cs:68`. The
gaps between them cause bug 1. Add to Core:

```csharp
public static class ExpectedErrors
{
    /// <summary>Network, disk and GitHub failures the UI reports instead of crashing (not user cancellation).</summary>
    public static bool IsExpected(Exception e, CancellationToken ct = default) =>
        e is HttpRequestException or IOException or UnauthorizedAccessException
            or GitHubException or ThemeResolveException or CatalogFormatException
        || (e is TaskCanceledException && !ct.IsCancellationRequested);

    /// <summary>"Couldn't reach GitHub…" for connection failures, else the exception's own message.</summary>
    public static string Describe(Exception e) => …;
}
```

Adding a new failure type then means editing one place (open/closed). Test it in Core.

### 4.4 Keep Windows wording out of Core

`ApplySummary.Describe` (`Theming/ApplySummary.cs:27`) adds "Some parts of Windows may only pick up the
new accent color after you sign out." Core is meant to be platform-neutral, and the macOS port had to drop
this sentence. Let the platform supply it: for example `IDesktopBackend.AccentColorNote` (null by default),
or a parameter the App passes. Update the `ApplySummaryTests` sign-out cases with it.

### 4.5 Small ones

- `DesktopWallpaperApi` returns `true` three times (`:74, 86, 116`) only because `Sta.RunAsync` takes a
  `Func<T>`. Add `Sta.RunAsync(Action)` and drop the dummy results. Verify with the live checks.
- `WindowsDesktopBackend.SetAccentColorAsync` computes `ToAbgr(color)` and `ToArgb(color, 0xC4)` twice.
  Locals would make the value list read like the registry table in `docs/ARCHITECTURE.md`.
- `ThemeDetailPage` subscribes the preview's handler through a `partial void InitializePreview()` hook.
  A direct call in the constructor is simpler; optional.

## 5. When done

- Delete each item from this file as it's completed. When the file is empty, delete it, and remove any
  pointer to it from `CLAUDE.md`.
- `CLAUDE.md` ("Building the Windows app") and `docs/ARCHITECTURE.md` (Tests) still say Core has 158
  tests. It's 161 now: update both, and add `JsonFile` and the ANSI swatch naming to the Core tests table.
- If 4.1 happens, add the new project to the repository layout and the Windows architecture section of
  `docs/ARCHITECTURE.md`, to the "where to look" table in `CLAUDE.md`, and to the `dotnet test` counts.
- Keep the Windows rules while doing this: never apply, restore or change the real desktop. Drive the UI
  only with `OMARCHY_THEMES_DRY_RUN=1` and `OMARCHY_THEMES_DATA_DIR`.
