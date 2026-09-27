using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.ViewModels;

public sealed partial class SettingsViewModel : ObservableObject
{
    public static readonly IReadOnlyList<WallpaperFit> FitOptions = Enum.GetValues<WallpaperFit>();

    private readonly SettingsStore _settings;
    private readonly ApplyService _apply;
    private readonly AppPaths _paths;
    private readonly ThemeStore _store;
    private bool _loading;

    public SettingsViewModel(SettingsStore settings, ApplyService apply, AppPaths paths, ThemeStore store)
    {
        _settings = settings;
        _apply = apply;
        _paths = paths;
        _store = store;

        _loading = true;
        var defaults = settings.Load().ApplyDefaults;
        ApplyWallpaper = defaults.Wallpaper;
        ApplyMode = defaults.AppearanceMode;
        ApplyAccent = defaults.AccentColor;
        Fit = defaults.Fit;
        _loading = false;

        HasOriginalSnapshot = apply.HasOriginalSnapshot;
        RefreshStorage();
    }

    [ObservableProperty]
    public partial bool ApplyWallpaper { get; set; }

    [ObservableProperty]
    public partial bool ApplyMode { get; set; }

    [ObservableProperty]
    public partial bool ApplyAccent { get; set; }

    [ObservableProperty]
    public partial WallpaperFit Fit { get; set; }

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(RestoreOriginalCommand))]
    public partial bool HasOriginalSnapshot { get; set; }

    [ObservableProperty]
    public partial string StorageText { get; set; } = "";

    [ObservableProperty]
    public partial string CacheText { get; set; } = "";

    [ObservableProperty]
    public partial bool IsResultOpen { get; set; }

    [ObservableProperty]
    public partial string ResultTitle { get; set; } = "";

    [ObservableProperty]
    public partial string ResultMessage { get; set; } = "";

    [ObservableProperty]
    public partial InfoBarSeverity ResultSeverity { get; set; }

    public string RestoreDescription => HasOriginalSnapshot
        ? "Puts back the wallpaper, light/dark mode and accent color you had before applying your first theme."
        : "Your desktop is saved automatically the first time you apply a theme. Nothing has been changed yet.";

    public string GitHubText => StorageInfo.GitHubToken() is null
        ? "Browsing themes uses GitHub's public API (60 theme lookups per hour). Set a GITHUB_TOKEN environment variable to raise the limit."
        : "Using the token from the GITHUB_TOKEN environment variable for GitHub requests.";

    partial void OnApplyWallpaperChanged(bool value) => SaveDefaults();
    partial void OnApplyModeChanged(bool value) => SaveDefaults();
    partial void OnApplyAccentChanged(bool value) => SaveDefaults();
    partial void OnFitChanged(WallpaperFit value) => SaveDefaults();
    partial void OnHasOriginalSnapshotChanged(bool value) => OnPropertyChanged(nameof(RestoreDescription));

    private void SaveDefaults()
    {
        if (_loading)
            return;
        _settings.Update(s => s with
        {
            ApplyDefaults = new ApplyOptions
            {
                Wallpaper = ApplyWallpaper,
                AppearanceMode = ApplyMode,
                AccentColor = ApplyAccent,
                Fit = Fit,
            },
        });
    }

    [RelayCommand(CanExecute = nameof(HasOriginalSnapshot))]
    private async Task RestoreOriginalAsync()
    {
        try
        {
            var restored = await _apply.RestoreOriginalAsync();
            ShowResult(restored ? InfoBarSeverity.Success : InfoBarSeverity.Informational,
                restored ? "Original desktop restored" : "Nothing to restore",
                restored ? "Your previous wallpaper and colors are back." : "No saved desktop was found.");
        }
        catch (Exception e)
        {
            ShowResult(InfoBarSeverity.Error, "Couldn't restore your desktop", e.Message);
        }
        HasOriginalSnapshot = _apply.HasOriginalSnapshot;
    }

    [RelayCommand]
    private void OpenDataFolder() =>
        Process.Start(new ProcessStartInfo { FileName = _paths.Root, UseShellExecute = true });

    [RelayCommand]
    private void ClearCache()
    {
        try
        {
            if (Directory.Exists(_paths.CacheDir))
                Directory.Delete(_paths.CacheDir, recursive: true);
            Directory.CreateDirectory(_paths.CacheDir);
            ShowResult(InfoBarSeverity.Success, "Cache cleared", "The catalog and theme details will be downloaded again when needed. Downloaded themes were kept.");
        }
        catch (IOException e)
        {
            ShowResult(InfoBarSeverity.Error, "Couldn't clear the cache", e.Message);
        }
        RefreshStorage();
    }

    private void RefreshStorage()
    {
        var count = _store.List().Count;
        StorageText = $"{count} downloaded {(count == 1 ? "theme" : "themes")} · {Ui.FormatBytes(StorageInfo.DirectorySize(_paths.ThemesDir))}";
        CacheText = $"Catalog and GitHub responses · {Ui.FormatBytes(StorageInfo.DirectorySize(_paths.CacheDir))}";
    }

    private void ShowResult(InfoBarSeverity severity, string title, string message)
    {
        ResultSeverity = severity;
        ResultTitle = title;
        ResultMessage = message;
        IsResultOpen = true;
    }
}
