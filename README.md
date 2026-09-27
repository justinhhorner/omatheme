# Omarchy Themes

A native desktop companion for the [Omarchy](https://omarchy.org) community theme gallery
([omarchy.org/themes](https://omarchy.org/themes/)). Omarchy themes are built for Linux/Hyprland; this
app doesn't run their configs. It reads each theme's **wallpaper and color palette** and re-applies
the equivalent look using the host OS's own theming APIs.

Omarchy is by DHH and contributors: https://github.com/basecamp/omarchy. This project is not
affiliated with Omarchy or 37signals; every theme belongs to its author.

| Platform | Stack | Status |
|---|---|---|
| Windows 10 (19041+) / 11 | WinUI 3 · Windows App SDK 2.5 · .NET 10 | Working v0.1 (`windows/`) |
| macOS 14+ | SwiftUI · Swift 6 · XcodeGen | Working v0.1 (`macos/`) |

Each platform is its own native codebase so it can follow that OS's conventions (Mica + Fluent
`NavigationView` on Windows; sidebar, vibrancy and traffic lights on macOS) and call the real
desktop-theming APIs directly.

## Repository layout

```
design/AppIcon.svg                 icon source of truth (original artwork, not the Omarchy logo)
windows/
  OmarchyThemes.sln
  Directory.Build.props            shared C# settings
  Directory.Packages.props         central NuGet versions
  src/OmarchyThemes.Core/          net10.0, no Windows deps: catalog, GitHub, palettes, store, ThemeApplier
  src/OmarchyThemes.Platform.Windows/  IDesktopBackend for Windows (COM + registry, via CsWin32)
  src/OmarchyThemes.App/           WinUI 3 app (unpackaged, self-contained Windows App SDK)
  tests/OmarchyThemes.Core.Tests/  xUnit, platform-neutral
  tests/OmarchyThemes.Platform.Windows.Tests/  xUnit, Windows backend against fake registry/COM
  tools/Generate-AppIcon.ps1       renders Assets/AppIcon.png + .ico
macos/
  project.yml                      XcodeGen spec (the .xcodeproj is generated, not committed)
  App/                             SwiftUI app target
  OmarchyThemesKit/                Swift package
    Sources/OmarchyThemesKit/      no AppKit: catalog, GitHub, palettes, store, ThemeApplier
    Sources/OmarchyThemesMac/      DesktopBackend for macOS (NSWorkspace + ImageIO)
    Tests/                         Swift Testing, platform-neutral + backend against a fake
  tools/generate-app-icon.swift    renders the asset-catalog icon set from design/AppIcon.svg
fixtures/                          test fixtures shared by the Windows and Swift tests
docs/windows-to-macos.md           Windows changes still to bring to the macOS app
```

## Building on Windows

Requirements: Windows 10 19041+ and the **.NET 10 SDK**. Nothing else. The Windows App SDK and the
Windows SDK build tools come from NuGet, and the app runs unpackaged + self-contained, so no MSIX
install or Windows App Runtime installer is needed. Visual Studio 2022/2026 with the *WinUI
application development* workload is optional (XAML designer and Hot Reload).

```bash
cd windows
dotnet build OmarchyThemes.sln
dotnet run --project src/OmarchyThemes.App
dotnet test
```

The app project defaults to the host architecture; pass `-p:Platform=ARM64` (or `x64`) to
cross-build. Solution and project builds share one output folder
(`src/OmarchyThemes.App/bin/Debug/net10.0-windows10.0.26100.0/win-x64/`), so `dotnet run` never
launches a stale build.

The same test switches as on macOS:

| Variable | Effect |
|---|---|
| `OMARCHY_THEMES_DRY_RUN=1` | Swaps in a backend that reports Windows' capabilities but changes nothing, so Apply and Restore can be exercised end to end without touching the desktop. The title bar shows a "Dry run" badge. |
| `OMARCHY_THEMES_DATA_DIR=<path>` | Uses another data folder instead of `%LOCALAPPDATA%\OmarchyThemes` (fresh first launch, no risk to real downloads or the saved original desktop). |

Use both whenever the UI is driven by a script:

```powershell
$env:OMARCHY_THEMES_DRY_RUN = '1'; $env:OMARCHY_THEMES_DATA_DIR = "$env:TEMP\omatheme-test"
dotnet run --project src/OmarchyThemes.App
```

Errors and notable events (catalog, default themes, downloads, apply, restore) are logged to
`logs\omarchy-themes-yyyyMMdd.log` in the data folder (kept for a week) and to the debugger output.

## Building on macOS

Requirements: macOS 14+, Xcode 16 or later (Swift 6), and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
(`brew install xcodegen`). SwiftSoup (HTML parsing) is the only package dependency and is fetched by SwiftPM.

```bash
cd macos
xcodegen                              # generates OmarchyThemes.xcodeproj
open OmarchyThemes.xcodeproj          # or build from the command line:
xcodebuild -scheme OmarchyThemes -derivedDataPath build/DerivedData build
xcodebuild -scheme OmarchyThemes -derivedDataPath build/DerivedData test

cd OmarchyThemesKit && swift test     # the package alone, no Xcode project needed
```

The app isn't sandboxed (it has to copy your current wallpaper from wherever it lives before the
first Apply) and is built with the hardened runtime for Developer ID distribution. Debug builds are
signed ad hoc.

Two environment variables help when testing the app:

| Variable | Effect |
|---|---|
| `OMARCHY_THEMES_DRY_RUN=1` | Swaps in a backend that changes nothing, so Apply and Restore can be exercised without touching the desktop. The window shows a "Dry run" badge. |
| `OMARCHY_THEMES_DATA_DIR=<path>` | Uses another data folder instead of `~/Library/Application Support/OmarchyThemes` (e.g. a fresh first launch). |

`GITHUB_TOKEN` (or `OMARCHY_THEMES_GITHUB_TOKEN`) raises the GitHub rate limit, as on Windows. Apps
started from the Finder don't inherit your shell's environment, so set it with `launchctl setenv` or
run the app binary from a terminal.

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
- **App** holds an `@Observable` `AppModel` (services, catalog state, downloads that carry on
  when you navigate away) and the views.

Local data lives in `~/Library/Application Support/OmarchyThemes`, with the same layout as on
Windows (`cache/`, `themes/<slug>/`, `settings.json`, `original-desktop.json`,
`original-desktop/`). Screenshot and wallpaper thumbnails are cached in `cache/images/`.

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
  theme share a single GitHub request (`ThemeDetailsService`).
- **Platform.Windows** implements `IDesktopBackend` (`WindowsDesktopBackend`). Each OS touchpoint
  sits behind a small interface (`IRegistryAccess`, `IWallpaperApi`, `ISettingsBroadcaster`,
  `IImageConverter`) so the backend is tested without changing the machine it runs on.
- **App** is the WinUI 3 UI (MVVM with CommunityToolkit.Mvvm, DI via
  Microsoft.Extensions.DependencyInjection).

Local data lives in `%LOCALAPPDATA%\OmarchyThemes`:

```
cache/                  HTTP cache (catalog page, GitHub trees, palette files) + ETags
themes/<slug>/          theme.json manifest, wallpapers/, screenshot
settings.json           welcome seen, one-click apply defaults, last applied theme
original-desktop.json   snapshot taken before the first Apply
original-desktop/       private copies of the original wallpapers, used by Restore
logs/                   daily log files, kept for a week
```

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
- If the snapshot can't be taken, nothing is applied.

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

## Tests

`dotnet test` in `windows/` runs both suites. No OS state is touched and nothing hits the network.

**Core (`OmarchyThemes.Core.Tests`)**

| Area | Covered |
|---|---|
| Catalog parser | live `li > a > img + span` markup, `<figure>` layout, relative screenshots, skipping nav/non-GitHub/duplicate cards, unique slugs |
| Default themes | folders under `themes/` only, Omarchy naming, `preview.png` screenshots, listed first, community-only fallback when GitHub fails, offline from cache, one shared tree call for listing and resolving |
| Repo links | `.git`, trailing slashes, `/tree/<ref>/<subdir>`, rejecting `/compare`, `/issues`, non-GitHub hosts, path traversal |
| Palettes | both `colors.toml` shapes, `alacritty.toml`, accent/mode fallbacks, lenient parsing of invalid TOML |
| HTTP cache / GitHub | ETag 304 revalidation, max-age, stale-when-offline, rate-limit reset time, 404, token sent only to the API |
| Theme resolution | palette priority, `light.mode`, wallpaper discovery and natural ordering, repo sub-folders |
| Store | atomic install (no partial theme on failure/cancel), reinstall, remove, settings round-trip |
| Downloads | starting again joins the running download (each file fetched once), progress in bytes across files ending at 100%, cancel leaves nothing behind, failures reported not thrown; concurrent lookups share one request, and one caller giving up doesn't cancel the shared lookup |
| `ThemeApplier` (fake backend) | snapshot-before-first-apply, abort if snapshot fails, per-step user/capability/data gating, partial failure, restore, the theme background passed as the wallpaper fill color |
| Accent math | ABGR/ARGB packing, 7-shade palette, `AccentPalette` bytes, normalizing unusable accents |
| Apply summaries | success, partial failure, total failure, snapshot failure and all-skipped messages |
| Terminal colors | named `colors.toml` mapped like Omarchy's own terminal template (black = background, white = foreground, bright black = `muted`, bright white and cursor = `bright_foreground`), `color0..15` and alacritty used as-is, fallbacks for missing colors |

**Windows backend (`OmarchyThemes.Platform.Windows.Tests`)**, run against in-memory registry,
wallpaper and broadcast fakes:

| Area | Covered |
|---|---|
| Light/dark | both `Personalize` values + `ImmersiveColorSet` broadcast |
| Accent | exact DWM/Explorer values and byte layout, `AutoColorization=0`, near-black accents lifted, nothing written outside `TrackedValues` |
| Wallpaper | conversion before setting, fit and exact fill color passed through, `COLORREF` packing (0x00BBGGRR) |
| Snapshot/restore | registry values restored and previously-absent values deleted, per-monitor wallpapers, fit and fill color, falling back to the private copy when the original file is gone, JSON round-trip |
| WIC conversion | real Windows Imaging Component on temp files: PNG output, reuse, actionable error for unreadable images |
| Windows Terminal fragments | a complete scheme (name + all 16 colors, which Terminal requires), UTF-8 without a BOM, schemes only (no profile changes), replace/remove/cleanup, unusual names, slug path safety |

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
apply summaries). It also covers the strict TOML reader (inline tables, quoted and dotted keys,
escapes, multi-line strings, arrays) and checks that concurrent applies never interleave.

**macOS backend (`OmarchyThemesMacTests`)**, against an in-memory `WallpaperAPI`:

| Area | Covered |
|---|---|
| Wallpaper | every screen set, fit → `imageScaling`/`allowClipping`, fill color, WebP converted before setting |
| Snapshot/restore | per-screen picture and options, one private copy per file, system pictures not copied, falling back to the copy when the original is gone, a display connected later, failure only when every screen fails |
| Capabilities | wallpaper only; through `ThemeApplier`, light/dark and accent report "not supported" |
| ImageIO conversion | real ImageIO on temp files: WebP decoding, BMP → PNG, reuse, actionable error for unreadable images |

`OMATHEME_LIVE=1 swift test --filter LiveChecks` runs opt-in, read-only checks against the
live site, a few theme repos and this Mac's current desktop. During development they parsed all 146
themes, resolved Aetheria and Vulkanite (WebP wallpapers, decoded and converted to a 3840×2160 PNG),
and checked the downloader's byte-level progress.
