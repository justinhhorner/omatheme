# Prompt: a Rust CLI for Omarchy Themes

Paste everything below the line into a new Claude Code session opened at the repo root.

---

Build **`omatheme`**, a command-line version of the Omarchy Themes apps, in **Rust**, in a new `cli/`
folder of this monorepo. It does what the Windows (WinUI) and macOS (SwiftUI) apps do, from a terminal:
list and search the Omarchy theme catalog, show a theme's palette and wallpapers, download themes, apply
one to the desktop, switch wallpapers, restore the original desktop, and send a theme's colors to a
terminal app.

## Read first

1. `CLAUDE.md`: project rules, decisions, data-source facts and gotchas. Its rules apply to you.
2. `docs/ARCHITECTURE.md`: how both apps work, their on-disk layout, apply mechanisms per OS, and tests.
3. The Core/Kit code you're porting. Port behaviour, not code, and treat the source as the spec wherever
   this prompt is less precise:
   - `windows/src/OmarchyThemes.Core/` (C#) and `macos/OmarchyThemesKit/Sources/OmarchyThemesKit/` (Swift):
     catalog parser, default themes, HTTP cache, GitHub client, palette parsers, `TerminalColors`,
     resolver, theme store, `ThemeApplier`, `ApplySummary`, `AppSettings.AfterApply`.
   - OS backends: `windows/src/OmarchyThemes.Platform.Windows/` (wallpaper via `IDesktopWallpaper`,
     light/dark and accent via HKCU registry + `WM_SETTINGCHANGE`, snapshot/restore, Windows Terminal
     fragments) and `macos/OmarchyThemesKit/Sources/OmarchyThemesMac/` (NSWorkspace wallpaper per screen,
     snapshot/restore, ImageIO conversion, terminal exporters).
4. `fixtures/`: test fixtures shared by both apps. Use them for the Rust tests too.

**Before scaffolding, confirm the plan with the user** (crate layout, dependencies, command set, and the
data-sharing decision below), as the macOS app did. Then build it.

## Rules (from CLAUDE.md, restated because they matter most)

- **Never apply a theme, restore the desktop, or otherwise change the real desktop, wallpaper, appearance,
  accent or terminal settings of the machine you're on without asking the user first.** Test apply logic
  against fake backends. Read-only calls against the real OS are fine.
- Honor the apps' test switches: `OMARCHY_THEMES_DRY_RUN=1` (a backend that reports the platform's
  capabilities but changes nothing; terminal exports go into the data folder) and
  `OMARCHY_THEMES_DATA_DIR=<path>` (another data folder). Use both whenever you run the CLI's apply,
  wallpaper or restore commands during development. Also offer a `--dry-run` flag that does the same.
- The GitHub API is unauthenticated by default (60 requests/hour). Live checks resolve a handful of
  themes, never the whole catalog. `GITHUB_TOKEN` raises the limit.
- Commit and push only when the user asks; commit messages end with the session's attribution line.

## Scope

- **Platforms:** macOS 14+ and Windows 10 (19041+)/11 can apply themes. Everything else (list, show,
  download, terminal export) is platform-neutral. On other OSes (e.g. Linux, where Omarchy applies themes
  itself), `apply`, `wallpaper` and `restore` say they're not supported there and exit non-zero; the rest
  works.
- **Capabilities** as in the apps: Windows applies wallpaper, light/dark and accent; macOS applies the
  wallpaper only (no public API for appearance or accent; say so, and point to System Settings ›
  Appearance). Apply runs each aspect independently and reports each one: applied, skipped by user, not
  supported, no data, or failed, with the same summary wording as `ApplySummary`.
- **Safety** as in the apps: nothing changes unless a command asks for it. Before the first apply,
  snapshot the desktop (per display/monitor wallpaper, options/fit, a private copy of each picture except
  system ones, and on Windows every registry value the backend may touch, including "was absent"). If the
  snapshot fails, apply nothing. `restore` puts it back and forgets it.

