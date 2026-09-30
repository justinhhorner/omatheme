using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Xaml.Media;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

/// <summary>One wallpaper of the current theme, in the gallery's "Current theme" section.</summary>
public sealed partial class CurrentWallpaperViewModel(string fileName, string path) : ObservableObject
{
    private ImageSource? _thumbnail;

    public string FileName { get; } = fileName;

    public ImageSource? Thumbnail => _thumbnail ??= Ui.Image(path, 320);

    /// <summary>This is the wallpaper on the desktop right now.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(AutomationName), nameof(Tooltip))]
    public partial bool IsCurrent { get; set; }

    [ObservableProperty]
    public partial bool IsApplying { get; set; }

    public string AutomationName => IsCurrent ? $"{FileName}, current wallpaper" : $"Set {FileName} as the desktop wallpaper";

    public string Tooltip => IsCurrent ? "Current wallpaper" : "Set as desktop wallpaper";
}

/// <summary>
/// The theme on the desktop, highlighted above the gallery. Clicking one of its wallpapers sets it
/// straight away (see <see cref="DesktopStore.SetCurrentWallpaperAsync"/>).
/// </summary>
public sealed partial class CurrentThemeViewModel : ObservableObject
{
    private readonly DesktopStore _desktop;
    private ImageSource? _screenshot;

    public CurrentThemeViewModel(InstalledTheme theme, DesktopStore desktop)
    {
        Theme = theme;
        _desktop = desktop;
        Wallpapers = theme.Wallpapers
            .Select(f => new CurrentWallpaperViewModel(f, theme.WallpaperPath(f)))
            .ToList();
        Colors = ColorChipViewModel.Summary(theme.Palette);
        Refresh();
    }

    public InstalledTheme Theme { get; }

    public string Name => Theme.Name;

    public string Details => $"{Ui.ModeName(Theme.Mode)} theme · {InstalledThemeViewModel.WallpaperCount(Theme.Wallpapers.Count)}";

    public IReadOnlyList<ColorChipViewModel> Colors { get; }

    public IReadOnlyList<CurrentWallpaperViewModel> Wallpapers { get; }

    public bool HasWallpapers => Wallpapers.Count > 0;

    /// <summary>The theme's screenshot, or its first wallpaper if it has none.</summary>
    public ImageSource? Screenshot => _screenshot ??=
        Ui.Image(Theme.ScreenshotPath ?? Theme.Wallpapers.Select(Theme.WallpaperPath).FirstOrDefault(), 480);

    public ResultBar Result { get; } = new();

    /// <summary>Moves the "current" mark and the spinner to match the desktop.</summary>
    public void Refresh()
    {
        foreach (var wallpaper in Wallpapers)
        {
            wallpaper.IsCurrent = wallpaper.FileName == _desktop.CurrentWallpaper;
            wallpaper.IsApplying = wallpaper.FileName == _desktop.SettingWallpaper;
        }
    }

    public async Task SetWallpaperAsync(CurrentWallpaperViewModel wallpaper)
    {
        Result.Close();
        if (await _desktop.SetCurrentWallpaperAsync(wallpaper.FileName) is { } problem)
            Result.Show(problem);
    }
}
