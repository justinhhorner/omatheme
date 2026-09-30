using System.ComponentModel;
using System.Diagnostics;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

public sealed partial class SettingsViewModel : ObservableObject
{
    private readonly Preferences _preferences;
    private readonly DesktopStore _desktop;
    private readonly ThemeLibrary _library;
    private readonly AppPaths _paths;
    private readonly AppEnvironment _environment;
    private bool _loading;

    public SettingsViewModel(Preferences preferences, DesktopStore desktop, ThemeLibrary library, AppPaths paths, AppEnvironment environment)
    {
        _preferences = preferences;
        _desktop = desktop;
        _library = library;
        _paths = paths;
        _environment = environment;

        _loading = true;
        var defaults = preferences.Settings.ApplyDefaults;
        ApplyWallpaper = defaults.Wallpaper;
        ApplyMode = defaults.AppearanceMode;
        ApplyAccent = defaults.AccentColor;
        Fit = defaults.Fit;
        _loading = false;

        HasOriginalSnapshot = desktop.HasOriginalSnapshot;
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

    public ResultBar Result { get; } = new();

    public string RestoreDescription => HasOriginalSnapshot
        ? "Puts back the wallpaper, light/dark mode and accent color you had before applying your first theme."
        : "Your desktop is saved automatically the first time you apply a theme. Nothing has been changed yet.";

    public string GitHubText => _environment.GitHubToken is null
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
        _preferences.Update(s => s with
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
        Result.Show(await _desktop.RestoreOriginalAsync());
        HasOriginalSnapshot = _desktop.HasOriginalSnapshot;
    }

    [RelayCommand]
    private void OpenDataFolder()
    {
        try
        {
            Process.Start(new ProcessStartInfo { FileName = _paths.Root, UseShellExecute = true });
        }
        catch (Win32Exception e)
        {
            Result.Show(InfoBarSeverity.Error, "Couldn't open the data folder", e.Message);
        }
    }

    [RelayCommand]
    private void ClearCache()
    {
        Result.Show(_library.ClearCache());
        RefreshStorage();
    }

    private void RefreshStorage()
    {
        var count = _library.Installed.Count;
        StorageText = $"{count} downloaded {(count == 1 ? "theme" : "themes")} · {Ui.FormatBytes(StorageInfo.DirectorySize(_paths.ThemesDir))}";
        CacheText = $"Catalog and GitHub responses · {Ui.FormatBytes(StorageInfo.DirectorySize(_paths.CacheDir))}";
    }
}
