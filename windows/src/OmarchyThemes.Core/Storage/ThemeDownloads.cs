using System.ComponentModel;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Storage;

public enum DownloadPhase
{
    /// <summary>Looking the theme up on GitHub.</summary>
    Resolving,
    Downloading,
    /// <summary>Saving the screenshot and manifest.</summary>
    Finishing,
}

/// <summary>Where a download is, in bytes where the sizes are known.</summary>
public sealed record DownloadStatus(DownloadPhase Phase, int FileIndex, int FileCount, long BytesDone, long TotalBytes)
{
    public static readonly DownloadStatus Starting = new(DownloadPhase.Resolving, 0, 0, 0, 0);

    /// <summary>0–100, or null when the total size isn't known yet.</summary>
    public double? Percent => TotalBytes > 0 ? Math.Min(100, 100.0 * BytesDone / TotalBytes) : null;
}

/// <summary>How a download ended.</summary>
/// <param name="Theme">The installed theme, when it succeeded.</param>
/// <param name="Error">What went wrong, when it failed.</param>
public sealed record DownloadOutcome(InstalledTheme? Theme, ThemeDetails? Details, bool Cancelled, Exception? Error)
{
    public bool Succeeded => Theme is not null;
}

/// <summary>
/// One running download. <see cref="Status"/> changes are raised on the thread (synchronization
/// context) that started the download, so a UI can bind to it directly.
/// </summary>
public sealed class DownloadOperation : INotifyPropertyChanged
{
    private readonly CancellationTokenSource _cts = new();
    private readonly SynchronizationContext? _context = SynchronizationContext.Current;
    private DownloadStatus _status = DownloadStatus.Starting;

    internal DownloadOperation(string slug) => Slug = slug;

    public event PropertyChangedEventHandler? PropertyChanged;

    public string Slug { get; }

    public DownloadStatus Status => _status;

    public Task<DownloadOutcome> Completion { get; internal set; } = null!;

    internal CancellationToken Token => _cts.Token;

    public void Cancel() => _cts.Cancel();

    internal void Report(DownloadStatus status)
    {
        _status = status;
        if (_context is null || _context == SynchronizationContext.Current)
            PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Status)));
        else
            _context.Post(_ => PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(nameof(Status))), null);
    }
}

/// <summary>
/// Owns theme downloads, keyed by slug, so they outlive the page that started them: leaving the
/// theme page doesn't cancel or orphan a download, coming back can show its live progress, and
/// starting it again joins the running one instead of racing it on the same folder.
/// </summary>
public sealed class ThemeDownloads
{
    private readonly ThemeDetailsService _details;
    private readonly ThemeStore _store;
    private readonly ILogger _log;
    private readonly object _gate = new();
    private readonly Dictionary<string, DownloadOperation> _running = new();

    public ThemeDownloads(ThemeDetailsService details, ThemeStore store, ILogger<ThemeDownloads>? log = null)
    {
        _details = details;
        _store = store;
        _log = log ?? NullLogger<ThemeDownloads>.Instance;
    }

    public DownloadOperation? Get(string slug)
    {
        lock (_gate)
            return _running.GetValueOrDefault(slug);
    }

    /// <summary>Starts downloading <paramref name="entry"/>, or returns its download if one is running.</summary>
    /// <param name="refresh">Re-resolve the theme first, so a re-download picks up changes to the repo.</param>
    public DownloadOperation Start(CatalogEntry entry, bool refresh = false)
    {
        lock (_gate)
        {
            if (_running.TryGetValue(entry.Slug, out var existing))
                return existing;

            var operation = new DownloadOperation(entry.Slug);
            _running[entry.Slug] = operation;
            // Run off the caller's thread; progress is marshalled back by DownloadOperation.
            operation.Completion = Task.Run(() => RunAsync(entry, refresh, operation));
            return operation;
        }
    }

    private async Task<DownloadOutcome> RunAsync(CatalogEntry entry, bool refresh, DownloadOperation operation)
    {
        try
        {
            var theme = await _details.ResolveAsync(entry, force: refresh, operation.Token).ConfigureAwait(false);
            var count = theme.Wallpapers.Count;
            var sizes = theme.Wallpapers.Select(w => w.Size ?? 0).ToArray();
            var totalBytes = sizes.Sum();

            // Wallpapers report bytes; anything after them (the screenshot, the manifest) is "finishing".
            DownloadStatus Finishing(int fileIndex) => new(DownloadPhase.Finishing, fileIndex, count, totalBytes, totalBytes);
            var progress = new SyncProgress<DownloadProgress>(p => operation.Report(p.FileIndex < count
                ? new DownloadStatus(DownloadPhase.Downloading, p.FileIndex, count, sizes.Take(p.FileIndex).Sum() + p.BytesReceived, totalBytes)
                : Finishing(p.FileIndex)));

            var installed = await _store.InstallAsync(theme, progress, operation.Token).ConfigureAwait(false);
            operation.Report(Finishing(count));
            _log.LogInformation("Downloaded {Slug}: {Count} wallpapers", entry.Slug, installed.Wallpapers.Count);
            return new DownloadOutcome(installed, theme, Cancelled: false, Error: null);
        }
        catch (OperationCanceledException) when (operation.Token.IsCancellationRequested)
        {
            _log.LogInformation("Download of {Slug} cancelled", entry.Slug);
            return new DownloadOutcome(null, null, Cancelled: true, Error: null);
        }
        catch (Exception e) when (e is HttpRequestException or TaskCanceledException or IOException or UnauthorizedAccessException
                                      or GitHubException or ThemeResolveException or InvalidOperationException)
        {
            _log.LogError(e, "Download of {Slug} failed", entry.Slug);
            return new DownloadOutcome(null, null, Cancelled: false, Error: e);
        }
        finally
        {
            lock (_gate)
                _running.Remove(entry.Slug);
        }
    }
}
