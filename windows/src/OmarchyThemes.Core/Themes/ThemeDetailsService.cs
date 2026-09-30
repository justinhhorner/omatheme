using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core.Catalog;

namespace OmarchyThemes.Core.Themes;

/// <summary>
/// Resolved theme details, memoized for the session so revisiting a theme is instant. Concurrent
/// lookups of the same theme (the theme page and a download, say) share one GitHub request.
/// </summary>
public sealed class ThemeDetailsService
{
    private readonly ThemeResolver _resolver;
    private readonly ILogger _log;
    private readonly object _gate = new();
    private readonly Dictionary<string, ThemeDetails> _cache = new();
    private readonly Dictionary<string, Task<ThemeDetails>> _inFlight = new();

    /// <summary>Bumped by <see cref="Clear"/>, so lookups already running don't refill the cache.</summary>
    private int _generation;

    public ThemeDetailsService(ThemeResolver resolver, ILogger<ThemeDetailsService>? log = null)
    {
        _resolver = resolver;
        _log = log ?? NullLogger<ThemeDetailsService>.Instance;
    }

    public ThemeDetails? TryGetCached(string slug)
    {
        lock (_gate)
            return _cache.GetValueOrDefault(slug);
    }

    /// <summary>Forgets every resolved theme (after Clear Cache), so the next lookup asks GitHub again.</summary>
    public void Clear()
    {
        lock (_gate)
        {
            _cache.Clear();
            _inFlight.Clear();
            _generation++;
        }
    }

    /// <param name="force">Look the theme up again even if it's cached (joins a lookup already running).</param>
    /// <param name="ct">Stops this caller waiting; a lookup that other callers share keeps going.</param>
    public Task<ThemeDetails> ResolveAsync(CatalogEntry entry, bool force = false, CancellationToken ct = default)
    {
        Task<ThemeDetails> lookup;
        lock (_gate)
        {
            if (!force && _cache.TryGetValue(entry.Slug, out var cached))
                return Task.FromResult(cached);
            if (!_inFlight.TryGetValue(entry.Slug, out lookup!))
            {
                lookup = ResolveSharedAsync(entry);
                // A lookup that finished synchronously has already removed itself; don't re-add it.
                if (!lookup.IsCompleted)
                    _inFlight[entry.Slug] = lookup;
            }
        }
        return lookup.WaitAsync(ct);
    }

    private async Task<ThemeDetails> ResolveSharedAsync(CatalogEntry entry)
    {
        int generation;
        lock (_gate)
            generation = _generation;
        try
        {
            var details = await _resolver.ResolveAsync(entry).ConfigureAwait(false);
            lock (_gate)
            {
                if (generation == _generation)
                    _cache[entry.Slug] = details;
            }
            if (details.PaletteError is { } paletteError)
                _log.LogWarning("{Slug}: {PaletteError}", entry.Slug, paletteError);
            if (details.IsStale)
                _log.LogWarning(details.StaleReason, "{Slug}: showing cached details", entry.Slug);
            return details;
        }
        catch (Exception e)
        {
            _log.LogError(e, "Couldn't resolve {Slug} ({Repo})", entry.Slug, entry.RepoUrl);
            throw;
        }
        finally
        {
            lock (_gate)
            {
                // After a Clear, the slug may already have a newer lookup running.
                if (generation == _generation)
                    _inFlight.Remove(entry.Slug);
            }
        }
    }
}
