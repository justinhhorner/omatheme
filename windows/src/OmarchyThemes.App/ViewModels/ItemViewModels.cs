using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Xaml.Media;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.App.ViewModels;

/// <summary>A palette color with a label, for swatch chips.</summary>
public sealed class ColorChipViewModel(string name, RgbColor color)
{
    public string Name { get; } = name;
    public string Hex { get; } = color.ToHex();
    public SolidColorBrush Brush { get; } = Ui.Brush(color);
    public SolidColorBrush Foreground { get; } = Ui.ContrastBrush(color);
    public string Tooltip => $"{Name} {Hex}";

    /// <summary>Background, foreground and accent (plus cursor/selection when present).</summary>
    public static IReadOnlyList<ColorChipViewModel> KeyColors(Palette? palette)
    {
        if (palette is null)
            return [];
        var chips = new List<ColorChipViewModel>
        {
            new("Background", palette.Background),
            new("Foreground", palette.Foreground),
            new("Accent", palette.Accent),
        };
        if (palette.Cursor is { } cursor) chips.Add(new("Cursor", cursor));
        if (palette.Selection is { } selection) chips.Add(new("Selection", selection));
        return chips;
    }

    public static IReadOnlyList<ColorChipViewModel> Swatches(Palette? palette) =>
        palette?.Swatches.Select(s => new ColorChipViewModel(s.Name, s.Color)).ToList() ?? [];

    /// <summary>A compact row for lists: background, foreground, accent, then the first 8 swatches.</summary>
    public static IReadOnlyList<ColorChipViewModel> Summary(Palette? palette) =>
        KeyColors(palette).Take(3).Concat(Swatches(palette).Take(8)).ToList();
}

/// <summary>A wallpaper thumbnail: a remote raw URL before download, a local file after.</summary>
public sealed class WallpaperItemViewModel(string fileName, string source, string? localFile)
{
    public string FileName { get; } = fileName;
    public string Source { get; } = source;

    /// <summary>File name inside the installed theme, or null if not downloaded.</summary>
    public string? LocalFile { get; } = localFile;

    public string PreviewLabel => $"Preview {FileName}";

    private ImageSource? _thumbnail;
    public ImageSource? Thumbnail => _thumbnail ??= Ui.Image(Source, 360);

    private ImageSource? _preview;

    /// <summary>For the large preview: full resolution, downsampled to at most ~2560 px wide.</summary>
    public ImageSource? Preview => _preview ??= Ui.Image(Source, 2560);
}

/// <summary>A card in the gallery grid.</summary>
public sealed partial class ThemeCardViewModel : ObservableObject
{
    private ImageSource? _screenshot;

    public ThemeCardViewModel(CatalogEntry entry, string? localScreenshot)
    {
        Entry = entry;
        LocalScreenshot = localScreenshot;
        RepoDisplay = entry.RepoDisplay;
    }

    public CatalogEntry Entry { get; }
    public string Name => Entry.Name;
    public string RepoDisplay { get; }

    /// <summary>Prefer the downloaded copy so downloaded themes render offline.</summary>
    private string? LocalScreenshot { get; }

    public ImageSource? Screenshot => _screenshot ??= Ui.Image(LocalScreenshot ?? Entry.ScreenshotUrl, 480);

    [ObservableProperty]
    public partial bool IsDownloaded { get; set; }

    [ObservableProperty]
    public partial bool IsActive { get; set; }

    public string AutomationName => IsDownloaded ? $"{Name}, downloaded" : Name;

    partial void OnIsDownloadedChanged(bool value) => OnPropertyChanged(nameof(AutomationName));
}

/// <summary>A downloaded theme in the Downloaded view.</summary>
public sealed partial class InstalledThemeViewModel : ObservableObject
{
    private ImageSource? _screenshot;

    public InstalledThemeViewModel(InstalledTheme theme, bool isActive)
    {
        Theme = theme;
        IsActive = isActive;
        Colors = ColorChipViewModel.Summary(theme.Palette);
    }

    public InstalledTheme Theme { get; }
    public string Name => Theme.Name;
    public string ModeText => Ui.ModeName(Theme.Mode);
    public IReadOnlyList<ColorChipViewModel> Colors { get; }

    public string Details => WallpaperCount(Theme.Wallpapers.Count);

    /// <summary>"No wallpaper", "1 wallpaper", "3 wallpapers".</summary>
    public static string WallpaperCount(int count) => count switch
    {
        0 => "No wallpaper",
        1 => "1 wallpaper",
        var n => $"{n} wallpapers",
    };

    public ImageSource? Screenshot => _screenshot ??= Ui.Image(Theme.ScreenshotPath, 480);

    [ObservableProperty]
    public partial bool IsActive { get; set; }

    [ObservableProperty]
    public partial bool IsApplying { get; set; }
}
