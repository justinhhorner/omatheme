using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

/// <summary>
/// The theme catalog (<see cref="CatalogStore"/>) as cards: the cached copy straight away, refreshed
/// in the background when it's old. Refresh always re-fetches.
/// </summary>
public sealed partial class GalleryViewModel : ObservableObject
{
    private readonly CatalogStore _catalog;
    private readonly ThemeLibrary _library;
    private readonly DesktopStore _desktop;
    private readonly NavigationService _navigation;
    private readonly DispatcherQueue _dispatcher;
    private IReadOnlyList<CatalogEntry>? _shownEntries;
    private Dictionary<string, ThemeCardViewModel> _cards = [];

    public GalleryViewModel(CatalogStore catalog, ThemeLibrary library, DesktopStore desktop, NavigationService navigation)
    {
        _catalog = catalog;
        _library = library;
        _desktop = desktop;
        _navigation = navigation;
        _dispatcher = DispatcherQueue.GetForCurrentThread();

        _catalog.Changed += (_, _) => ShowCatalog();
        _library.Changed += (_, _) => _dispatcher.TryEnqueue(UpdateBadges);
        _desktop.Changed += (_, _) => UpdateBadges();
    }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsEmptyResult), nameof(ResultText))]
    public partial IReadOnlyList<ThemeCardViewModel> VisibleThemes { get; set; } = [];

    /// <summary>Visible themes that ship with Omarchy ("Included with Omarchy" section).</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasDefaultSection))]
    public partial IReadOnlyList<ThemeCardViewModel> DefaultThemes { get; set; } = [];

    /// <summary>Visible themes from omarchy.org/themes ("Community" section).</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(HasCommunitySection))]
    public partial IReadOnlyList<ThemeCardViewModel> CommunityThemes { get; set; } = [];

    /// <summary>Shown when Omarchy's own themes couldn't be listed (the community gallery still loads).</summary>
    [ObservableProperty]
    public partial string? DefaultThemesNotice { get; set; }

    public bool HasDefaultSection => DefaultThemes.Count > 0;
    public bool HasCommunitySection => CommunityThemes.Count > 0;

    /// <summary>The theme on the desktop, if it's still downloaded ("Current theme" section).</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowCurrentTheme))]
    public partial CurrentThemeViewModel? CurrentTheme { get; set; }

    /// <summary>Hidden while searching, so results come first.</summary>
    public bool ShowCurrentTheme => CurrentTheme is not null && string.IsNullOrWhiteSpace(SearchText);

    [ObservableProperty]
    public partial string SearchText { get; set; } = "";

    [ObservableProperty]
    public partial bool ShowDownloadedOnly { get; set; }

    /// <summary>First load with nothing cached.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowLoading))]
    public partial bool IsLoading { get; set; }

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(RefreshCommand))]
    public partial bool IsRefreshing { get; set; }

    /// <summary>Error shown in place of the grid when there is no catalog at all.</summary>
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowError))]
    public partial string? ErrorMessage { get; set; }

    /// <summary>Warning shown above the grid, e.g. "showing the cached catalog".</summary>
    [ObservableProperty]
    public partial string? Notice { get; set; }

    [ObservableProperty]
    public partial string Subtitle { get; set; } = "";

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(ShowLoading), nameof(ShowError), nameof(IsEmptyResult))]
    public partial bool HasThemes { get; set; }

    public bool ShowLoading => IsLoading && !HasThemes;
    public bool ShowError => ErrorMessage is not null && !HasThemes;
    public bool IsEmptyResult => HasThemes && VisibleThemes.Count == 0;

    public string ResultText => ShowDownloadedOnly && string.IsNullOrWhiteSpace(SearchText)
        ? "You haven't downloaded any themes yet."
        : $"No themes match “{SearchText.Trim()}”.";

    partial void OnSearchTextChanged(string value)
    {
        ApplyFilter();
        OnPropertyChanged(nameof(ShowCurrentTheme));
    }

    partial void OnShowDownloadedOnlyChanged(bool value) => ApplyFilter();

    public Task EnsureLoadedAsync()
    {
        // Local only, so it shows even when the catalog can't load.
        UpdateCurrentTheme();
        return _catalog.LoadIfNeededAsync();
    }

    private bool CanRefresh() => !IsRefreshing;

    [RelayCommand(CanExecute = nameof(CanRefresh))]
    private Task RefreshAsync() => _catalog.RefreshAsync();

    public void Open(ThemeCardViewModel card) => _navigation.OpenTheme(card.Entry);

    public void OpenCurrentTheme()
    {
        if (CurrentTheme?.Theme is { } theme)
            _navigation.OpenTheme(_catalog.EntryFor(theme));
    }

    private void ShowCatalog()
    {
        IsLoading = _catalog.IsLoading;
        IsRefreshing = _catalog.IsRefreshing;
        ErrorMessage = _catalog.Error;
        Notice = _catalog.Notice;
        DefaultThemesNotice = _catalog.DefaultThemesNotice;

        if (!ReferenceEquals(_catalog.Entries, _shownEntries))
        {
            _shownEntries = _catalog.Entries;
            var screenshots = _library.Installed.ToDictionary(t => t.Slug, t => t.ScreenshotPath);
            _cards = _catalog.Entries.ToDictionary(
                e => e.Slug, e => new ThemeCardViewModel(e, screenshots.GetValueOrDefault(e.Slug)));
            HasThemes = _cards.Count > 0;
            UpdateBadges();
        }
    }

    private void UpdateBadges()
    {
        var installed = _library.InstalledSlugs;
        var active = _desktop.ActiveSlug;
        foreach (var card in _cards.Values)
        {
            card.IsDownloaded = installed.Contains(card.Entry.Slug);
            card.IsActive = card.Entry.Slug == active;
        }
        ApplyFilter(installed);
        UpdateCurrentTheme();
    }

    private void UpdateCurrentTheme()
    {
        var theme = _desktop.CurrentTheme;
        if (theme is null)
            CurrentTheme = null;
        else if (CurrentTheme?.Theme is { } shown && shown.Slug == theme.Slug && shown.DownloadedAt == theme.DownloadedAt)
            CurrentTheme.Refresh(); // same theme: just move the "current" mark
        else
            CurrentTheme = new CurrentThemeViewModel(theme, _desktop);
    }

    private void ApplyFilter() => ApplyFilter(_library.InstalledSlugs);

    private void ApplyFilter(IReadOnlySet<string> installed)
    {
        var entries = _shownEntries ?? [];
        VisibleThemes = CatalogStore.Filter(entries, SearchText, ShowDownloadedOnly, installed)
            .Select(e => _cards[e.Slug])
            .ToList();
        DefaultThemes = VisibleThemes.Where(c => c.Entry.IsDefaultTheme).ToList();
        CommunityThemes = VisibleThemes.Where(c => !c.Entry.IsDefaultTheme).ToList();

        var count = VisibleThemes.Count == entries.Count ? $"{entries.Count} themes" : $"{VisibleThemes.Count} of {entries.Count} themes";
        Subtitle = _catalog.FetchedAt is { } at ? $"{count} · updated {TimeText.Ago(at, _catalog.Now)}" : count;
    }
}
