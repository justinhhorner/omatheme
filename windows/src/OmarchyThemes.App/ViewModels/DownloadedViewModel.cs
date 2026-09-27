using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.ViewModels;

/// <summary>Downloaded themes, with one-click apply using the saved Apply defaults.</summary>
public sealed partial class DownloadedViewModel : ObservableObject
{
    private readonly ThemeStore _store;
    private readonly ApplyService _apply;
    private readonly SettingsStore _settings;
    private readonly NavigationService _navigation;
    private readonly GalleryViewModel _gallery;
    private readonly DispatcherQueue _dispatcher;

    public DownloadedViewModel(ThemeStore store, ApplyService apply, SettingsStore settings, NavigationService navigation, GalleryViewModel gallery)
    {
        _store = store;
        _apply = apply;
        _settings = settings;
        _navigation = navigation;
        _gallery = gallery;
        _dispatcher = DispatcherQueue.GetForCurrentThread();

        _store.Changed += (_, _) => _dispatcher.TryEnqueue(Reload);
        _apply.ActiveThemeChanged += (_, _) => _dispatcher.TryEnqueue(UpdateActive);
        Reload();
    }

    public ObservableCollection<InstalledThemeViewModel> Items { get; } = [];

    [ObservableProperty]
    public partial bool IsEmpty { get; set; }

    [ObservableProperty]
    public partial string Subtitle { get; set; } = "";

    [ObservableProperty]
    public partial bool IsResultOpen { get; set; }

    [ObservableProperty]
    public partial string ResultTitle { get; set; } = "";

    [ObservableProperty]
    public partial string ResultMessage { get; set; } = "";

    [ObservableProperty]
    public partial InfoBarSeverity ResultSeverity { get; set; }

    public void Reload()
    {
        var active = _apply.ActiveSlug;
        Items.Clear();
        foreach (var theme in _store.List())
            Items.Add(new InstalledThemeViewModel(theme, theme.Slug == active));
        IsEmpty = Items.Count == 0;
        Subtitle = Items.Count switch
        {
            0 => "",
            1 => "1 theme saved on this PC. Downloaded themes apply without an internet connection.",
            var n => $"{n} themes saved on this PC. Downloaded themes apply without an internet connection.",
        };
    }

    private void UpdateActive()
    {
        var active = _apply.ActiveSlug;
        foreach (var item in Items)
            item.IsActive = item.Theme.Slug == active;
    }

    /// <summary>One-click apply with the defaults from Settings.</summary>
    [RelayCommand]
    private Task SetAsDesktopAsync(InstalledThemeViewModel item) =>
        ApplyAsync(item, _settings.Load().ApplyDefaults);

    public async Task ApplyAsync(InstalledThemeViewModel item, ApplyOptions options)
    {
        if (Items.Any(i => i.IsApplying))
            return;
        item.IsApplying = true;
        IsResultOpen = false;
        try
        {
            var settings = _settings.Load();
            var wallpaper = settings.LastAppliedSlug == item.Theme.Slug ? settings.LastAppliedWallpaper : null;
            var summary = await _apply.ApplyAsync(item.Theme, wallpaper, options);
            ResultSeverity = Ui.Severity(summary.Kind);
            ResultTitle = summary.Title;
            ResultMessage = summary.Message;
            IsResultOpen = true;
        }
        finally
        {
            item.IsApplying = false;
        }
    }

    public void Remove(InstalledThemeViewModel item) => _store.Remove(item.Theme.Slug);

    public void Open(InstalledThemeViewModel item) =>
        _navigation.OpenTheme(_gallery.Find(item.Theme.Slug)
            ?? new CatalogEntry(item.Theme.Slug, item.Theme.Name, item.Theme.RepoUrl, ScreenshotUrl: null));

    [RelayCommand]
    private void BrowseThemes() => _navigation.ShowSection("Gallery");
}
