using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores;

/// <summary>Downloaded themes, theme lookups (GitHub) and downloads in progress.</summary>
public sealed class ThemeLibrary
{
    private readonly ThemeStore _store;
    private readonly ThemeDetailsService _details;
    private readonly ThemeDownloads _downloads;
    private readonly AppPaths _paths;
    private readonly ILogger _log;
    private IReadOnlyList<InstalledTheme> _installed;

    public ThemeLibrary(
        ThemeStore store, ThemeDetailsService details, ThemeDownloads downloads, AppPaths paths, ILogger<ThemeLibrary>? log = null)
    {
        _store = store;
        _details = details;
        _downloads = downloads;
        _paths = paths;
        _log = log ?? NullLogger<ThemeLibrary>.Instance;
        store.CleanUpStaging();
        _installed = store.List();

        _store.Changed += (_, _) =>
        {
            Reload();
            Changed?.Invoke(this, EventArgs.Empty);
        };
    }

    /// <summary>
    /// Raised after a theme is installed or removed, on the thread that did it (a download finishes
    /// off the UI thread).
    /// </summary>
    public event EventHandler? Changed;

    /// <summary>Downloaded themes, most recent first.</summary>
    public IReadOnlyList<InstalledTheme> Installed => Volatile.Read(ref _installed);

    public IReadOnlySet<string> InstalledSlugs => Installed.Select(t => t.Slug).ToHashSet();

    public InstalledTheme? Get(string slug) => Installed.FirstOrDefault(t => t.Slug == slug);

    /// <summary>Reads the downloaded themes from disk again.</summary>
    public void Reload() => Volatile.Write(ref _installed, _store.List());

    // Lookups

    public ThemeDetails? CachedDetails(string slug) => _details.TryGetCached(slug);

    /// <summary>Resolves a theme from GitHub, memoized for the session.</summary>
    public Task<ThemeDetails> ResolveAsync(CatalogEntry entry, CancellationToken ct = default) =>
        _details.ResolveAsync(entry, ct: ct);

    // Downloads

    /// <summary>The theme's download, if one is running (it may have been started from another page).</summary>
    public DownloadOperation? RunningDownload(string slug) => _downloads.Get(slug);

    /// <summary>
    /// Downloads the theme, or joins its download if one is running. A theme that's already downloaded
    /// is looked up again first, so updates to it are picked up.
    /// </summary>
    public DownloadOperation Download(CatalogEntry entry) =>
        _downloads.Start(entry, refresh: Get(entry.Slug) is not null);

    /// <summary>What to tell the user about a download that didn't install the theme.</summary>
    public static Banner DescribeFailedDownload(DownloadOutcome outcome) => outcome.Cancelled
        ? new Banner(SummaryKind.Info, "Download cancelled", "Nothing was saved.")
        : new Banner(SummaryKind.Error, "Download failed",
            outcome.Error is { } error ? ExpectedErrors.Describe(error) : "Something went wrong.");

    // Removing and clearing

    public Banner Remove(InstalledTheme theme)
    {
        try
        {
            _store.Remove(theme.Slug);
            return new Banner(SummaryKind.Info, "Download removed", $"{theme.Name} was removed from this PC.");
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            _log.LogError(e, "Removing {Slug} failed", theme.Slug);
            return new Banner(SummaryKind.Error, $"Couldn't remove {theme.Name}", e.Message);
        }
    }

    /// <summary>
    /// Deletes the HTTP cache (catalog, GitHub responses) and forgets resolved themes. Downloaded themes
    /// are kept.
    /// </summary>
    public Banner ClearCache()
    {
        try
        {
            if (Directory.Exists(_paths.CacheDir))
                Directory.Delete(_paths.CacheDir, recursive: true);
            Directory.CreateDirectory(_paths.CacheDir);
            return new Banner(SummaryKind.Success, "Cache cleared",
                "The catalog and theme details will be downloaded again when needed. Downloaded themes were kept.");
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            _log.LogError(e, "Clearing the cache failed");
            return new Banner(SummaryKind.Error, "Couldn't clear the cache", e.Message);
        }
        finally
        {
            // Even a partly deleted cache must be read again rather than served from memory.
            _details.Clear();
        }
    }
}
