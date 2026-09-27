using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows;

namespace OmarchyThemes.App.ViewModels;

/// <summary>
/// One theme. Downloaded themes render entirely from disk (no network); others are resolved
/// from GitHub on open. Download shows per-file progress and can be cancelled.
/// </summary>
public sealed partial class ThemeDetailViewModel : ObservableObject
{
    private readonly ThemeDetailsService _details;
    private readonly ThemeStore _store;
    private readonly ApplyService _apply;
    private readonly SettingsStore _settings;
    private readonly NavigationService _navigation;
    private readonly ThemeDownloads _downloads;
    private readonly WindowsTerminalSchemes _terminal;
    private DownloadOperation? _download;
    private ImageSource? _screenshot;

    public ThemeDetailViewModel(
        ThemeDetailsService details, ThemeStore store, ApplyService apply, SettingsStore settings,
        NavigationService navigation, ThemeDownloads downloads, WindowsTerminalSchemes terminal)
    {
        _details = details;
        _store = store;
        _apply = apply;
        _settings = settings;
        _navigation = navigation;
        _downloads = downloads;
        _terminal = terminal;
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

    [ObservableProperty]
    public partial bool IsResultOpen { get; set; }

    [ObservableProperty]
    public partial string ResultTitle { get; set; } = "";

    [ObservableProperty]
    public partial string ResultMessage { get; set; } = "";

    [ObservableProperty]
    public partial InfoBarSeverity ResultSeverity { get; set; }

    public bool IsInstalled => Installed is not null;

    public bool IsActive => IsInstalled && _apply.ActiveSlug == Entry.Slug;

    public bool CanApply => !IsDownloading && !IsApplying && (IsInstalled || Details?.CanApply == true);

    public string ApplyButtonText => IsInstalled ? "Apply to desktop" : "Download and apply";

    public AppearanceMode Mode => Installed?.Mode ?? Details?.Mode ?? AppearanceMode.Dark;

    public string ModeText => Installed is null && Details is null ? "" : Mode == AppearanceMode.Light ? "Light theme" : "Dark theme";

    public bool ShowPalette => !IsResolving && KeyColors.Count > 0;

    public bool HasWallpapers => Wallpapers.Count > 0;

    public string WallpapersHeader => Wallpapers.Count == 1 ? "Wallpaper" : $"Wallpapers ({Wallpapers.Count})";

    public Palette? Palette => Installed?.Palette ?? Details?.Palette;

    public async Task LoadAsync(CatalogEntry entry)
    {
        Entry = entry;
        OnPropertyChanged(string.Empty);

        // A download started earlier (then navigated away from) is still running: show its live progress.
        RefreshTerminalState();

        if (_downloads.Get(entry.Slug) is { } running)
            _ = ObserveDownloadAsync(running, selectedIndex: 0);

        Installed = _store.Get(entry.Slug);
        if (Installed is not null)
        {
            ShowInstalled(Installed);
            return; // Everything needed is on disk; no network.
        }

        if (_details.TryGetCached(entry.Slug) is { } cached)
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
            ShowDetails(await _details.ResolveAsync(Entry));
        }
        catch (Exception e) when (e is GitHubException or ThemeResolveException or HttpRequestException or TaskCanceledException)
        {
            ResolveError = e is HttpRequestException or TaskCanceledException
                ? "Couldn't reach GitHub. Check your internet connection and try again."
                : e.Message;
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
            ShowResult(InfoBarSeverity.Warning, "Can't apply this theme",
                "It has no wallpapers and no readable color palette in a format this app understands.");
    }

    private void ShowInstalled(InstalledTheme theme, int selectedIndex = -1)
    {
        KeyColors = ColorChipViewModel.KeyColors(theme.Palette);
        Swatches = ColorChipViewModel.Swatches(theme.Palette);
        PaletteError = theme.Palette is null ? "Couldn't read this theme's palette, so only the wallpaper can be applied." : null;
        Wallpapers = theme.Wallpapers
            .Select(f => new WallpaperItemViewModel(f, theme.WallpaperPath(f), f))
            .ToList();

        if (selectedIndex < 0)
        {
            var settings = _settings.Load();
            selectedIndex = settings.LastAppliedSlug == theme.Slug && settings.LastAppliedWallpaper is { } last
                ? Math.Max(0, Wallpapers.ToList().FindIndex(w => w.LocalFile == last))
                : 0;
        }
        SelectedWallpaper = Wallpapers.ElementAtOrDefault(selectedIndex) ?? Wallpapers.FirstOrDefault();
    }

    private bool CanDownload() => !IsDownloading;

    /// <summary>
    /// Downloads the theme, or joins its download if one is already running. The download itself
    /// belongs to <see cref="ThemeDownloads"/>, so it keeps going if the user leaves this page.
    /// </summary>
    [RelayCommand(CanExecute = nameof(CanDownload))]
    public Task<bool> DownloadAsync()
    {
        IsResultOpen = false;
        var selectedIndex = SelectedWallpaper is null ? 0 : Wallpapers.ToList().IndexOf(SelectedWallpaper);
        // Re-resolve when re-downloading so updates to the theme are picked up.
        return ObserveDownloadAsync(_downloads.Start(Entry, refresh: IsInstalled), selectedIndex);
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
            if (outcome.Cancelled)
                ShowResult(InfoBarSeverity.Informational, "Download cancelled", "Nothing was saved.");
            else
                ShowResult(InfoBarSeverity.Error, "Download failed", outcome.Error switch
                {
                    TaskCanceledException => "The connection timed out. Try again.",
                    { } error => error.Message,
                    null => "Something went wrong.",
                });
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

    public async Task ApplyAsync(ApplyOptions options)
    {
        if (Installed is null)
            return;
        IsApplying = true;
        IsResultOpen = false;
        try
        {
            var summary = await _apply.ApplyAsync(Installed, SelectedWallpaper?.LocalFile, options);
            ShowResult(Ui.Severity(summary.Kind), summary.Title, summary.Message);
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
        _store.Remove(Installed.Slug);
        Installed = null;
        if (Details is not null)
            ShowDetails(Details);
        else
            _navigation.GoBack();
        ShowResult(InfoBarSeverity.Informational, "Download removed", $"{Name} was removed from this PC.");
    }

    private void ShowResult(InfoBarSeverity severity, string title, string message)
    {
        ResultSeverity = severity;
        ResultTitle = title;
        ResultMessage = message;
        IsResultOpen = true;
    }
}
