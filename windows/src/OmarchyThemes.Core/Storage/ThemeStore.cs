using System.Text.Json.Serialization;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Storage;

/// <summary>A theme saved locally. Everything needed to apply it is on disk (no network).</summary>
public sealed record InstalledTheme
{
    public required string Slug { get; init; }
    public required string Name { get; init; }
    public required string RepoUrl { get; init; }
    public Palette? Palette { get; init; }
    public AppearanceMode Mode { get; init; }

    /// <summary>Wallpaper file names inside the theme's wallpapers folder, in display order.</summary>
    public IReadOnlyList<string> Wallpapers { get; init; } = [];

    public string? ScreenshotFile { get; init; }
    public DateTimeOffset DownloadedAt { get; init; }

    /// <summary>Absolute folder of this theme; set when loaded from disk.</summary>
    [JsonIgnore]
    public string Directory { get; init; } = "";

    public string WallpaperPath(string fileName) => Path.Combine(Directory, ThemeStore.WallpapersFolder, fileName);

    [JsonIgnore]
    public string? ScreenshotPath => ScreenshotFile is null ? null : Path.Combine(Directory, ScreenshotFile);
}

public sealed record DownloadProgress(int FileIndex, int FileCount, string FileName, long BytesReceived, long? TotalBytes);

/// <summary>Reports synchronously; callers' own IProgress (e.g. UI Progress&lt;T&gt;) handles thread marshaling.</summary>
internal sealed class SyncProgress<T>(Action<T> report) : IProgress<T>
{
    public void Report(T value) => report(value);
}

/// <summary>Downloaded themes under %LOCALAPPDATA%\OmarchyThemes\themes\&lt;slug&gt;.</summary>
public sealed class ThemeStore
{
    internal const string ManifestFile = "theme.json";
    internal const string WallpapersFolder = "wallpapers";

    // Hidden working folders beside the themes (List skips names starting with a dot).
    private const string StagingPrefix = ".staging-";
    private const string PreviousPrefix = ".old-";
    private const string RemovedPrefix = ".removed-";

    private readonly AppPaths _paths;
    private readonly IDownloader _downloader;
    private readonly TimeProvider _time;

    /// <summary>Renames a folder; replaceable so tests can make a move fail.</summary>
    internal Action<string, string> MoveDirectory { get; init; } = Directory.Move;

    public ThemeStore(AppPaths paths, IDownloader downloader, TimeProvider? time = null)
    {
        _paths = paths;
        _downloader = downloader;
        _time = time ?? TimeProvider.System;
    }

    /// <summary>Raised after a theme is installed or removed.</summary>
    public event EventHandler? Changed;

    public IReadOnlyList<InstalledTheme> List()
    {
        if (!Directory.Exists(_paths.ThemesDir))
            return [];
        return Directory.EnumerateDirectories(_paths.ThemesDir)
            .Where(d => !Path.GetFileName(d).StartsWith('.'))
            .Select(Load)
            .OfType<InstalledTheme>()
            .OrderByDescending(t => t.DownloadedAt)
            .ToList();
    }

    public InstalledTheme? Get(string slug) =>
        AppPaths.IsValidSlug(slug) ? Load(_paths.ThemeDir(slug)) : null;

    public bool IsInstalled(string slug) => Get(slug) is not null;

    private static InstalledTheme? Load(string dir)
    {
        var manifest = JsonFile.TryRead<InstalledTheme>(Path.Combine(dir, ManifestFile));
        return manifest is null ? null : manifest with { Directory = dir };
    }

    /// <summary>
    /// Downloads every wallpaper (plus the screenshot) into a staging folder and swaps it in
    /// atomically, so a cancelled or failed download never leaves a half-installed theme.
    /// </summary>
    public async Task<InstalledTheme> InstallAsync(ThemeDetails details, IProgress<DownloadProgress>? progress = null, CancellationToken ct = default)
    {
        if (!details.CanApply)
            throw new InvalidOperationException($"{details.Entry.Name} has neither wallpapers nor a readable palette, so there's nothing to download.");

        var slug = details.Entry.Slug;
        var finalDir = _paths.ThemeDir(slug);
        var staging = Path.Combine(_paths.ThemesDir, $"{StagingPrefix}{slug}-{Guid.NewGuid():N}");
        Directory.CreateDirectory(Path.Combine(staging, WallpapersFolder));

        try
        {
            var wallpaperNames = new List<string>();
            var usedNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
            var fileCount = details.Wallpapers.Count + (details.Entry.ScreenshotUrl is null ? 0 : 1);

            for (var i = 0; i < details.Wallpapers.Count; i++)
            {
                var index = i;
                var wallpaper = details.Wallpapers[i];
                var name = UniqueFileName(wallpaper.FileName, usedNames);
                progress?.Report(new DownloadProgress(index, fileCount, wallpaper.FileName, 0, wallpaper.Size));
                var fileProgress = progress is null ? null : new SyncProgress<long>(bytes =>
                    progress.Report(new DownloadProgress(index, fileCount, wallpaper.FileName, bytes, wallpaper.Size)));
                await _downloader.DownloadAsync(wallpaper.DownloadUrl, Path.Combine(staging, WallpapersFolder, name), fileProgress, ct)
                    .ConfigureAwait(false);
                wallpaperNames.Add(name);
            }

            // The screenshot is a nice-to-have for offline browsing; don't fail the install over it.
            string? screenshotFile = null;
            if (details.Entry.ScreenshotUrl is { } screenshotUrl)
            {
                var ext = Path.GetExtension(new Uri(screenshotUrl).AbsolutePath);
                var candidate = "screenshot" + (string.IsNullOrEmpty(ext) ? ".webp" : ext.ToLowerInvariant());
                try
                {
                    progress?.Report(new DownloadProgress(fileCount - 1, fileCount, candidate, 0, null));
                    await _downloader.DownloadAsync(new Uri(screenshotUrl), Path.Combine(staging, candidate), null, ct).ConfigureAwait(false);
                    screenshotFile = candidate;
                }
                catch (Exception e) when (e is HttpRequestException or IOException)
                {
                }
            }

            var theme = new InstalledTheme
            {
                Slug = slug,
                Name = details.Entry.Name,
                RepoUrl = details.Entry.RepoUrl,
                Palette = details.Palette,
                Mode = details.Mode,
                Wallpapers = wallpaperNames,
                ScreenshotFile = screenshotFile,
                DownloadedAt = _time.GetUtcNow(),
            };
            JsonFile.WriteAtomic(Path.Combine(staging, ManifestFile), theme);

            ReplaceInstalled(slug, staging, finalDir);

            Changed?.Invoke(this, EventArgs.Empty);
            return theme with { Directory = finalDir };
        }
        finally
        {
            if (Directory.Exists(staging))
                TryDeleteDirectory(staging);
        }
    }

