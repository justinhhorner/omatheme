using OmarchyThemes.Core.Net;

namespace OmarchyThemes.Core.Catalog;

/// <param name="IsStale">True when omarchy.org couldn't be reached and a cached copy is shown.</param>
public sealed record ThemeCatalog(IReadOnlyList<CatalogEntry> Entries, DateTimeOffset FetchedAt, bool IsStale, Exception? StaleReason = null);

public sealed class CatalogFormatException(string message) : Exception(message);

/// <summary>
/// Loads the omarchy.org/themes catalog. The raw page is cached (not the parsed result), so a
/// parser fix applies to the cached copy too and the app starts instantly offline.
/// </summary>
public sealed class CatalogService
{
    private readonly HttpCache _cache;
    private readonly Uri _pageUri;

    public CatalogService(HttpCache cache, Uri? pageUri = null)
    {
        _cache = cache;
        _pageUri = pageUri ?? CatalogParser.DefaultPageUri;
    }

    /// <summary>The last downloaded catalog, without any network access; null on first run.</summary>
    public ThemeCatalog? LoadCached()
    {
        var cached = _cache.TryGetCached(_pageUri);
        if (cached is null)
            return null;
        var entries = CatalogParser.Parse(cached.Text, _pageUri);
        return entries.Count == 0 ? null : new ThemeCatalog(entries, cached.FetchedAt, IsStale: false);
    }

    /// <summary>Re-fetches omarchy.org/themes (conditional GET), falling back to the cached copy if offline.</summary>
    public async Task<ThemeCatalog> RefreshAsync(CancellationToken ct = default)
    {
        var response = await _cache.GetAsync(_pageUri, new CacheOptions { ForceRevalidate = true }, ct).ConfigureAwait(false);
        var entries = CatalogParser.Parse(response.Text, _pageUri);
        if (entries.Count == 0)
            throw new CatalogFormatException(
                "No themes were found on omarchy.org/themes. The page layout may have changed; check for an app update.");
        return new ThemeCatalog(entries, response.FetchedAt, response.IsStale, response.Error);
    }
}
