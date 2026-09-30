using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Stores;

/// <summary>
/// The gallery's catalog: Omarchy's default themes and the community gallery. Shows the cached copy
/// straight away, refreshes it in the background when it's old, and says what went wrong when
/// omarchy.org or GitHub can't be reached.
/// </summary>
public sealed class CatalogStore
{
    /// <summary>The catalog is refreshed in the background when the cached copy is older than this.</summary>
    public static readonly TimeSpan AutoRefreshAfter = TimeSpan.FromHours(12);

    private readonly CatalogService _service;
    private readonly TimeProvider _time;
    private readonly ILogger _log;
    private bool _loaded;
    private Task? _refresh;

    public CatalogStore(CatalogService service, TimeProvider? time = null, ILogger<CatalogStore>? log = null)
    {
        _service = service;
        _time = time ?? TimeProvider.System;
        _log = log ?? NullLogger<CatalogStore>.Instance;
    }

    /// <summary>Raised after any property changes, on the thread that called into the store.</summary>
    public event EventHandler? Changed;

    public IReadOnlyList<CatalogEntry> Entries { get; private set; } = [];

    public DateTimeOffset? FetchedAt { get; private set; }

    /// <summary>First load with nothing cached.</summary>
    public bool IsLoading { get; private set; }

    public bool IsRefreshing { get; private set; }

    /// <summary>Shown in place of the gallery when there is no catalog at all.</summary>
    public string? Error { get; private set; }

    /// <summary>Shown above the gallery, e.g. "showing the cached catalog".</summary>
    public string? Notice { get; private set; }

    /// <summary>Shown when Omarchy's own themes couldn't be listed (the community gallery still loads).</summary>
    public string? DefaultThemesNotice { get; private set; }

    public DateTimeOffset Now => _time.GetUtcNow();

    /// <summary>
    /// Shows the cached catalog straight away, then refreshes it if it's old or has no default themes
    /// yet (e.g. they failed to load last time). Only the first call does anything.
    /// </summary>
    public Task LoadIfNeededAsync()
    {
        if (_loaded)
            return Task.CompletedTask;
        _loaded = true;

        if (_service.LoadCached() is { } cached)
        {
            Show(cached);
            var hasDefaultThemes = cached.Entries.Any(e => e.IsDefaultTheme);
            if (hasDefaultThemes && Now - cached.FetchedAt < AutoRefreshAfter)
            {
                OnChanged();
                return Task.CompletedTask;
            }
        }
        else
        {
            IsLoading = true;
        }
        OnChanged();
        return RefreshAsync();
    }

    /// <summary>Re-fetches the catalog. Callers that ask while a refresh is running share it.</summary>
    public Task RefreshAsync()
    {
        if (_refresh is { IsCompleted: false })
            return _refresh;
        return _refresh = PerformRefreshAsync();
    }

    public CatalogEntry? Find(string slug) => Entries.FirstOrDefault(e => e.Slug == slug);

    /// <summary>The catalog entry for a downloaded theme, or one rebuilt from its manifest.</summary>
    public CatalogEntry EntryFor(InstalledTheme theme) =>
        Find(theme.Slug) ?? new CatalogEntry(theme.Slug, theme.Name, theme.RepoUrl, ScreenshotUrl: null);

    /// <summary>
    /// The entries matching a search (by name or repo) and the "downloaded only" switch, in catalog
    /// order.
    /// </summary>
    public static IReadOnlyList<CatalogEntry> Filter(
        IEnumerable<CatalogEntry> entries, string search, bool downloadedOnly, IReadOnlySet<string> installedSlugs)
    {
        var query = search.Trim();
        return entries
            .Where(e => query.Length == 0
                || e.Name.Contains(query, StringComparison.CurrentCultureIgnoreCase)
                || e.RepoDisplay.Contains(query, StringComparison.OrdinalIgnoreCase))
            .Where(e => !downloadedOnly || installedSlugs.Contains(e.Slug))
            .ToList();
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

    private async Task PerformRefreshAsync()
    {
        IsRefreshing = true;
        Error = null;
        OnChanged();
        try
        {
            var catalog = await _service.RefreshAsync();
            Show(catalog);
            Notice = catalog.IsStale
                ? $"Couldn't reach omarchy.org, so this is the catalog from {TimeText.Ago(catalog.FetchedAt, Now)}."
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
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            _log.LogError(e, "Catalog refresh failed");
            var message = e is HttpRequestException or TaskCanceledException
                ? "Couldn't load themes from omarchy.org. Check your internet connection and try again."
                : e.Message;
            if (Entries.Count > 0)
                Notice = message;
            else
                Error = message;
        }
        finally
        {
            IsRefreshing = false;
            IsLoading = false;
            OnChanged();
        }
    }

    private void Show(ThemeCatalog catalog)
    {
        Entries = catalog.Entries;
        FetchedAt = catalog.FetchedAt;
    }

    private void OnChanged() => Changed?.Invoke(this, EventArgs.Empty);
}
