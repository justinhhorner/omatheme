using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.Extensions.Logging;
using Microsoft.UI.Dispatching;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
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
    private readonly ILogger<GalleryViewModel> _log;
    private List<ThemeCardViewModel> _all = [];
    private DateTimeOffset? _fetchedAt;
    private bool _loaded;

    public GalleryViewModel(CatalogService catalog, ThemeStore store, ApplyService apply, NavigationService navigation, ILogger<GalleryViewModel> log)
    {
        _log = log;
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
            DefaultThemesNotice = catalog.DefaultThemesError is { } defaultsError && !catalog.Entries.Any(e => e.IsDefaultTheme)
                ? DescribeDefaultThemesError(defaultsError)
                : null;

            if (catalog.IsStale)
                _log.LogWarning(catalog.StaleReason, "Catalog refresh failed; showing the copy from {FetchedAt}", catalog.FetchedAt);
            if (catalog.DefaultThemesError is { } error)
                _log.LogError(error, "Default themes failed to load");
            _log.LogInformation("Catalog: {Default} default + {Community} community themes",
                catalog.Entries.Count(e => e.IsDefaultTheme), catalog.Entries.Count(e => !e.IsDefaultTheme));
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or CatalogFormatException)
        {
            _log.LogError(e, "Catalog refresh failed");
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

    internal static string DescribeDefaultThemesError(Exception error)
    {
        var reason = error switch
        {
            GitHubRateLimitException or GitHubNotFoundException => error.Message,
            HttpRequestException or TaskCanceledException => "GitHub couldn't be reached.",
            _ => error.Message,
        };
        return $"{reason} The themes that ship with Omarchy are listed from its GitHub repository; the community themes below still work.";
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
        DefaultThemes = VisibleThemes.Where(c => c.Entry.IsDefaultTheme).ToList();
        CommunityThemes = VisibleThemes.Where(c => !c.Entry.IsDefaultTheme).ToList();

        var count = VisibleThemes.Count == _all.Count ? $"{_all.Count} themes" : $"{VisibleThemes.Count} of {_all.Count} themes";
        Subtitle = _fetchedAt is { } at ? $"{count} · updated {Ui.Ago(at, DateTimeOffset.Now)}" : count;
    }
}
