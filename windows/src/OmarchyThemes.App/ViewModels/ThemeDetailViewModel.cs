using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

/// <summary>
/// One theme. Downloaded themes render entirely from disk (no network); others are resolved
/// from GitHub on open. Download shows per-file progress and can be cancelled.
/// </summary>
public sealed partial class ThemeDetailViewModel : ObservableObject
{
    private readonly ThemeLibrary _library;
    private readonly DesktopStore _desktop;
    private readonly TerminalStore _terminal;
    private readonly NavigationService _navigation;
    private DownloadOperation? _download;
    private ImageSource? _screenshot;

    public ThemeDetailViewModel(ThemeLibrary library, DesktopStore desktop, TerminalStore terminal, NavigationService navigation)
    {
        _library = library;
        _desktop = desktop;
        _terminal = terminal;
        _navigation = navigation;
    }

    public CatalogEntry Entry { get; private set; } = null!;

    public string Name => Entry.Name;

    public string RepoDisplay => Entry.RepoDisplay;

    public Uri RepoUri => new(Entry.RepoUrl);

    public ImageSource? Screenshot => _screenshot ??= Ui.Image(Installed?.ScreenshotPath ?? Entry.ScreenshotUrl, 1200);

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsInstalled), nameof(ApplyButtonText), nameof(CanApply), nameof(Mode), nameof(ModeText), nameof(IsActive))]
    public partial InstalledTheme? Installed { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanApply), nameof(Mode), nameof(ModeText))]
    public partial ThemeDetails? Details { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowPalette))]
    public partial bool IsResolving { get; set; }

    [ObservableProperty]
    public partial string? ResolveError { get; set; }

    [ObservableProperty]
    public partial string? Notice { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowPalette))]
    public partial IReadOnlyList<ColorChipViewModel> KeyColors { get; set; } = [];

    [ObservableProperty]
    public partial IReadOnlyList<ColorChipViewModel> Swatches { get; set; } = [];

    [ObservableProperty]
    public partial string? PaletteError { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(WallpapersHeader), nameof(HasWallpapers))]
    public partial IReadOnlyList<WallpaperItemViewModel> Wallpapers { get; set; } = [];

    [ObservableProperty]
    public partial WallpaperItemViewModel? SelectedWallpaper { get; set; }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanApply))]
    [NotifyCanExecuteChangedFor(nameof(DownloadCommand))]
    public partial bool IsDownloading { get; set; }

    [ObservableProperty]
    public partial double DownloadPercent { get; set; }

    [ObservableProperty]
    public partial bool IsDownloadIndeterminate { get; set; }

    [ObservableProperty]
    public partial string DownloadStatus { get; set; } = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(CanApply))]
    public partial bool IsApplying { get; set; }

    public ResultBar Result { get; } = new();

    public bool IsInstalled => Installed is not null;

    public bool IsActive => IsInstalled && _desktop.ActiveSlug == Entry.Slug;

    public bool CanApply => !IsDownloading && !IsApplying && (IsInstalled || Details?.CanApply == true);

    public string ApplyButtonText => IsInstalled ? "Apply to desktop" : "Download and apply";

    public AppearanceMode Mode => Installed?.Mode ?? Details?.Mode ?? AppearanceMode.Dark;

    public string ModeText => Installed is null && Details is null ? "" : $"{Ui.ModeName(Mode)} theme";

    public bool ShowPalette => !IsResolving && KeyColors.Count > 0;

    public bool HasWallpapers => Wallpapers.Count > 0;

    public string WallpapersHeader => Wallpapers.Count == 1 ? "Wallpaper" : $"Wallpapers ({Wallpapers.Count})";

    public Palette? Palette => Installed?.Palette ?? Details?.Palette;

    public async Task LoadAsync(CatalogEntry entry)
    {
        Entry = entry;
        OnPropertyChanged(string.Empty);
        RefreshTerminalState();

        // A download started earlier (then navigated away from) is still running: show its live progress.
        if (_library.RunningDownload(entry.Slug) is { } running)
            _ = ObserveDownloadAsync(running, selectedIndex: 0);

        Installed = _library.Get(entry.Slug);
        if (Installed is not null)
        {
            ShowInstalled(Installed);
            return; // Everything needed is on disk; no network.
        }

        if (_library.CachedDetails(entry.Slug) is { } cached)
        {
            ShowDetails(cached);
            return;
        }
        await ResolveAsync();
    }

    [RelayCommand]
    private async Task ResolveAsync()
    {
        IsResolving = true;
        ResolveError = null;
        try
        {
            ShowDetails(await _library.ResolveAsync(Entry));
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            ResolveError = ExpectedErrors.Describe(e);
        }
        finally
        {
            IsResolving = false;
        }
    }

    private void ShowDetails(ThemeDetails details)
    {
        Details = details;
        KeyColors = ColorChipViewModel.KeyColors(details.Palette);
        Swatches = ColorChipViewModel.Swatches(details.Palette);
        PaletteError = details.PaletteError;
        Wallpapers = details.Wallpapers
            .Select(w => new WallpaperItemViewModel(w.FileName, w.DownloadUrl.AbsoluteUri, null))
            .ToList();
        SelectedWallpaper = Wallpapers.FirstOrDefault();
        Notice = details.IsStale
            ? "GitHub couldn't be reached" + (details.StaleReason is GitHubRateLimitException rl ? $" ({rl.Message})" : "") + ", so this is a cached copy."
            : null;
        if (!details.CanApply)
            Result.Show(InfoBarSeverity.Warning, "Can't apply this theme",
                "It has no wallpapers and no readable color palette in a format this app understands.");
    }

    /// <param name="selectedIndex">The wallpaper to select; by default the one last applied, else the first.</param>
    private void ShowInstalled(InstalledTheme theme, int? selectedIndex = null)
    {
        KeyColors = ColorChipViewModel.KeyColors(theme.Palette);
        Swatches = ColorChipViewModel.Swatches(theme.Palette);
        PaletteError = theme.Palette is null ? "Couldn't read this theme's palette, so only the wallpaper can be applied." : null;
        Wallpapers = theme.Wallpapers
            .Select(f => new WallpaperItemViewModel(f, theme.WallpaperPath(f), f))
            .ToList();

        var preferred = _desktop.PreferredWallpaper(theme);
        SelectedWallpaper = (selectedIndex is { } index ? Wallpapers.ElementAtOrDefault(index) : null)
            ?? Wallpapers.FirstOrDefault(w => w.LocalFile == preferred)
            ?? Wallpapers.FirstOrDefault();
    }

    private bool CanDownload() => !IsDownloading;

    /// <summary>
    /// Downloads the theme, or joins its download if one is already running. The download itself
    /// belongs to <see cref="ThemeDownloads"/>, so it keeps going if the user leaves this page.
    /// </summary>
    [RelayCommand(CanExecute = nameof(CanDownload))]
    public Task<bool> DownloadAsync()
    {
        Result.Close();
        var selectedIndex = SelectedWallpaper is null ? 0 : Ui.IndexOf(Wallpapers, SelectedWallpaper);
        return ObserveDownloadAsync(_library.Download(Entry), selectedIndex);
    }

    private async Task<bool> ObserveDownloadAsync(DownloadOperation download, int selectedIndex)
    {
        if (_download == download)
            return (await download.Completion).Theme is not null;

        _download = download;
        IsDownloading = true;
        MirrorProgress(download);
        download.PropertyChanged += OnDownloadProgressChanged;
        try
        {
            var outcome = await download.Completion;
            if (outcome.Theme is { } installed)
            {
                Details = outcome.Details;
                Installed = installed;
                ShowInstalled(installed, selectedIndex);
                return true;
            }
            Result.Show(ThemeLibrary.DescribeFailedDownload(outcome));
            return false;
        }
        finally
        {
            download.PropertyChanged -= OnDownloadProgressChanged;
            _download = null;
            IsDownloading = false;
        }
    }

    private void OnDownloadProgressChanged(object? sender, System.ComponentModel.PropertyChangedEventArgs e)
    {
        if (sender is DownloadOperation download)
            MirrorProgress(download);
    }

    private void MirrorProgress(DownloadOperation download)
    {
        var status = download.Status;
        DownloadPercent = status.Percent ?? 0;
        IsDownloadIndeterminate = status.Percent is null;
        DownloadStatus = status.Phase switch
        {
            DownloadPhase.Resolving => "Reading theme from GitHub…",
            DownloadPhase.Finishing => "Finishing up…",
            _ => $"Downloading wallpaper {status.FileIndex + 1} of {status.FileCount} · "
                 + $"{Ui.FormatBytes(status.BytesDone)} of {Ui.FormatBytes(status.TotalBytes)}",
        };
    }

    [RelayCommand]
    private void CancelDownload() => _download?.Cancel();

    /// <summary>Apply with the Apply dialog's choices.</summary>
    public async Task ApplyAsync(ApplyOptions options, bool rememberOptions)
    {
        if (Installed is null)
            return;
        IsApplying = true;
        Result.Close();
        try
        {
            Result.Show(await _desktop.ApplyAsync(Installed, SelectedWallpaper?.LocalFile, options, rememberOptions));
            OnPropertyChanged(nameof(IsActive));
        }
        finally
        {
            IsApplying = false;
        }
    }

    public void Remove()
    {
        if (Installed is null)
            return;
        var banner = _library.Remove(Installed);
        if (banner.Kind != SummaryKind.Error)
        {
            Installed = null;
            if (Details is not null)
                ShowDetails(Details);
            else
                _navigation.GoBack();
        }
        Result.Show(banner);
    }
}
