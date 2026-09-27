using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

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
/// straight away: wallpaper only (with the saved fit and the theme's fill color), since the theme's
/// light/dark mode and accent are already applied.
/// </summary>
public sealed partial class CurrentThemeViewModel : ObservableObject
{
    private readonly ApplyService _apply;
    private readonly SettingsStore _settings;
    private ImageSource? _screenshot;

    public CurrentThemeViewModel(InstalledTheme theme, string? currentWallpaper, ApplyService apply, SettingsStore settings)
    {
        Theme = theme;
        _apply = apply;
        _settings = settings;
        Wallpapers = theme.Wallpapers
            .Select(f => new CurrentWallpaperViewModel(f, theme.WallpaperPath(f)))
            .ToList();
        Colors = ColorChipViewModel.KeyColors(theme.Palette).Take(3)
            .Concat(ColorChipViewModel.Swatches(theme.Palette).Take(8))
            .ToList();
        MarkCurrent(currentWallpaper);
    }

    public InstalledTheme Theme { get; }

    public string Name => Theme.Name;

    public string Details
    {
        get
        {
            var mode = Theme.Mode == AppearanceMode.Light ? "Light theme" : "Dark theme";
            var count = Theme.Wallpapers.Count == 1 ? "1 wallpaper" : $"{Theme.Wallpapers.Count} wallpapers";
            return $"{mode} · {count}";
        }
    }

    public IReadOnlyList<ColorChipViewModel> Colors { get; }

    public IReadOnlyList<CurrentWallpaperViewModel> Wallpapers { get; }

    public bool HasWallpapers => Wallpapers.Count > 0;

    /// <summary>The theme's screenshot, or its first wallpaper if it has none.</summary>
    public ImageSource? Screenshot => _screenshot ??=
        Ui.Image(Theme.ScreenshotPath ?? Theme.Wallpapers.Select(Theme.WallpaperPath).FirstOrDefault(), 480);

    [ObservableProperty]
    public partial bool IsBusy { get; set; }

    [ObservableProperty]
    public partial bool IsResultOpen { get; set; }

    [ObservableProperty]
    public partial string ResultTitle { get; set; } = "";

    [ObservableProperty]
    public partial string ResultMessage { get; set; } = "";

    [ObservableProperty]
    public partial InfoBarSeverity ResultSeverity { get; set; }

    public void MarkCurrent(string? fileName)
    {
        foreach (var wallpaper in Wallpapers)
            wallpaper.IsCurrent = wallpaper.FileName == fileName;
    }

    public async Task SetWallpaperAsync(CurrentWallpaperViewModel wallpaper)
    {
        if (IsBusy || wallpaper.IsCurrent)
            return;

        IsBusy = true;
        wallpaper.IsApplying = true;
        IsResultOpen = false;
        try
        {
            var options = new ApplyOptions
            {
                Wallpaper = true,
                AppearanceMode = false,
                AccentColor = false,
                Fit = _settings.Load().ApplyDefaults.Fit,
            };
            var summary = await _apply.ApplyAsync(Theme, wallpaper.FileName, options);
            // Success shows as the "current" mark moving; only problems need words.
            if (summary.Kind != SummaryKind.Success)
            {
                ResultSeverity = Ui.Severity(summary.Kind);
                ResultTitle = summary.Kind == SummaryKind.Error ? "Couldn't change the wallpaper" : summary.Title;
                ResultMessage = summary.Message;
                IsResultOpen = true;
            }
        }
        finally
        {
            wallpaper.IsApplying = false;
            IsBusy = false;
        }
    }
}
