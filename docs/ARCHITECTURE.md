# Omarchy Themes: architecture and development

How the two apps and the command-line tool are built, how they read Omarchy's themes and apply them,
and how they're tested. For what the app is and how to build it, see the [README](../README.md).

Each platform is its own native codebase so it can follow that OS's conventions (Mica + Fluent
`NavigationView` on Windows; sidebar, vibrancy and traffic lights on macOS) and call the real
desktop-theming APIs directly. The command-line tool (`omatheme`, Rust) does what both apps do from a
terminal, on either OS, and shares their data.

## Repository layout

```
design/AppIcon.svg                 icon source of truth (original artwork, not the Omarchy logo)
windows/
  OmarchyThemes.sln
  Directory.Build.props            shared C# settings
  Directory.Packages.props         central NuGet versions
  src/OmarchyThemes.Core/          net10.0, no Windows deps: catalog, GitHub, palettes, store, ThemeApplier
  src/OmarchyThemes.Platform.Windows/  IDesktopBackend for Windows (COM + registry, via CsWin32)
  src/OmarchyThemes.Stores/        net10.0: the app's logic as stores (catalog, library, desktop, terminal)
  src/OmarchyThemes.App/           WinUI 3 app (unpackaged, self-contained Windows App SDK)
  tests/OmarchyThemes.Core.Tests/  xUnit, platform-neutral
  tests/OmarchyThemes.Platform.Windows.Tests/  xUnit, Windows backend against fake registry/COM
  tests/OmarchyThemes.Stores.Tests/  xUnit, the stores wired to Core's fakes
  tools/Generate-AppIcon.ps1       renders Assets/AppIcon.png + .ico
macos/
  project.yml                      XcodeGen spec (the .xcodeproj is generated, not committed)
  App/                             SwiftUI app target
  OmarchyThemesKit/                Swift package
    Sources/OmarchyThemesKit/      no AppKit: catalog, GitHub, palettes, store, ThemeApplier
    Sources/OmarchyThemesMac/      DesktopBackend for macOS (NSWorkspace + ImageIO)
    Sources/OmarchyThemesStores/   the app's observable stores, built from injectable services
    Sources/OmarchyThemesTestSupport/  fakes and fixtures shared by the tests
    Tests/                         Swift Testing: Kit, backend and stores, all against fakes
  tools/generate-app-icon.swift    renders the asset-catalog icon set from design/AppIcon.svg
cli/                               Rust command-line tool (omatheme), one crate: library + thin binary
  src/                             catalog, GitHub, palettes, store, theming, platform/ (both backends), terminals/, cli/
  tests/live.rs                    opt-in live checks
fixtures/                          test fixtures shared by the Windows and Swift tests
docs/ARCHITECTURE.md               this file
docs/screenshots/                  README screenshots
```


## Development

### Test switches

Both apps read the same environment variables:

| Variable | Effect |
|---|---|
| `OMARCHY_THEMES_DRY_RUN=1` | Swaps in a backend that reports the platform's capabilities but changes nothing, so Apply and Restore can be exercised end to end without touching the desktop. The window shows a "Dry run" badge (on Windows, in the title bar). Terminal color exports stay in the data folder too: on Windows, Windows Terminal schemes; on macOS, iTerm2 and Ghostty files go to `dry-run-home/` there, and Terminal.app isn't opened. |
| `OMARCHY_THEMES_DATA_DIR=<path>` | Uses another data folder instead of `%LOCALAPPDATA%\OmarchyThemes` / `~/Library/Application Support/OmarchyThemes` (fresh first launch, no risk to real downloads or the saved original desktop). |