    /// <summary>
    /// Swaps a staged theme in for the installed copy. The old folder is renamed aside first and
    /// deleted only once the new one is in place, so a failure at any point leaves a complete copy.
    /// </summary>
    private void ReplaceInstalled(string slug, string staging, string finalDir)
    {
        string? previous = null;
        if (Directory.Exists(finalDir))
        {
            previous = Path.Combine(_paths.ThemesDir, $"{PreviousPrefix}{slug}-{Guid.NewGuid():N}");
            MoveDirectory(finalDir, previous);
        }

        try
        {
            MoveDirectory(staging, finalDir);
        }
        catch when (previous is not null)
        {
            // Put the old copy back. If even that fails, CleanUpStaging restores it on the next launch.
            try { MoveDirectory(previous, finalDir); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
            throw;
        }

        if (previous is not null)
            TryDeleteDirectory(previous);
    }

    /// <summary>
    /// Renames the theme aside before deleting it, so a locked file fails the removal with the theme
    /// still whole rather than half-deleted.
    /// </summary>
    public void Remove(string slug)
    {
        var dir = _paths.ThemeDir(slug);
        if (!Directory.Exists(dir))
            return;
        var removed = Path.Combine(_paths.ThemesDir, $"{RemovedPrefix}{slug}-{Guid.NewGuid():N}");
        MoveDirectory(dir, removed);
        TryDeleteDirectory(removed);
        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>
    /// Tidies up after an interrupted download, reinstall or removal (e.g. the app was killed): deletes
    /// staging and removed folders, and puts back a previous copy that a failed reinstall left aside.
    /// </summary>
    public void CleanUpStaging()
    {
        if (!Directory.Exists(_paths.ThemesDir))
            return;
        foreach (var dir in Directory.EnumerateDirectories(_paths.ThemesDir, StagingPrefix + "*")
                     .Concat(Directory.EnumerateDirectories(_paths.ThemesDir, RemovedPrefix + "*")))
            TryDeleteDirectory(dir);

        foreach (var dir in Directory.EnumerateDirectories(_paths.ThemesDir, PreviousPrefix + "*"))
        {
            var target = SlugOfPrevious(Path.GetFileName(dir)) is { } slug ? _paths.ThemeDir(slug) : null;
            if (target is null || Directory.Exists(target))
            {
                TryDeleteDirectory(dir);
                continue;
            }
            try { MoveDirectory(dir, target); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
        }
    }

    /// <summary>"tokyo" from ".old-tokyo-&lt;32 hex digits&gt;", or null if the name isn't one of ours.</summary>
    private static string? SlugOfPrevious(string folderName)
    {
        const int guidLength = 32;
        var slugLength = folderName.Length - PreviousPrefix.Length - 1 - guidLength;
        if (slugLength <= 0 || folderName[^(guidLength + 1)] != '-')
            return null;
        var slug = folderName.Substring(PreviousPrefix.Length, slugLength);
        return AppPaths.IsValidSlug(slug) ? slug : null;
    }

    private static string UniqueFileName(string fileName, HashSet<string> used)
    {
        var safe = string.Concat(fileName.Select(c => Path.GetInvalidFileNameChars().Contains(c) ? '_' : c));
        if (string.IsNullOrWhiteSpace(safe) || safe.StartsWith('.'))
            safe = "wallpaper" + safe;
        var candidate = safe;
        for (var n = 2; !used.Add(candidate); n++)
            candidate = $"{Path.GetFileNameWithoutExtension(safe)}-{n}{Path.GetExtension(safe)}";
        return candidate;
    }

    private static void TryDeleteDirectory(string dir)
    {
        try { Directory.Delete(dir, recursive: true); }
        catch (IOException) { }
        catch (UnauthorizedAccessException) { }
    }
}
