# Windows → macOS: changes to bring across

Written on Windows (Sep 2026). It lists what the Windows app changed that the macOS app doesn't have yet, so a
session on a Mac can implement it (the Swift app can't be built or checked on Windows). Port behaviour, not
code, and keep macOS conventions (see CLAUDE.md).

The usual rule applies: never apply a theme or restore the desktop on the real machine without asking. Run UI
checks with `OMARCHY_THEMES_DRY_RUN=1` and `OMARCHY_THEMES_DATA_DIR=<temp>`.

When an item is done, delete it from this file (and this file once it's empty), and update README/CLAUDE.md.

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