## Data: share the GUI apps' folder

Use the same data folder and file formats as the GUI app on the same OS, so a theme downloaded in the CLI
shows up in the app and vice versa, and either can restore the original desktop:

- `%LOCALAPPDATA%\OmarchyThemes` on Windows, `~/Library/Application Support/OmarchyThemes` on macOS
  (a platform-appropriate folder elsewhere, e.g. `$XDG_DATA_HOME/omarchy-themes`).
- **Follow `docs/data-format.md` exactly.** It's the contract both apps implement (layout, every JSON
  file's keys and types, dates, colors, enums, and the older formats you must still read). Test against
  `fixtures/data/` like the apps' `DataFormatTests` do: write exactly the samples, read them and every file
  in `legacy/`. Preserve unknown keys in `settings.json` when you rewrite it.
- The HTTP cache (`cache/<xx>/<sha256>.body` + `.json`) is part of the format too, so the CLI can share
  it with the app on the same machine.
- `original-desktop.json`'s `values` keys are backend-specific (`monitor:…` and `reg:…` on Windows,
  `screens` and `screen.url.…` on macOS). Read and write the same keys as that OS's app backend, so a
  snapshot taken by one can be restored by the other.
- After an apply, update `lastAppliedSlug` / `lastAppliedWallpaper` with the same rule as
  `AppSettings.AfterApply` (the wallpaper is only recorded when the wallpaper step applied), so the app's
  "Current theme" section reflects what the CLI did.

## Behaviour to port

- **Catalog:** community themes parsed from https://omarchy.org/themes/ (cards are
  `li > a[href=github repo] > img + span`, a `<figure>` layout as fallback; skip links without an image,
  non-GitHub links and duplicates; unique slugs from the screenshot file name). Plus **Omarchy's default
  themes**: the folders under `themes/` in `omacom/omarchy` at `HEAD`, from one cached recursive-tree API
  call that also serves each default theme's details. Slugs are `omarchy.<folder>`, names follow
  `omarchy-theme-list` ("retro-82" → "Retro 82"), and the screenshot is `preview.png`. Default themes are
  listed first; a GitHub failure drops them, not the catalog. Cache the raw page and tree, not parsed
  results.
- **HTTP:** a disk cache with ETag/Last-Modified revalidation and max-age (a 304 doesn't count against the
  rate limit), stale copies served when offline or on server errors, a User-Agent on every request, the
  token sent only to `api.github.com`, and a clear message with the reset time on rate limiting (403 with
  `x-ratelimit-remaining: 0`, or 429).
- **Resolution:** one API call per theme (`git/trees/HEAD?recursive=1`, honoring `/tree/<ref>/<subdir>`
  links), files from `raw.githubusercontent.com`. Palette priority: `colors.toml` (both shapes: named keys
  incl. `mode`, `muted`, `bright_foreground`; or `color0..15`), falling back to `alacritty.toml`. Parse
  strictly, falling back to the lenient line scanner for hand-edited files. Wallpapers come from
  `backgrounds/`, then `wallpapers/`, then a root `background.*`, in natural sort order. Light/dark comes
  from `mode`, a `light.mode` file, or background luminance.
- **Wallpaper formats:** convert WebP/BMP to PNG before setting (as both apps do), caching the result
  next to the file.
