# Omarchy Themes

A native desktop companion for the [Omarchy](https://omarchy.org) community theme gallery
([omarchy.org/themes](https://omarchy.org/themes/)). Omarchy themes are built for Linux/Hyprland; this
app doesn't run their configs. It reads each theme's **wallpaper and color palette** and re-applies
the equivalent look using the host OS's own theming APIs.

Omarchy is by DHH and contributors: https://github.com/basecamp/omarchy. This project is not
affiliated with Omarchy or 37signals; every theme belongs to its author.

| Platform | Stack | Status |
|---|---|---|
| Windows 10 (19041+) / 11 | WinUI 3 · Windows App SDK 2.5 · .NET 10 | Scaffolded (`windows/`) |
| macOS | SwiftUI (separate native app) | Planned (`macos/`) |

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
  tests/OmarchyThemes.Core.Tests/  xUnit
  tools/Generate-AppIcon.ps1       renders Assets/AppIcon.png + .ico
macos/                             (later)
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
cross-build.

## Architecture (Windows)

> **Status:** only the scaffold exists so far: solution, window shell, welcome/about/settings
> pages, icon, and test project. The sections below describe the design being implemented.

- **Core** holds all logic that doesn't touch the OS, so it's unit-tested without Windows:
  catalog parsing, GitHub repo resolution with an ETag cache, palette parsers, the on-disk theme
  store, and `ThemeApplier`, which drives an `IDesktopBackend` abstraction.
- **Platform.Windows** implements `IDesktopBackend` with real Windows APIs.
- **App** is the WinUI 3 UI (MVVM with CommunityToolkit.Mvvm, DI via
  Microsoft.Extensions.DependencyInjection). Local data lives in `%LOCALAPPDATA%\OmarchyThemes`.

### Catalog parsing

The live omarchy.org/themes page is a minified Astro page where each theme is
`ul > li > a[href="https://github.com/…"] > img[src="/assets/themes/<slug>.webp"] + span(name)`
(there are no `<figure>` elements, despite older descriptions). The parser selects those anchors
and resolves screenshot URLs against the page URL. A `<figure>` layout is accepted as a fallback,
and entries without a GitHub link are skipped. The parsed catalog is cached as JSON and refreshed on
demand.

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

| Aspect | Mechanism |
|---|---|
| Wallpaper | `IDesktopWallpaper::SetWallpaper` on every monitor + `SetPosition(DWPOS_FILL)`; `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` fallback |
| Light / dark | `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize` `AppsUseLightTheme` / `SystemUsesLightTheme`, then broadcast `WM_SETTINGCHANGE("ImmersiveColorSet")` |
| Accent color | `HKCU\Software\Microsoft\Windows\DWM` `AccentColor`/`ColorizationColor`, `…\Explorer\Accent` `AccentPalette`/`AccentColorMenu`, `AutoColorization=0`, then broadcast. Windows has no public API for this, so it's best effort and some shell surfaces only refresh after sign-in. |

The app never changes anything on launch. Before the first Apply it snapshots the current
wallpaper, mode and accent so **Restore my original desktop** can undo it. Each aspect has its
own checkbox in the Apply dialog.

### Adding a new platform theming API

1. Add a capability to `DesktopCapabilities` in Core, along with the matching method on
   `IDesktopBackend`.
2. Teach `ThemeApplier` when to call it (it gates every step on the backend's capabilities and on
   the user's Apply options), and extend `DesktopSnapshot` if the setting should be restorable.
3. Implement it in the platform backend (`Platform.Windows`, or the macOS app's equivalent) and
   report the capability.
4. Add a `ThemeApplier` test using the fake backend.

## Tests

`dotnet test` in `windows/` runs the Core suite: catalog parser fixtures, repo URL parsing,
palette parsers, the GitHub client against a fake HTTP handler, accent math, and `ThemeApplier`
against a fake `IDesktopBackend`. No OS state is touched.