The command-line tool reads both too, and has `--dry-run` and `--data-dir <path>` flags that do the same
(see [Architecture (command line)](#architecture-command-line)).

Use both whenever the UI is driven by a script, for example on Windows:

```powershell
$env:OMARCHY_THEMES_DRY_RUN = '1'; $env:OMARCHY_THEMES_DATA_DIR = "$env:TEMP\omatheme-test"
dotnet run --project src/OmarchyThemes.App
```

### GitHub rate limit

`GITHUB_TOKEN` raises the GitHub rate limit (60 requests/hour unauthenticated). On macOS, apps started
from the Finder don't inherit your shell's environment, so set it with `launchctl setenv` or run the app
binary from a terminal.

### Logs

On Windows, errors and notable events (catalog, default themes, downloads, apply, restore) are logged to
`logs\omarchy-themes-yyyyMMdd.log` in the data folder (kept for a week) and to the debugger output. On
macOS they go to the unified log (`os.Logger`, subsystem `com.justinhhorner.OmarchyThemes`).

### Build notes

- **Windows:** the app project defaults to the host architecture; pass `-p:Platform=ARM64` (or `x64`)
  to cross-build. Solution and project builds share one output folder
  (`src/OmarchyThemes.App/bin/Debug/net10.0-windows10.0.26100.0/win-x64/`), so `dotnet run` never
  launches a stale build. Visual Studio 2022/2026 with the *WinUI application development* workload is
  optional (XAML designer and Hot Reload).
- **macOS:** SwiftSoup (HTML parsing) is the only package dependency and is fetched by SwiftPM. After
  `xcodegen`, open `OmarchyThemes.xcodeproj` in Xcode, or run the app's tests from the command line
  with `xcodebuild -scheme OmarchyThemes -derivedDataPath build/DerivedData test` (`swift test` in
  `OmarchyThemesKit` runs the package alone, no Xcode project needed). The app isn't sandboxed (it has
  to copy your current wallpaper from wherever it lives before the first Apply) and is built with the
  hardened runtime for Developer ID distribution. Debug builds are signed ad hoc.

## Architecture (macOS)

The macOS app is its own SwiftUI codebase following macOS conventions: a `NavigationSplitView`
sidebar (Gallery, Downloaded) with drill-in detail pages, a unified toolbar with search and refresh
(⌘R), a `Settings` window (⌘,), the standard About panel with credits, and "Show Welcome Screen"
in the Help menu.

- **OmarchyThemesKit** ports Core's behaviour (not its code) and has no AppKit: the catalog
  parser (SwiftSoup instead of AngleSharp), the ETag HTTP cache over a small `HTTPTransport`
  protocol (URLSession in the app, a routing fake in tests), the GitHub client, the palette parsers
  (a strict TOML reader for the subset palette files use, with the same lenient line-scanner
  fallback), `ThemeResolver`, `ThemeStore`, `ThemeApplier` and `ApplySummary`.
- **OmarchyThemesMac** implements `DesktopBackend` (`MacDesktopBackend`) behind a `WallpaperAPI`
  protocol, so it's tested without changing the desktop.
- **OmarchyThemesStores** holds the app's state and logic as `@MainActor` `@Observable` stores, each
  with one job: `CatalogStore` (entries, refresh, notices), `ThemeLibrary` (downloaded themes, GitHub
  lookups, downloads that carry on when you navigate away), `DesktopStore` (apply, the current theme,
  restore), `TerminalStore` (exporters and the selected terminal), `Preferences` (settings, welcome) and
  `Banners` (result messages). `AppStores` builds them from an `AppServices` struct; `AppServices.live()`
  wires up the real Mac (and the test switches), and tests pass fakes, so the stores are covered by
  `swift test` (`OmarchyThemesStoresTests`).
- **OmarchyThemesTestSupport** has the fakes and fixtures shared by the test targets.
- **App** is the views, plus a thin `AppModel` (the stores, the thumbnail loader, sidebar navigation
  and Clear Cache). `appEnvironment(_:)` puts each store in the SwiftUI environment, so a view depends
  only on the stores it uses.

The UI matches Windows feature for feature, in macOS form. The gallery opens with a **Current Theme**
card when the theme on the desktop is still downloaded (hidden while searching, shown even if the
catalog can't load, gone after Restore). It shows the theme's screenshot, light/dark, colors,
**View Theme** and a row of its wallpapers with the one on the desktop ringed and check-marked;
clicking another sets it straight away (wallpaper only, with the saved fit and the theme's fill
color). The current wallpaper is only recorded when the wallpaper step actually applied
(`AppSettings.afterApply`). Theme pages show wallpaper thumbnails without captions (the name is the
tooltip, the VoiceOver label and in the large preview).

### Terminal colors on macOS

A theme page's **Terminal Colors** section sends the theme's colors to a terminal app, picked from a
menu (iTerm2 until you pick another; the choice is saved as `terminalApp` in settings). The colors
come from `TerminalColors` in the Kit, the same mapping as Windows (Omarchy's own terminal template).
The scheme is always named "<Theme> (Omarchy)", as in Windows Terminal. Themes downloaded before
`muted`/`bright_foreground` were read are looked up again (usually from cache) for those colors.

Each terminal is a `TerminalExporter` (in `OmarchyThemesMac/Terminals/`). An exporter only adds its
own files and never edits the terminal's settings:

| Terminal | What Add does | Remove |
|---|---|---|
| iTerm2 (default) | Writes a Dynamic Profile, `~/Library/Application Support/iTerm2/DynamicProfiles/omarchy-themes-<slug>.json`, with a stable GUID. iTerm2 picks it up live. | Deletes the file. |
| Ghostty | Writes a theme file, `$XDG_CONFIG_HOME/ghostty/themes/<Theme> (Omarchy)` (`~/.config` by default), tagged with a `# omarchy-themes-slug:` comment so a renamed theme still finds its file and two themes with the same name don't overwrite each other (Add says so instead). The user sets `theme = "…"` and reloads the config. Counts as installed if the app or a Ghostty config folder exists. | Deletes the theme's files. |
| Terminal.app | Writes a `.terminal` profile (keyed-archived `NSColor`s) to `terminal/` in the data folder and opens it with Terminal, which imports it and opens a window. Whether it's there is read from Terminal's preferences (read-only). | Manual: the button explains how (Terminal › Settings › Profiles, −), since removing it would mean editing Terminal's settings. |

To add a terminal, write a type conforming to `TerminalExporter` (`ITermExporter` is the smallest
example) and add it to `TerminalExporters.all(in:)`. Everything outside the app (home and config
folders, finding and opening apps, reading another app's preferences) goes through
`TerminalEnvironment`, so exporters are tested against temp folders and fakes.

Local data lives in `~/Library/Application Support/OmarchyThemes`, with the same layout **and file
format** as on Windows (see [data-format.md](data-format.md)). Screenshot and wallpaper thumbnails are
also cached in `cache/images/<id>/` (macOS only), a URLCache. Clear Cache starts a new `<id>` folder
and deletes the old one: `removeAllCachedResponses` leaves URLCache's disk store behind, and its files
can't be deleted while it's in use.

### Applying a theme on macOS

| Aspect | Mechanism |
|---|---|
| Wallpaper | `NSWorkspace.setDesktopImageURL(_:for:options:)` for every `NSScreen`. Fill, Fit, Stretch and Center map to `imageScaling` + `allowClipping`; the theme's background color is passed as `fillColor` for the area around the image. WebP and BMP wallpapers are converted to PNG with ImageIO first (into a hidden `.converted` folder next to the file), because it's undocumented whether the desktop displays them. |
| Light / dark | Not supported: macOS has no public API for an app to change the system appearance. The Apply sheet shows it disabled, with a link to Appearance settings. |
| Accent color | Not supported, for the same reason. |

Safety is the same as on Windows. Nothing changes on launch, and nothing changes until you apply a
theme. Before the first Apply, each screen's desktop picture and options are saved, along with a
private copy of each picture (except the built-in ones under `/System`, which are always there).
**Restore my original desktop** in Settings puts them back, falling back to the private copy if the
original file is gone. A display connected after the snapshot gets the main display's picture. If
the snapshot can't be taken, nothing is applied.

Known limitations:

- `setDesktopImageURL` changes the **current Space** on each display, not every Space.
- Dynamic and Aerial (video) wallpapers are restored as the picture file the system reports, which
  may be a still image.
- Removing a downloaded theme deletes its wallpapers, including the one on your desktop; the
  confirmation says so.

## Architecture (Windows)

- **Core** holds all logic that doesn't touch the OS, so it's unit-tested without Windows:
  catalog parsing, GitHub repo resolution with an ETag cache, palette parsers, the on-disk theme
  store, and `ThemeApplier`, which drives an `IDesktopBackend` abstraction. It also owns
  **downloads** (`ThemeDownloads`, keyed by theme): they outlive the page that started them, so
  leaving a theme mid-download neither cancels nor orphans it, coming back shows the live
  progress, and pressing Download again joins the running download. Concurrent lookups of one
  theme share a single GitHub request (`ThemeDetailsService`). `ExpectedErrors` decides which
  failures are part of normal use (network, disk, GitHub) and are reported rather than crashing,
  and words the common ones.
- **Platform.Windows** implements `IDesktopBackend` (`WindowsDesktopBackend`). Each OS touchpoint
  sits behind a small interface (`IRegistryAccess`, `IWallpaperApi`, `ISettingsBroadcaster`,
  `IImageConverter`) so the backend is tested without changing the machine it runs on. It also has
  `WindowsTerminalSchemes`, the `ITerminalSchemes` (Core) for Windows Terminal.
- **Stores** holds the app's logic, platform-neutral and tested like Core: `Preferences`
  (settings in memory), `CatalogStore` (cached load, the 12-hour refresh rule, shared refreshes,
  notices, search filtering), `ThemeLibrary` (downloaded themes, lookups, downloads, removal, Clear
  Cache), `DesktopStore` (apply, one at a time, the current theme and wallpaper, restore) and
  `TerminalStore`. They return a `Banner` (or `ApplySummary`) for each outcome and raise `Changed`
  events; they mirror the macOS stores.
- **App** is the WinUI 3 UI (MVVM with CommunityToolkit.Mvvm, DI via
  Microsoft.Extensions.DependencyInjection). Its view models are thin adapters that turn store state
  into bindable properties and `InfoBar`s; new logic goes in a store, with a test. Unhandled
  exceptions are logged with their stack trace before the app exits.

Local data lives in `%LOCALAPPDATA%\OmarchyThemes`:

```
cache/                  HTTP cache (catalog page, GitHub trees, palette files) + ETags
themes/<slug>/          theme.json manifest, wallpapers/, screenshot
settings.json           welcome seen, one-click apply defaults, last applied theme
original-desktop.json   snapshot taken before the first Apply
original-desktop/       private copies of the original wallpapers, used by Restore
logs/                   daily log files, kept for a week
```

The files are in the format both apps share, documented in [data-format.md](data-format.md).

### The UI

| View | What it does |
|---|---|
| Welcome | First launch only (and from Settings): what the app is, links to omarchy.org and basecamp/omarchy, **Browse Themes**. |
| Current theme | Above the gallery (hidden while searching): the theme on the desktop, if it's still downloaded, with its screenshot, light/dark, colors and **View theme**, plus a row of its wallpapers with the one on the desktop marked. Clicking another wallpaper sets it straight away: wallpaper only (saved fit, theme fill color), since the theme's mode and accent are already applied. Disappears after **Restore my original desktop**. |
| Gallery | Two sections, **Included with Omarchy** and **Community**, each with a count; search (Ctrl+F) and the "downloaded only" filter apply to both, and an empty section is hidden. Screenshot cards in two virtualizing `ItemsRepeater`s (`UniformGridLayout`) sharing one scroll area, with arrow-key navigation between cards. Refresh (F5). Shows the cached catalog instantly and refreshes in the background when it's older than 12 hours (or has no default themes yet). Has loading, error, empty-result and "showing cached copy" states, plus an "Omarchy's themes are missing" warning when the default themes couldn't be listed. |
| Theme detail | Large screenshot, key colors and terminal swatches, wallpaper picker, light/dark badge, **View on GitHub** (GitHub's mark). **Download** shows per-file progress in bytes and can be cancelled, and keeps going if you leave the page; **Download and apply** / **Apply to desktop** opens the Apply dialog. Downloaded themes render entirely from disk. |
| Windows Terminal | On the theme page (downloaded or not): **Add to Windows Terminal** turns the theme's terminal colors into a Windows Terminal color scheme named "<Theme> (Omarchy)", through Terminal's [JSON fragment extensions](https://learn.microsoft.com/windows/terminal/json-fragment-extensions): one file per theme in `%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\OmarchyThemes\`. The user's Terminal settings are never edited; **Remove** deletes the file. Terminal picks it up when it starts; the scheme is then chosen per profile (or under Profiles › Defaults). With the test switches, fragments go to the data folder instead. |
| Wallpaper preview | Shows a wallpaper large over the theme page: from the **Preview** button, a hover button on a thumbnail, double-click, or the thumbnail's context menu. Full resolution (decoded at up to 2560 px) with a spinner while a remote image loads; previous/next (also ← / →, wrapping), file name and "3 of 8", **Use this wallpaper** (selects it for Apply) and close (also Esc, or click outside the bar). Focus moves into the preview and back to the thumbnail. |
| Apply dialog | One checkbox per aspect (wallpaper + fit, light/dark, accent). Options the OS or theme can't provide are disabled with a reason. Can save the choices as one-click defaults. |
| Downloaded | Downloaded themes with one-click **Set as desktop theme** (uses the saved defaults), plus Apply with options, View details and Remove. |
| Settings | One-click apply defaults, **Restore my original desktop**, storage and cache, GitHub rate-limit info, show welcome. |
| About | Credits Omarchy with links to omarchy.org, the theme gallery and basecamp/omarchy. |

Windows conventions: Mica backdrop, the `TitleBar` control with back and pane buttons and standard
caption buttons, `NavigationView`, Fluent cards, `InfoBar` for every network or apply outcome, and
following the system light/dark setting.

### Catalog parsing

The live omarchy.org/themes page is a minified Astro page where each theme is
`ul > li > a[href="https://github.com/…"] > img[src="/assets/themes/<slug>.webp"] + span(name)`
(there are no `<figure>` elements, despite older descriptions). The parser selects those anchors
and resolves screenshot URLs against the page URL. A `<figure>` layout is accepted as a fallback,
and entries without a GitHub link are skipped. The raw page (not the parsed result) is cached, so
the app starts instantly and offline, and a parser fix applies to the cached copy too. If the page
ever yields zero themes, the app says the layout may have changed instead of showing an empty
gallery.

### Omarchy's default themes

The themes that ship with Omarchy (Tokyo Night, Catppuccin, Gruvbox and the rest) aren't on
omarchy.org/themes; they live in the `themes/` folder of
[omacom/omarchy](https://github.com/omacom/omarchy/tree/quattro/themes). Both apps list them first
in the gallery, "Included with Omarchy". They come from one cached call for the repo's file tree
(`HEAD`, currently the `quattro` branch), which also serves every default theme's palette and
wallpapers, so opening one costs no extra API request. Names follow Omarchy's own
`omarchy-theme-list` (folder `retro-82` → "Retro 82"), the screenshot is each theme's
`preview.png`, and slugs are `omarchy.<folder>` so they can never collide with community themes.
If GitHub can't be reached (or is rate-limited) with nothing cached, the community gallery still
loads without them, and both apps say so with a warning above the gallery.

### Theme resolution (GitHub)

Repos are resolved only when a theme is opened. That costs one API call,
`GET /repos/{owner}/{repo}/git/trees/HEAD?recursive=1`, sent with `If-None-Match`; a 304 reply
doesn't count against the 60/hour unauthenticated limit. File contents come from
`raw.githubusercontent.com`, which isn't API-metered. If `GITHUB_TOKEN` is set, it's used to raise
the limit.

- **Palette:** `colors.toml` (Omarchy's named-key format with `mode`/`accent`/`background`…, or the
  older `color0..color15` format), falling back to `alacritty.toml`. Themes with neither show a
  "couldn't read this theme's palette" state.
- **Wallpapers:** images under `backgrounds/`.
- **Light/dark:** `mode = "light"` or a `light.mode` file; otherwise inferred from background
  luminance.

### Applying a theme on Windows

Everything is per-user (HKCU) and needs no elevation.

| Aspect | Mechanism |
|---|---|
| Wallpaper | `IDesktopWallpaper::SetBackgroundColor` (the theme's `background`, so Fit/Center wallpapers are framed in the theme's color instead of black) + `SetPosition` (the chosen fit) + `SetWallpaper(NULL, path)` for every monitor, on an STA thread; `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` fallback. WebP and other formats are first converted to PNG with WIC (`BitmapDecoder`/`BitmapEncoder`). |
| Light / dark | `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` `AppsUseLightTheme` + `SystemUsesLightTheme`, then `WM_SETTINGCHANGE("ImmersiveColorSet")` via `SendMessageTimeout(SMTO_ABORTIFHUNG)` |
| Accent color | `HKCU\…\DWM` `AccentColor` (ABGR) + `ColorizationColor`/`ColorizationAfterglow` (ARGB); `HKCU\…\Explorer\Accent` `AccentPalette` (8 × RGBA shades), `AccentColorMenu`, `StartColorMenu`; `HKCU\Control Panel\Desktop` `AutoColorization=0`; then the same broadcast. Near-black or near-white accents are lifted to a readable lightness first. |

Safety:

- Nothing changes on launch or until the user picks a theme and applies it.
- Before the first Apply, the backend snapshots every registry value it may touch (including
  "value was absent") and each monitor's wallpaper. It keeps a private copy of each wallpaper
  file, because Windows' `TranscodedWallpaper` is overwritten by the next wallpaper change.
  **Restore my original desktop** writes the values back (deleting ones that didn't exist before)
  and restores the per-monitor wallpapers, fit and desktop fill color (which is system-wide).
- If the snapshot can't be taken, nothing is applied. If a saved snapshot exists but can't be read,
  nothing is applied either (rather than overwriting it with the themed desktop); Restore says why.
  The private wallpaper copies are made in a staging folder and swapped in only when all succeed.
- Downloads, reinstalls and removals never leave a half-installed theme: a reinstall renames the old
  copy aside and deletes it only once the new one is in place, and the next launch puts back a copy
  a failed reinstall left aside. Cache writes are atomic with unique temp files and never fail a
  request that succeeded.

Known limitations:

- **Accent color:** Windows has no public API for it. The values above mirror what the Settings
  app stores, and most surfaces pick them up from the broadcast, but some only refresh after
  signing out and back in. The UI says so.
- **Restoring a slideshow or Windows Spotlight background** puts back the image that was showing
  when the snapshot was taken, not the slideshow itself.
- **WebP wallpapers** need the WebP codec. It's built into Windows 11; on Windows 10 it comes with
  the "WebP Image Extensions" Store package. The error message says so if it's missing.

### Adding a new platform theming API

1. Add a capability to `DesktopCapabilities` in Core, along with the matching method on
   `IDesktopBackend`.
2. Teach `ThemeApplier` when to call it (it gates every step on the backend's capabilities and on
   the user's Apply options), and extend `DesktopSnapshot` if the setting should be restorable.
3. Implement it in the platform backend (`Platform.Windows`, or the macOS app's equivalent) and
   report the capability. On Windows, add any registry values it writes to
   `WindowsDesktopBackend.TrackedValues` so they're snapshotted and restored automatically. Put
   new Win32/COM entry points in `NativeMethods.txt` (CsWin32 generates the bindings).
4. Add a checkbox to the Apply dialog and a toggle in Settings (`ApplyOptions` in Core).
5. Add tests: a `ThemeApplier` test with the fake backend, and a `WindowsDesktopBackendTests`
   case asserting exactly which values are written.

## Architecture (command line)

`omatheme` is one Rust crate in `cli/`: a library with all the logic (tested with `cargo test`, no
network or OS changes) and a thin `main.rs`. It ports the Kit/Core behaviour, not the code:

| Module | What it holds |
|---|---|
| `catalog`, `github`, `net`, `resolver` | The omarchy.org parser (`scraper`), Omarchy's default themes, the ETag disk cache over a `Transport` trait (ureq, or a routing fake in tests), the one-call-per-theme GitHub client, theme resolution |
| `palette` | Both `colors.toml` shapes and `alacritty.toml` (the `toml` crate, falling back to the same lenient line scanner), `TerminalColors` |
| `store` | `theme.json`/`settings.json` in the shared format, the staged theme store (the Windows app's rename-aside reinstall and removal), the streaming downloader |
| `theming` | `DesktopBackend`, `ThemeApplier`, `ApplySummary`, the snapshot stores, Windows accent math, the dry-run backend |
| `platform` | Both OS backends: `mac.rs` and `windows.rs` hold each backend's logic behind small traits (wallpaper API, registry, broadcast, image conversion) and are compiled and tested on every OS; `mac_native.rs` (objc2: NSWorkspace, NSKeyedArchiver, read-only NSUserDefaults) and `windows_native.rs` (`windows` crate: IDesktopWallpaper on the main thread as an STA, HKCU registry, `SendMessageTimeout`) are the only `cfg`-gated code |
| `terminals` | `TerminalExporter` and its registry (`terminals::all`): Windows Terminal on Windows; iTerm2, Ghostty and Terminal.app on macOS; Ghostty elsewhere. Same files, names and messages as the apps |
| `cli` | clap arguments, the context (data folder, switches, services), theme-argument matching, output, one module per command group, and `cli/tui` (the full-screen interface) |

Commands:

```
omatheme list [--search TEXT] [--downloaded] [--default | --community] [--refresh] [--json]
omatheme show <THEME> [--json]
omatheme download <THEME>... [--force]
omatheme apply <THEME> [--wallpaper N|NAME] [--fit FIT] [--no-wallpaper] [--mode|--no-mode]
                       [--accent|--no-accent] [--yes] [--json]
omatheme current [--json]
omatheme wallpaper <N|NAME|next|prev> [--fit FIT] [--yes] [--json]
omatheme restore [--yes] [--json]
omatheme remove <THEME> [--yes]
omatheme terminal apps [--json] | add <THEME> [--app ID] | remove <THEME> [--app ID]
omatheme cache clear
omatheme paths [--json]
omatheme tui                        # alias: ui
```

Global flags: `--dry-run`, `--data-dir <path>`. `<THEME>` is a slug, a name, a default theme's folder
name (`tokyo-night`), or a unique start of any of them; an exact slug wins, then an exact name, then a
prefix, and ambiguous input lists the candidates. Exit codes: 0 success, 1 failure, 2 usage (including
an ambiguous theme or a missing `--yes`), 3 partly applied.

- **Catalog:** a cached catalog younger than 12 hours (with default themes) is used without a request,
  as the apps do on launch; otherwise, or with `list --refresh`, it's revalidated, falling back to the
  cache. Downloaded themes the catalog doesn't list are still listed and can be named.
- **Apply:** starts from the app's one-click defaults (`applyDefaults` in settings.json), changed by the
  flags; the fit must be one this OS supports (a saved Tile/Span falls back to Fill on macOS). It shows
  what will change and asks on a terminal; without one it refuses unless `--yes`. Each step's outcome
  is listed with the apps' `ApplySummary` wording, and `lastAppliedSlug`/`lastAppliedWallpaper` are
  updated with `AfterApply`. `wallpaper` is the Current Theme card's wallpaper switch (wallpaper only,
  saved fit). `apply` downloads the theme first if needed.
- **Output:** human-readable by default, `--json` for scripts; colors (and truecolor swatches) only on
  a terminal, never with `NO_COLOR`; progress (bytes, per file) on stderr only when it's a terminal.
  The first Ctrl-C stops a download between chunks (the staging folder is deleted); a second quits.
- **TLS:** the OS's own stack on macOS and Windows (ureq's `native-tls`: Security.framework, SChannel),
  so nothing is compiled from C there; rustls elsewhere. WebP and BMP are converted with the `image`
  crate (so Windows 10 doesn't need Microsoft's WebP extension), into the same place as the app on that
  OS (`.converted/<name>.png` on macOS, `.<stem>.wallpaper.png` on Windows).

### The TUI (`omatheme tui`)

A full-screen interface with ratatui (crossterm backend): the theme list on the left (Omarchy's own
first, then the community; ✓ downloaded, ● on the desktop), and on the right the selected theme's
screenshot (or, when browsing a downloaded theme's wallpapers, the wallpaper under the cursor) drawn
with `▀` half blocks in truecolor, its colors and terminal colors as swatches, and its wallpapers.
Keys: `/` search, `o` filter (all, downloaded, Omarchy's, community), `⏎` open, `d` download (progress
in the footer), `a` apply (a dialog like the apps': each aspect, unsupported ones disabled, the fit),
`w` or `⏎` on a wallpaper to switch the current theme's wallpaper (asks first), `t` terminal colors,
`x` remove, `r` restore (asks first), `R` refresh, `?` help, `q` quit.

It's split like the rest of the CLI: `tui/app.rs` is the state and key handling, which returns
`Effect`s and is tested without a terminal; `tui/view.rs` draws it (tested with ratatui's
`TestBackend`); `tui/mod.rs` is the runtime that carries out the effects. It reuses the commands'
logic (`CatalogLoader`, `apply_and_record`, `restore_and_record`, `choose_fit`,
`preferred_wallpaper`). The catalog, lookups, previews and downloads run on background threads and
report over a channel, so the screen stays responsive; applying runs on the main thread (AppKit).
Community themes cost a GitHub API call, so they're looked up on `⏎`; default themes (their tree is
shared and cached, their files aren't metered) and screenshots load once the selection rests for
250 ms. Quitting during a download cancels it between chunks and waits, so nothing is left
half-installed. It needs a terminal (exit 2 otherwise), honours the test switches (with a badge in
the header), and `NO_COLOR` turns off the swatches and previews.

### Sharing data with the apps

The CLI uses the app's data folder on the same OS (`$XDG_DATA_HOME/omarchy-themes` elsewhere) and
writes every file exactly as [data-format.md](data-format.md) says, including the HTTP cache, and keeps
unknown keys when it rewrites `settings.json`. `original-desktop.json` uses that OS's backend keys
(`screens`/`screen.url.…`/`screen.options.…`/`screen.copy.…` on macOS, `reg:…`/`monitor:…`/
`monitor-copy:…`/`wallpaper-fit` (the .NET enum name)/`wallpaper-background` on Windows), so either
client restores what the other saved. It reads `terminalApp` but never writes it (`--app` is per run),
and never writes `applyDefaults`.

Interrupted downloads (`.staging-`, `.removed-` and `.old-` folders in `themes/`) are tidied up only
when older than six hours, so the CLI never deletes a download the app is running.

Test switches: `--dry-run`/`OMARCHY_THEMES_DRY_RUN=1` uses a backend that reports this OS's
capabilities but changes nothing, lists what a real run would have done, and keeps terminal exports in
the data folder (`dry-run-home/`, and Windows Terminal fragments in `windows-terminal-fragments/` also
with another data folder, as the Windows app does); nothing is opened. With another data folder it
behaves like the apps' dry run, so restore can be exercised end to end; **against the real data folder
it writes neither settings.json nor original-desktop.json**, so a pretend desktop can never replace
the real one (downloads still go to the data folder, as in the apps). Confirmations are still asked in
a dry run.

On Linux and other OSes, `apply`, `wallpaper` and `restore` say applying isn't supported there and exit
1; everything else works (Ghostty is the terminal).

## Tests

`dotnet test` in `windows/` runs the three suites. No OS state is touched and nothing hits the network.

**Core (`OmarchyThemes.Core.Tests`)**

| Area | Covered |
|---|---|
| Catalog parser | live `li > a > img + span` markup, `<figure>` layout, relative screenshots, skipping nav/non-GitHub/duplicate cards, unique slugs |
| Default themes | folders under `themes/` only, Omarchy naming, `preview.png` screenshots, listed first, community-only fallback when GitHub fails, offline from cache, one shared tree call for listing and resolving |
| Repo links | `.git`, trailing slashes, `/tree/<ref>/<subdir>`, rejecting `/compare`, `/issues`, non-GitHub hosts, path traversal |
| Palettes | both `colors.toml` shapes, `alacritty.toml`, accent/mode fallbacks, lenient parsing of invalid TOML, both formats naming the 16 ANSI swatches the same way (`AnsiColors`) |
| HTTP cache / GitHub | ETag 304 revalidation, max-age, stale-when-offline, rate-limit reset time, 404, token sent only to the API, the shared User-Agent, a failed cache write still returning the response |
| JSON files | atomic writes with other serializer options (UTF-8 without a BOM), no temp file left after success or failure, the previous file kept when a write fails |
| Theme resolution | palette priority, `light.mode`, wallpaper discovery and natural ordering, repo sub-folders |
| Store | atomic install (no partial theme on failure/cancel), reinstall (the old copy kept if the swap fails), remove (nothing removed while in use), cleanup restoring a copy left aside, settings round-trip |
| Downloads | starting again joins the running download (each file fetched once), progress in bytes across files ending at 100%, cancel leaves nothing behind, failures reported not thrown; concurrent lookups share one request, and one caller giving up doesn't cancel the shared lookup, clearing the details memo (a lookup in flight doesn't refill it) |
| `ThemeApplier` (fake backend) | snapshot-before-first-apply, abort if snapshot fails, never overwriting an unreadable snapshot, per-step user/capability/data gating, partial failure, restore, the theme background passed as the wallpaper fill color |
| Accent math | ABGR/ARGB packing, 7-shade palette, `AccentPalette` bytes, normalizing unusable accents |
| Apply summaries | success (with the backend's accent note), partial failure, total failure, snapshot failure and all-skipped messages |
| Expected errors | which failures are reported rather than crashing, caller cancellation, the "Couldn't reach GitHub" wording |
| Terminal colors | named `colors.toml` mapped like Omarchy's own terminal template (black = background, white = foreground, bright black = `muted`, bright white and cursor = `bright_foreground`), `color0..15` and alacritty used as-is, fallbacks for missing colors |

**Windows backend (`OmarchyThemes.Platform.Windows.Tests`)**, run against in-memory registry,
wallpaper and broadcast fakes:

| Area | Covered |
|---|---|
| Light/dark | both `Personalize` values + `ImmersiveColorSet` broadcast |
| Accent | exact DWM/Explorer values and byte layout, `AutoColorization=0`, near-black accents lifted, nothing written outside `TrackedValues` |
| Wallpaper | conversion before setting, fit and exact fill color passed through, `COLORREF` packing (0x00BBGGRR) |
| Snapshot/restore | registry values restored and previously-absent values deleted, per-monitor wallpapers, fit and fill color, falling back to the private copy when the original file is gone, JSON round-trip, a failed capture keeping the previous copies |
| WIC conversion | real Windows Imaging Component on temp files: PNG output, reuse, actionable error for unreadable images |
| Windows Terminal fragments | a complete scheme (name + all 16 colors, which Terminal requires), UTF-8 without a BOM, schemes only (no profile changes), replace/remove/cleanup, unusual names, slug path safety |

**Stores (`OmarchyThemes.Stores.Tests`)** build the real stores from Core's fakes (linked from
`OmarchyThemes.Core.Tests/TestSupport/Fakes.cs`) and an in-memory terminal, mirroring the macOS stores
suite: settings saved (or kept for the session when the file can't be written); catalog load, cache
freshness, shared refreshes, offline, format-change and missing-default-theme notices, search
filtering; downloads, failed and cancelled downloads, Clear Cache, removal and a removal that fails;
apply, one-click apply with the saved defaults, remembering the Apply dialog's choices, one apply at a
time, switching the current wallpaper (wallpaper only, saved fit), an apply whose settings can't be
saved, restore and failed restore; terminal add/remove, failures, and refreshing old downloads' bright
colors.

Fixtures are hand-written to mirror the real page and theme repos rather than copied from them.

**Live checks** (opt-in, read-only, skipped by default) run against the real site, GitHub and this
PC's desktop:

```powershell
$env:OMATHEME_LIVE = '1'; dotnet test --filter Category=Live
```

Core parses omarchy.org (at least 100 community themes) and lists the default themes (at least 10), resolves Tokyo Night,
Aetheria and Vulkanite (both `colors.toml` shapes, WebP wallpapers), and downloads one wallpaper
checking that progress is reported in bytes and ends at the file size. The backend reads the
current wallpaper, fit and fill color through `IDesktopWallpaper`, and every tracked registry value
(checking it survives a snapshot round trip). About 3 GitHub API calls per run.

### macOS tests

`swift test` in `macos/OmarchyThemesKit` (or `xcodebuild … test`) runs both Swift Testing suites.
They use the same `fixtures/` as the Windows tests, touch no OS state and make no network requests.

**Kit (`OmarchyThemesKitTests`)** mirrors the Core suite above (catalog parser, repo links,
palettes, HTTP cache and GitHub client, theme resolution, store and settings, `ThemeApplier`,
apply summaries), including the terminal color mapping (`TerminalColorsTests`) and recording the
current wallpaper only when the wallpaper step applied (`AfterApplyTests`), each with the same cases as
on Windows. It also covers the strict TOML reader (inline tables, quoted and dotted keys, escapes,
multi-line strings, arrays), Windows (CRLF) line endings in both TOML readers, and checks that
concurrent applies never interleave.

**Stores (`OmarchyThemesStoresTests`)** build the real stores from fake services (routed HTTP, a
downloader that writes the URL into each file, a recording desktop backend, in-memory terminals): catalog
load, cache freshness, shared refreshes, offline and missing-default-theme notices; downloads (shared,
failed, cancelled), memoized lookups and Clear Cache, removal, looking a removed theme up again, the
wording of failed lookups; apply, one apply at a time, switching the current wallpaper (wallpaper only,
saved fit), restore and failed restore, fit fallback; terminal selection, add/remove, manual removal,
failures, and refreshing old downloads' bright colors.

**macOS backend (`OmarchyThemesMacTests`)**, against an in-memory `WallpaperAPI`:

| Area | Covered |
|---|---|
| Wallpaper | every screen set, fit → `imageScaling`/`allowClipping`, fill color, WebP converted before setting, a failing display doesn't stop the others (the error says how many changed) |
| Snapshot/restore | per-screen picture and options, one private copy per file, system pictures not copied, a failed capture keeps the previous copies, falling back to the copy when the original is gone, a display connected later, failure only when every screen fails it tried, no display connected keeps the snapshot |
| Capabilities | wallpaper only; through `ThemeApplier`, light/dark and accent report "not supported" |
| ImageIO conversion | real ImageIO on temp files: WebP decoding, BMP → PNG, reuse, actionable error for unreadable images |
| Terminal exporters | against a fake Mac (temp home, chosen installed apps, recorded "open" calls): registry order and default (iTerm2), shared scheme name, iTerm2 Dynamic Profile JSON (every color as sRGB components, stable GUID, add/remove), Ghostty theme file (all keys and 16 palette entries, config folder counts as installed, renamed themes, same-name themes, untagged older files), Terminal.app `.terminal` plist (keyed-archived NSColors, opened with Terminal, profile detection read-only, removal manual) |

`OMATHEME_LIVE=1 swift test --filter LiveChecks` runs opt-in, read-only checks against the
live site, a few theme repos and this Mac's current desktop. During development they parsed all 146
themes, resolved Aetheria and Vulkanite (WebP wallpapers, decoded and converted to a 3840×2160 PNG),
and checked the downloader's byte-level progress.

### Command-line tests

`cargo test` in `cli/` runs the unit tests (249), using the shared `fixtures/` (read from the repo root),
a routing fake transport, a fake downloader, a recording desktop backend and temp folders. They touch
no OS state and make no network requests. They mirror the Kit/Core suites case for case: catalog parser,
default themes, repo links, palettes and the TOML readers (CRLF included), terminal colors (halves round
to even), HTTP cache and GitHub client (304, max-age, stale offline, rate limit, the token only to the
API), resolver, theme store (atomic install, cancel and failure leave nothing, a failed swap keeps the
old copy, removal, cleanup), `ThemeApplier`, `ApplySummary` and `AfterApply`, and the data format against
`fixtures/data/` (writes the samples, reads every legacy file, keeps unknown settings). Both backends'
logic is tested on every OS against fakes (`platform::mac` and `platform::windows`: the macOS and
Windows backend suites' cases, plus restoring a snapshot in the other app's exact format), and the
exporters against temp folders (`terminals::*`). The CLI layer has tests for theme matching, wallpaper
choice, fits, the apply flags, filtering and argument parsing (including the credits in `--help`).

The TUI's state and keys (`cli::tui::app`: navigation, search, filters, the dwell before free lookups,
which preview to show, the apply dialog's defaults and disabled rows, what each confirmation does,
one download at a time) and its drawing (`cli::tui::view`: sections and markers, colors from disk,
lookup states, dialogs, download progress, half-block previews, tiny terminals) are unit-tested. The
real interface was also driven in a pseudo-terminal with the test switches, reading the screen back
with a terminal emulator (`pyte`): start, search, open, look up, apply, switch wallpaper, terminal
colors, restore, download, help, and quitting mid-download.

The Windows code is type-checked from a Mac with `cargo clippy --target x86_64-pc-windows-msvc`
(`rustup target add x86_64-pc-windows-msvc`). Its logic is tested everywhere, but its native layer
(`platform/windows_native.rs`) hasn't run on a Windows machine yet.

Opt-in live checks (read-only, about three GitHub API calls):

```bash
OMATHEME_LIVE=1 cargo test -- --ignored --test-threads=1
```

They parse omarchy.org (146 community themes) and list the default themes (22), resolve Tokyo Night and
Vulkanite, download a WebP wallpaper checking progress in bytes ends at the file size and that it
converts to a 3840×2160 PNG, and read (never set) the current wallpaper through the built binary, since
AppKit wants the main thread.