- **Terminal colors:** the same `TerminalColors` mapping (Omarchy's terminal template; the same test cases
  as `TerminalColorsTests`, and round halves to even like .NET's `Math.Round`). Exporters behind a trait,
  one per terminal, listed in one registry so adding a terminal is one type plus one line: Windows Terminal
  (JSON fragment, see `WindowsTerminalSchemes.cs`), iTerm2 (Dynamic Profile), Ghostty (theme file) and
  Terminal.app (`.terminal` profile, removal manual). Use the same file names and scheme name
  ("<Theme> (Omarchy)") as the apps. Never edit a terminal's own settings. Default: Windows Terminal on
  Windows, iTerm2 on macOS, Ghostty elsewhere.

## Commands (proposed; refine with the user)

```
omatheme list [--search TEXT] [--downloaded] [--default | --community] [--json]
omatheme show <THEME> [--json]                  # palette, terminal colors, wallpapers, mode, repo
omatheme download <THEME>... [--force]          # per-file byte progress; Ctrl-C leaves nothing half-installed
omatheme apply <THEME> [--wallpaper N|NAME] [--fit fill|fit|stretch|center|tile|span]
                       [--no-wallpaper] [--no-mode] [--no-accent] [--yes]
omatheme current [--json]                       # the theme and wallpaper on the desktop, per settings.json
omatheme wallpaper <N|NAME|next|prev> [--yes]   # another wallpaper of the current theme, wallpaper only
omatheme restore [--yes]
omatheme remove <THEME>
omatheme terminal apps
omatheme terminal add|remove <THEME> [--app ID]
omatheme cache clear
omatheme paths                                  # data folder, cache, snapshot status
```

- `<THEME>` matches a slug, a case-insensitive name, or a unique prefix of either; ambiguous input lists
  the candidates. `apply` downloads the theme first if needed.
- Commands that change the desktop ask for confirmation on a TTY unless `--yes`. When stdin isn't a TTY,
  they refuse without `--yes`.
- Human-readable output by default, `--json` for scripts; progress and colors only on a TTY (respect
  `NO_COLOR`), progress on stderr. Exit codes: 0 success, 1 failure, 2 usage, 3 partial apply.
- Credit Omarchy in `--help`/`--version` (https://omarchy.org, https://github.com/basecamp/omarchy, which
  redirects to omacom/omarchy) and say it isn't affiliated with Omarchy or 37signals.

## Suggested stack (confirm with the user)

A single crate with a library (all logic, testable) and a thin binary: `clap` (derive) for arguments;
blocking HTTP (`ureq` or `reqwest` with rustls) behind a small transport trait so tests use a fake;
`scraper` for HTML; `toml` plus the lenient scanner; `serde`/`serde_json`; `directories` for paths;
`indicatif` for progress; `image` for WebP → PNG; `sha2` for cache keys. OS code behind `cfg`:
`objc2`/`objc2-app-kit` for NSWorkspace on macOS (main thread), and the `windows` crate for
`IDesktopWallpaper`, the registry and `SendMessageTimeout` on Windows. Each backend's OS calls sit behind a
small trait (like `WallpaperAPI` on macOS, and the registry/wallpaper/broadcast interfaces on Windows), so
the backends are tested without changing the machine.

## Tests

- `cargo test` runs without network or OS changes: parsers (catalog, TOML, palettes, repo links), HTTP
  cache (304, max-age, stale-offline, rate limit), resolver, default themes, store (atomic install,
  cancel/failure leaves nothing), `ThemeApplier` against a fake backend (snapshot first, abort if it fails,
  per-step gating, partial failure, restore), `AfterApply`, `TerminalColors`, exporters against temp folders,
  JSON compatibility with both GUI formats, and theme-argument matching. Mirror the existing suites' cases.
- Opt-in live checks (`OMATHEME_LIVE=1 cargo test -- --ignored`): parse omarchy.org, list default themes,
  resolve Tokyo Night and Vulkanite (WebP), download one wallpaper with byte progress, and read (never set)
  the current wallpaper.
- Check the CLI end to end with `OMARCHY_THEMES_DRY_RUN=1` and `OMARCHY_THEMES_DATA_DIR` set. Before and
  after, read the real desktop state (read-only) to prove nothing changed.

## When done

- `README.md` (keep it minimal): add the CLI to the platform table and a short build/usage section.
- `docs/ARCHITECTURE.md`: a CLI section (layout, commands, data sharing, backends, tests).
- `CLAUDE.md`: decisions and gotchas you found. If the CLI gains something the apps lack, or the reverse,
  list it in a `docs/<from>-to-<to>.md` as the project does.
