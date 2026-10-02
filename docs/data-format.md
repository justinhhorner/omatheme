# Data format

Both apps and the command-line tool (`cli/`) read and write the **same** files in the data folder.
This document is the contract. `fixtures/data/` holds a sample of each file, and every client's test suite
checks that it writes exactly that (as JSON: same keys, types and values) and reads it back, so the
clients can't drift apart.

Data folder: `%LOCALAPPDATA%\OmarchyThemes` (Windows), `~/Library/Application Support/OmarchyThemes`
(macOS), `$XDG_DATA_HOME/omarchy-themes` (the CLI on other OSes), or `OMARCHY_THEMES_DATA_DIR` when set.

```
settings.json           app settings
themes/<slug>/          one downloaded theme: theme.json, wallpapers/, screenshot.<ext>
original-desktop.json   snapshot taken before the first Apply
original-desktop/       private copies of the original wallpapers
cache/<xx>/<sha256>.body + .json   HTTP cache (xx = first two hex digits of the SHA-256 of the URL)
```

Platform-specific files (logs, `cache/images/` on macOS, terminal exports) aren't part of the contract.

## Rules for every JSON file

- UTF-8, no byte-order mark. Indentation, key order and whitespace don't matter; readers parse JSON, never
  compare text.
- Keys are **camelCase** (`repoUrl`, `etag`, `brightForeground`).
- **Missing and `null` mean the same thing.** Writers **omit** absent optional values instead of writing
  `null`. Readers use the documented default for anything missing and ignore unknown keys.
- **Dates:** ISO 8601 in UTC with milliseconds and `Z`, e.g. `"2026-09-27T09:00:00.123Z"`. Readers
  also accept other fraction lengths, no fraction, and numeric offsets (`+00:00`).
- **Colors:** `"#rrggbb"`, lowercase.
- **Enums:** camelCase strings: `"dark"`, `"light"`; `"colorsToml"`, `"alacritty"`;
  `"fill"`, `"fit"`, `"stretch"`, `"center"`, `"tile"`, `"span"`.
- Write atomically (temporary file + rename).
- **Don't persist values derived from others** (a palette's effective mode, absolute paths).

## `settings.json`

| Key | Type | Default | Meaning |
|---|---|---|---|
| `welcomeSeen` | bool | `false` | The welcome screen was dismissed. |
| `applyDefaults` | object | all `true`, `"fill"` | One-click Apply choices: `wallpaper`, `appearanceMode`, `accentColor` (bools, default `true`) and `fit` (enum, default `"fill"`). |
| `lastAppliedSlug` | string | absent | The theme on the desktop (last applied). Cleared by Restore. |
| `lastAppliedWallpaper` | string | absent | Which of its wallpapers is on the desktop; only set when the wallpaper step applied. |
| `terminalApp` | string | absent (platform default) | Terminal the theme page sends colors to: `"iterm2"`, `"ghostty"`, `"terminal"` (macOS). The Windows app only has Windows Terminal and leaves it absent but keeps it. |

## `themes/<slug>/theme.json`

| Key | Type | Meaning |
|---|---|---|
| `slug` | string | Folder name. Community slugs are `[a-z0-9_-]`; default themes are `omarchy.<folder>`. |
| `name` | string | Display name. |
| `repoUrl` | string | GitHub URL of the theme (`…/tree/<ref>/<path>` for a sub-folder). |
| `palette` | object | Absent if no palette could be read. See below. |
| `mode` | enum | `"dark"` or `"light"`: the theme's effective mode. |
| `wallpapers` | string[] | File names in `wallpapers/`, in display order. |
| `screenshotFile` | string | File name of the screenshot in the theme folder; absent if none. |
| `downloadedAt` | date | When it was downloaded. |

`palette`: `background`, `foreground`, `accent` (colors, required); `cursor`, `selection`, `muted`,
`brightForeground` (colors, optional); `declaredMode` (enum, optional: the mode the theme states itself);
`swatches` (array of `{ "name": string, "color": color }`, in display order); `source` (enum).

## `original-desktop.json`

```json
{ "takenAt": "2026-09-27T09:00:00.250Z", "values": { "<key>": "<string>" } }
```

The envelope is shared; the `values` keys are the platform backend's own (e.g. `monitor:…` and `reg:…` on
Windows, `screens` and `screen.url.…` on macOS), since what a desktop consists of differs by OS.

## `cache/**/<sha256>.json`

| Key | Type | Meaning |
|---|---|---|
| `url` | string | The absolute URL (the SHA-256 of this string names the files). |
| `fetchedAt` | date | When the response was fetched or last revalidated. |
| `etag` | string | The `ETag` header, verbatim (quotes included); absent if none. |
| `lastModified` | string | The `Last-Modified` header, verbatim (an HTTP date); absent if none. |

The body is stored next to it as `<sha256>.body`.

## Older files

Before this format existed, the apps wrote slightly different JSON. Every client still reads it:

| Written by | Differences |
|---|---|
| Windows (v0.1) | `null` for absent values; a derived `mode` inside `palette` and an absolute `screenshotPath` in `theme.json` (both ignored); dates like `…09:00:00.1234567+00:00`; cache `eTag` and `lastModified` as an ISO date. |
| macOS (v0.1) | `repoURL` instead of `repoUrl`; dates without a time zone (`…09:00:00.123`, which meant UTC). |

`fixtures/data/legacy/` has an example of each, and every test suite reads them.
