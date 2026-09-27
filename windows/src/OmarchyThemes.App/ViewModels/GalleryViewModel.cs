using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Dispatching;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.App.ViewModels;

/// <summary>
/// The theme catalog. Shows the cached copy instantly, then refreshes from omarchy.org in the
/// background when the cache is old. Refresh always re-fetches.
/// </summary>
public sealed partial class GalleryViewModel : ObservableObject
{
    private static readonly TimeSpan AutoRefreshAfter = TimeSpan.FromHours(12);

    private readonly CatalogService _catalog;
    private readonly ThemeStore _store;
    private readonly ApplyService _apply;
    private readonly NavigationService _navigation;
    private readonly DispatcherQueue _dispatcher;
    private List<ThemeCardViewModel> _all = [];
    private DateTimeOffset? _fetchedAt;
    private bool _loaded;

    public GalleryViewModel(CatalogService catalog, ThemeStore store, ApplyService apply, NavigationService navigation)
    {
        _catalog = catalog;
        _store = store;
        _apply = apply;
        _navigation = navigation;
        _dispatcher = DispatcherQueue.GetForCurrentThread();

        _store.Changed += (_, _) => _dispatcher.TryEnqueue(UpdateBadges);
        _apply.ActiveThemeChanged += (_, _) => _dispatcher.TryEnqueue(UpdateBadges);
    }

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(IsEmptyResult), nameof(ResultText))]
    public partial IReadOnlyList<ThemeCardViewModel> VisibleThemes { get; set; } = [];

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

    partial void OnSearchTextChanged(string value) => ApplyFilter();

    partial void OnShowDownloadedOnlyChanged(bool value) => ApplyFilter();

    public async Task EnsureLoadedAsync()
    {
        if (_loaded)
            return;
        _loaded = true;

        if (_catalog.LoadCached() is { } cached)
        {
            Show(cached);
            // Refresh when the cache is old, or has no default themes yet (e.g. they failed to load).
            if (cached.Entries.Any(e => e.IsDefaultTheme) && DateTimeOffset.Now - cached.FetchedAt < AutoRefreshAfter)
                return;
        }
        else
        {
            IsLoading = true;
        }
        await RefreshAsync();
    }

    private bool CanRefresh() => !IsRefreshing;

    [RelayCommand(CanExecute = nameof(CanRefresh))]
    private async Task RefreshAsync()
    {
        IsRefreshing = true;
        ErrorMessage = null;
        try
        {
            var catalog = await _catalog.RefreshAsync();
            Show(catalog);
            Notice = catalog.IsStale
                ? $"Couldn't reach omarchy.org, so this is the catalog from {Ui.Ago(catalog.FetchedAt, DateTimeOffset.Now)}."
                : null;
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or CatalogFormatException)
        {
            var message = e is CatalogFormatException
                ? e.Message
                : "Couldn't load themes from omarchy.org. Check your internet connection and try again.";
            if (HasThemes)
                Notice = message;
            else
                ErrorMessage = message;
        }
        finally
        {
            IsRefreshing = false;
            IsLoading = false;
        }
    }

    public void Open(ThemeCardViewModel card) => _navigation.OpenTheme(card.Entry);

    public CatalogEntry? Find(string slug) => _all.FirstOrDefault(c => c.Entry.Slug == slug)?.Entry;

    private void Show(ThemeCatalog catalog)
    {
        var installed = _store.List().ToDictionary(t => t.Slug);
        _all = catalog.Entries
            .Select(e => new ThemeCardViewModel(e, installed.GetValueOrDefault(e.Slug)?.ScreenshotPath))
            .ToList();
        _fetchedAt = catalog.FetchedAt;
        HasThemes = _all.Count > 0;
        UpdateBadges();
        ApplyFilter();
    }

    private void UpdateBadges()
    {
        var installed = _store.List().Select(t => t.Slug).ToHashSet();
        var active = _apply.ActiveSlug;
        foreach (var card in _all)
        {
            card.IsDownloaded = installed.Contains(card.Entry.Slug);
            card.IsActive = card.Entry.Slug == active;
        }
        if (ShowDownloadedOnly)
            ApplyFilter();
    }

    private void ApplyFilter()
    {
        var query = SearchText.Trim();
        VisibleThemes = _all
            .Where(c => query.Length == 0
                || c.Name.Contains(query, StringComparison.CurrentCultureIgnoreCase)
                || c.RepoDisplay.Contains(query, StringComparison.OrdinalIgnoreCase))
            .Where(c => !ShowDownloadedOnly || c.IsDownloaded)
            .ToList();

        var count = VisibleThemes.Count == _all.Count ? $"{_all.Count} themes" : $"{VisibleThemes.Count} of {_all.Count} themes";
        Subtitle = _fetchedAt is { } at ? $"{count} · updated {Ui.Ago(at, DateTimeOffset.Now)}" : count;
    }
}
