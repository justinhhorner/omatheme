# Windows → macOS: changes to bring across

Written on Windows (Sep 2026). It lists what the Windows app changed that the macOS app doesn't have yet, so a
session on a Mac can implement it (the Swift app can't be built or checked on Windows). Port behaviour, not
code, and keep macOS conventions (see CLAUDE.md).

The usual rule applies: never apply a theme or restore the desktop on the real machine without asking. Run UI
checks with `OMARCHY_THEMES_DRY_RUN=1` and `OMARCHY_THEMES_DATA_DIR=<temp>`.

When an item is done, delete it from this file (and this file once it's empty), and update `docs/ARCHITECTURE.md` (and CLAUDE.md where relevant; keep the README minimal).

---

## 1. No file name under wallpaper thumbnails

On the theme page, the wallpaper thumbnails no longer show the file name underneath; each thumbnail is just
the image (the user asked for this). The name is still available:

- as a hover tooltip on the thumbnail,
- as the thumbnail's accessibility label (screen readers still announce it),
- in the large preview's control bar.

Windows: `windows/src/OmarchyThemes.App/Views/ThemeDetailPage.xaml` (wallpaper `GridView` item template:
caption `TextBlock` removed, `ToolTipService.ToolTip` and `AutomationProperties.Name` kept).

macOS: `macos/App/Views/ThemeDetailView.swift`, `WallpaperThumbnail` (around line 370). Remove the
`Text(item.name)` caption and the `VStack` that wraps the image and caption, and add `.help(item.name)` so the
name shows on hover. The thumbnail `Button` in `wallpaperSection` already has `.accessibilityLabel(item.name)`,
so VoiceOver is unaffected. Check the selection outline and hover Preview button still sit on the image, then
build and run the dry-run app to confirm.

## 2. "Current theme" section above the gallery

A card above the gallery's sections highlights the theme on the desktop and lets the user switch between its
wallpapers in one click.

- **When:** shown when `settings.lastAppliedSlug` names a theme that's still downloaded; hidden while searching;
  gone after Restore (which clears the slug). It's local-only, so it shows even when the catalog can't load.
- **Content:** screenshot (or the first wallpaper if there's none), name, an "On your desktop" badge,
  "Dark theme · 8 wallpapers", a color strip, **View theme** (opens the theme page), and a horizontal row of
  the theme's wallpapers. The one on the desktop (`lastAppliedWallpaper`) has an accent ring and a check mark.
- **Clicking a wallpaper** applies it immediately with options *wallpaper only* (mode and accent off, since
  they're already the theme's), the saved fit, and the theme background as fill color. A spinner shows on
  that tile; on success the check mark moves (no banner); a warning/error shows in a banner in the card.
  Accessibility names: "Set <file> as the desktop wallpaper" / "<file>, current wallpaper".
- **Bug fix that goes with it:** `lastAppliedWallpaper` used to be set to the request's wallpaper even when the
  wallpaper step didn't run (unchecked in the Apply sheet). Now it's only updated when the wallpaper step
  applied; otherwise it keeps the previous value for the same theme, or clears it for a different theme.
  Windows: `AppSettings.AfterApply` in `windows/src/OmarchyThemes.Core/Storage/SettingsStore.cs`, with tests in
  `ThemeStoreTests` ("Applying_…"). Port to the Kit's settings/apply path with the same three tests.

Windows: `windows/src/OmarchyThemes.App/ViewModels/CurrentThemeViewModel.cs`, `GalleryViewModel`
(`CurrentTheme`, `ShowCurrentTheme`, `UpdateCurrentTheme`), and the section at the top of
`Views/GalleryPage.xaml`.

macOS: add the card at the top of `macos/App/Views/GalleryView.swift`'s grid, driven by `AppModel`
(the active slug/wallpaper it already tracks for the "Current" badges). Use a `ScrollView(.horizontal)` of
wallpaper thumbnails and `model.apply(theme, wallpaperFile:options:)` with wallpaper-only options.

## 3. Send a theme's terminal colors to a terminal app

Windows: **Add to Windows Terminal** on the theme page writes the theme's terminal colors as a Windows
Terminal color scheme ("<Theme> (Omarchy)") via Terminal's JSON fragments, one file per theme; **Remove**
deletes it. The user's own terminal settings are never edited.

These parts port directly:

- **Palette:** named `colors.toml` now also keeps `muted` and `bright_foreground` (`Palette.Muted`,
  `Palette.BrightForeground`). Windows: `windows/src/OmarchyThemes.Core/Palettes/Palette.cs`, `PaletteParsers.cs`.
- **The mapping** (`TerminalColors.From` in `windows/src/OmarchyThemes.Core/Palettes/TerminalColors.cs`, tests
  in `TerminalColorsTests`): follows Omarchy's own template `default/themed/alacritty.toml.tpl` in
  `omacom/omarchy`: black = background, white = foreground, bright black = `muted`, bright white and cursor =
  `bright_foreground`; `color0..15` / alacritty colors used as-is; missing bright colors fall back to normal.
  Port it to the Kit with the same four tests.
- **Old downloads:** themes downloaded before `muted`/`bright_foreground` were read saved a palette without
  them; the Windows view model looks the theme up again (usually cached) before exporting, falling back to
  the saved palette offline (`ThemeDetailViewModel.Terminal.cs`, `PaletteForTerminalAsync`).

The target differs on macOS. Candidates that, like Windows Terminal's fragments, add a scheme without editing the
user's config (decide on the Mac, and check what's installed):

- **Ghostty**: a theme file in `~/.config/ghostty/themes/<name>` (`background = …`, `foreground = …`,
  `palette = 0=#…` … `palette = 15=#…`, `cursor-color`, `selection-background`); the user picks it with
  `theme = <name>`. Omarchy itself ships Ghostty configs, so this is the closest match.
- **iTerm2**: a Dynamic Profile JSON in `~/Library/Application Support/iTerm2/DynamicProfiles/` (colors as
  RGB component dictionaries); appears as a new profile.
- Terminal.app needs a `.terminal` profile with archived `NSColor`s: more work, lower value.

Put it on the theme page with add/remove and the same "restart the terminal to see it" guidance.

## 4. A macOS screenshot in the README

The README now opens with the app icon, centered, and a screenshot of the Windows gallery
(`docs/screenshots/windows-gallery.png`). Add a macOS one (`docs/screenshots/macos-gallery.png`, the gallery
with the Current theme section, dark mode, taken from the normal app rather than a dry run so no badge shows)
and show both, e.g. side by side in a two-cell table or one under the other with captions.
