using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

/// <summary>Downloaded themes, with one-click apply using the saved Apply defaults.</summary>
public sealed partial class DownloadedViewModel : ObservableObject
{
    private readonly ThemeLibrary _library;
    private readonly DesktopStore _desktop;
    private readonly CatalogStore _catalog;
    private readonly NavigationService _navigation;
    private readonly DispatcherQueue _dispatcher;

    public DownloadedViewModel(ThemeLibrary library, DesktopStore desktop, CatalogStore catalog, NavigationService navigation)
    {
        _library = library;
        _desktop = desktop;
        _catalog = catalog;
        _navigation = navigation;
        _dispatcher = DispatcherQueue.GetForCurrentThread();

        _library.Changed += (_, _) => _dispatcher.TryEnqueue(ShowThemes);
        _desktop.Changed += (_, _) => UpdateState();
        ShowThemes();
    }

    public ObservableCollection<InstalledThemeViewModel> Items { get; } = [];

    [ObservableProperty]
    public partial bool IsEmpty { get; set; }

    [ObservableProperty]
    public partial string Subtitle { get; set; } = "";

    public ResultBar Result { get; } = new();

    /// <summary>Reads the downloaded themes from disk again (e.g. when the page is shown).</summary>
    public void Reload()
    {
        _library.Reload();
        ShowThemes();
    }

    private void ShowThemes()
    {
        Items.Clear();
        foreach (var theme in _library.Installed)
            Items.Add(new InstalledThemeViewModel(theme, isActive: false));
        UpdateState();
        IsEmpty = Items.Count == 0;
        Subtitle = Items.Count switch
        {
            0 => "",
            1 => "1 theme saved on this PC. Downloaded themes apply without an internet connection.",
            var n => $"{n} themes saved on this PC. Downloaded themes apply without an internet connection.",
        };
    }

    private void UpdateState()
    {
        foreach (var item in Items)
        {
            item.IsActive = item.Theme.Slug == _desktop.ActiveSlug;
            item.IsApplying = item.Theme.Slug == _desktop.ApplyingSlug;
        }
    }

    /// <summary>One-click apply with the defaults from Settings.</summary>
    [RelayCommand]
    private async Task SetAsDesktopAsync(InstalledThemeViewModel item)
    {
        Result.Close();
        Result.Show(await _desktop.ApplyWithDefaultsAsync(item.Theme));
    }

    /// <summary>Apply with the Apply dialog's choices.</summary>
    public async Task ApplyAsync(InstalledThemeViewModel item, ApplyOptions options, bool rememberOptions)
    {
        Result.Close();
        Result.Show(await _desktop.ApplyAsync(item.Theme, _desktop.PreferredWallpaper(item.Theme), options, rememberOptions));
    }

    public void Remove(InstalledThemeViewModel item)
    {
        var banner = _library.Remove(item.Theme);
        // Removal is visible in the list; only a failure needs words.
        if (banner.Kind == SummaryKind.Error)
            Result.Show(banner);
    }

    public void Open(InstalledThemeViewModel item) => _navigation.OpenTheme(_catalog.EntryFor(item.Theme));

    [RelayCommand]
    private void BrowseThemes() => _navigation.ShowSection(Sections.Gallery);
}
