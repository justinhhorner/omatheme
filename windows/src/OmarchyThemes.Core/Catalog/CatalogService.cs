using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;

namespace OmarchyThemes.Core.Catalog;

/// <param name="Entries">Omarchy's default themes first, then the community gallery.</param>
/// <param name="IsStale">True when omarchy.org couldn't be reached and a cached copy is shown.</param>
/// <param name="DefaultThemesError">Why the default themes are missing, if they are (the community gallery still loads).</param>
public sealed record ThemeCatalog(
    IReadOnlyList<CatalogEntry> Entries,
    DateTimeOffset FetchedAt,
    bool IsStale,
    Exception? StaleReason = null,
    Exception? DefaultThemesError = null);

public sealed class CatalogFormatException(string message) : Exception(message);

/// <summary>
/// Loads the catalog: the community gallery from omarchy.org/themes plus, when a GitHub client is
/// given, the themes that ship with Omarchy. The raw page and repo tree are cached (not the parsed
/// result), so a parser fix applies to the cached copy too and the app starts instantly offline.
/// </summary>
public sealed class CatalogService
{
    private readonly HttpCache _cache;
    private readonly Uri _pageUri;
    private readonly GitHubClient? _github;

    public CatalogService(HttpCache cache, Uri? pageUri = null, GitHubClient? github = null)
    {
        _cache = cache;
        _pageUri = pageUri ?? CatalogParser.DefaultPageUri;
        _github = github;
    }

    /// <summary>The last downloaded catalog, without any network access; null on first run.</summary>
    public ThemeCatalog? LoadCached()
    {
        var cached = _cache.TryGetCached(_pageUri);
        if (cached is null)
            return null;
        var entries = CatalogParser.Parse(cached.Text, _pageUri);
        if (entries.Count == 0)
            return null;

        var defaults = _github?.TryGetCachedTree(DefaultThemes.Repo) is { } tree ? DefaultThemes.FromTree(tree) : [];
        return new ThemeCatalog([.. defaults, .. entries], cached.FetchedAt, IsStale: false);
    }

    /// <summary>Re-fetches omarchy.org/themes (conditional GET), falling back to the cached copy if offline.</summary>
    public async Task<ThemeCatalog> RefreshAsync(CancellationToken ct = default)
    {
        var response = await _cache.GetAsync(_pageUri, new CacheOptions { ForceRevalidate = true }, ct).ConfigureAwait(false);
        var entries = CatalogParser.Parse(response.Text, _pageUri);
        if (entries.Count == 0)
            throw new CatalogFormatException(
                "No themes were found on omarchy.org/themes. The page layout may have changed; check for an app update.");

        var (defaults, defaultsError) = await LoadDefaultThemesAsync(ct).ConfigureAwait(false);
        return new ThemeCatalog([.. defaults, .. entries], response.FetchedAt, response.IsStale, response.Error, defaultsError);
    }

    /// <summary>
    /// The default themes, from Omarchy's repo tree (re-checked at most hourly, then with a free
    /// ETag revalidation). A GitHub failure with nothing cached drops them rather than the catalog.
    /// </summary>
    private async Task<(IReadOnlyList<CatalogEntry>, Exception?)> LoadDefaultThemesAsync(CancellationToken ct)
    {
        if (_github is null)
            return ([], null);
        try
        {
            var tree = await _github.GetTreeAsync(DefaultThemes.Repo, ct).ConfigureAwait(false);
            return (DefaultThemes.FromTree(tree), tree.StaleReason);
        }
        catch (Exception e) when (e is GitHubException or HttpRequestException || (e is TaskCanceledException && !ct.IsCancellationRequested))
        {
            return ([], e);
        }
    }
}
