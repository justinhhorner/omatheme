# CLAUDE.md: Omarchy Themes

Context for Claude Code sessions on this repo. The Windows app was built in one session on Windows; this file
carries its decisions and lessons so work can continue on the macOS app on a Mac. Read `README.md` for the full
architecture; this file holds what the README doesn't: decisions, rules and gotchas.

## What this is

A native desktop companion for the Omarchy community theme gallery (https://omarchy.org/themes/). It lists the
catalog, previews a theme (screenshot, palette, wallpapers), downloads it (wallpapers + palette from the theme's
GitHub repo) and applies the equivalent look on the host OS (wallpaper, plus light/dark and accent where the OS
allows). It never runs Omarchy's Linux/Hyprland configs. Credit Omarchy (https://omarchy.org,
https://github.com/basecamp/omarchy); not affiliated.

## Decisions already made (don't reopen)

- **Separate native apps per platform, no cross-platform UI.** A Tauri plan was rejected in favour of WinUI 3 on
  Windows and **SwiftUI on macOS**. Each app follows its OS's conventions; don't reskin one look for both.
- Windows app: `windows/`, WinUI 3 + Windows App SDK 2.5 + .NET 10, unpackaged and self-contained. **Done (v0.1).**
- macOS app: `macos/`, **next**. See "macOS plan" below.
- One monorepo. `design/AppIcon.svg` is the shared icon (original artwork, not the Omarchy logo).
- GitHub: `justinhhorner/omatheme` (private). Default branch `main`.

## Rules for working here

- **Never apply a theme, restore the original desktop, or otherwise change the real desktop, wallpaper,
  appearance or accent of the machine you're running on without asking the user first.** Test apply logic
  against fake backends. When checking the UI, only open dialogs and cancel them; never script "Apply", "Set as
  desktop theme" or "Restore". (On Windows a UI script once nearly clicked "Set as desktop theme"; it didn't
  fire, but that's the failure mode to avoid.)
- Read-only calls against the real OS (e.g. reading the current wallpaper) are fine and useful.
- The GitHub API is unauthenticated by default (60 req/hour). Live checks should resolve a handful of themes,
  not the whole catalog.
- Commit messages end with the attribution line the session provides (`Co-Authored-By: Claude …`).
- Commit and push only when the user asks.

## Data source facts (verified live, Sep 2026)

- **omarchy.org/themes** is a minified Astro page. There are **no `<figure>` elements** (the original brief was
  wrong). Each theme card is
  `ul > li > a[href="https://github.com/owner/repo"] > img[src="/assets/themes/<slug>.webp" alt="<Name> theme screenshot"] + span(Name)`.
  There were 146 themes. Other GitHub links on the page (nav, "Share your theme" → `omacom/omarchy-site/compare`)
  have no `<img>` and must be skipped. The parser also accepts a `<figure>` layout as a fallback.
- **Theme repos** vary:
  - Palette: `colors.toml` comes in two shapes:
    - named keys: `mode`, `accent`, `background`, `foreground`, `red`, `bright_red`, … (Omarchy's newer format);
    - terminal-style: `accent`, `cursor`, `foreground`, `background`, `selection_*`, `color0..color15`.
  - Fallback palette: `alacritty.toml` (`[colors.primary]`, `[colors.normal]`, `[colors.bright]`; values may be
    `#rrggbb` or `0xrrggbb`, and some are non-colors like `CellForeground`).
  - Hand-edited TOML can be invalid, so fall back to a lenient line scanner.
  - Wallpapers live in `backgrounds/` (sometimes `wallpapers/`, or a root `background.*`). **Many are `.webp`**
    (e.g. Vulkanite). Use natural sort (`2.png` < `10.png`).
  - Light themes: `mode = "light"` or a `light.mode` file, else infer from background luminance.
  - Default branches vary (e.g. `omarchy-aetheria-theme`). Use `HEAD`, which works for both the API
    (`git/trees/HEAD?recursive=1`) and `raw.githubusercontent.com/<o>/<r>/HEAD/<path>`.
- **GitHub strategy**: one API call per theme (recursive tree), only when the user opens a theme; file bytes
  from `raw.githubusercontent.com` (not API-metered). Cache with ETag; a 304 doesn't count against the rate limit.
  Send a User-Agent (the API rejects requests without one). On 403 with `x-ratelimit-remaining: 0`, or on 429,
  show the reset time from `x-ratelimit-reset`. Optional token from `GITHUB_TOKEN`.
- Offline: serve the cached catalog/tree marked stale; downloaded themes need no network at all.

## Windows implementation: where to look when porting

Port behaviour, not code. The equivalents on Windows:

| Concern | Windows file |
|---|---|
| Catalog parser | `windows/src/OmarchyThemes.Core/Catalog/CatalogParser.cs` |
| HTTP cache (ETag, max-age, stale-on-error) | `…/Core/Net/HttpCache.cs` |
| GitHub client + repo URL parsing | `…/Core/GitHub/` |
| Palette parsers | `…/Core/Palettes/` |
| Theme resolution (palette priority, wallpapers, light.mode) | `…/Core/Themes/ThemeResolver.cs` |
| Staged, atomic theme download | `…/Core/Storage/ThemeStore.cs` |
| Apply orchestration: snapshot-before-first-apply, per-step gating, partial failure | `…/Core/Theming/ThemeApplier.cs` |
| User-facing apply messages | `…/Core/Theming/ApplySummary.cs` |
| OS backend | `windows/src/OmarchyThemes.Platform.Windows/WindowsDesktopBackend.cs` |

**Test fixtures** in `windows/tests/OmarchyThemes.Core.Tests/Fixtures/` (catalog HTML in both layouts, both
`colors.toml` shapes, alacritty) are hand-written to mirror real data. Reuse them for the Swift tests; consider
moving them to a shared `fixtures/` folder at the root.

Key behaviours to keep on macOS:

- **Nothing** changes on launch. The welcome screen shows on first launch only (and from Settings).
- Before the first Apply, snapshot the user's desktop (each screen's wallpaper URL + options, and a private copy
  of the image). If the snapshot fails, apply nothing. Offer **Restore my original desktop** in Settings.
- Apply runs each aspect independently, and the result reports each one: applied, skipped by user, not
  supported, no data, or failed.
- The UI has a gallery with search, a "downloaded only" filter and refresh, plus loading/error/empty/stale
  states; a detail page with the palette, a wallpaper picker, download progress and cancel; a Downloaded view
  with one-click apply using saved defaults; and Settings.

## macOS plan (proposed; confirm with the user before scaffolding)

- **Stack:** SwiftUI app, macOS 14+ (Observation, `NavigationSplitView`), Swift 6.
- **Structure:**
  - `macos/OmarchyThemesKit`: a Swift package with the Core equivalents, no AppKit, tested with Swift Testing
    via `swift test`.
  - A thin app target that uses the package.
  - The Xcode project is generated with XcodeGen (`project.yml`) so it's reproducible and diff-friendly, and
    can be built from the CLI with `xcodebuild`.
- **Native look:** `NavigationSplitView` sidebar (Gallery, Downloaded) with the system sidebar material,
  unified toolbar with search (`.searchable`) and refresh, standard traffic lights, `Settings {}` scene
  (⌘,), About panel with credits and links, and a menu command to show the welcome screen again.
- **Wallpaper:** `NSWorkspace.shared.setDesktopImageURL(_:for:options:)` for every `NSScreen`; options map
  fill/fit/stretch/center via `NSWorkspace.DesktopImageOptionKey` (`imageScaling`, `allowClipping`,
  `fillColor`). Snapshot with `desktopImageURL(for:)` and `desktopImageOptions(for:)`. macOS decodes WebP
  natively (macOS 11+), so no conversion is needed; verify.
- **Appearance:** read and observe only (`NSApp.effectiveAppearance`, KVO, or the
  `AppleInterfaceThemeChangedNotification` distributed notification). macOS has **no public API to set system
  dark mode or the accent color**, so report those capabilities as unsupported (the only mechanisms are
  AppleScript/System Events or private defaults, which the brief rules out). Apply dialog shows them disabled
  with a reason.
- **Data:** `~/Library/Application Support/OmarchyThemes/` (same layout as Windows: `cache/`, `themes/<slug>/`,
  `settings.json`, `original-desktop.json`).
- **Tests:** parser, palette, resolver (with a URLProtocol-based fake), store, and `ThemeApplier` against a fake
  backend, mirroring the Windows suites.
- **App Sandbox:** decide early. The sandbox allows `setDesktopImageURL` for user-selected or app-container
  files, but check behaviour for files in the container before committing to the sandbox.

## Verifying UI on each platform

- **Windows** (what was done): launch the built exe, drive it with UI Automation from PowerShell
  (`System.Windows.Automation`: Invoke/SelectionItem/Value patterns, no focus stealing), and capture with
  `PrintWindow(PW_RENDERFULLCONTENT)`. Give buttons whose content isn't plain text an
  `AutomationProperties.Name`; this was a real accessibility bug found that way.
- **macOS** (to set up): build with `xcodebuild`, launch the `.app`, and capture the window with
  `screencapture -l <windowID>` (needs Screen Recording permission for the terminal). Use XCUITest or the
  Accessibility API for scripted navigation. The same rule applies: open dialogs and cancel, never apply.

## Building the Windows app (for reference)

```bash
cd windows
dotnet build OmarchyThemes.sln
dotnet test          # Core (117) + Windows backend (22) tests
dotnet run --project src/OmarchyThemes.App
```
