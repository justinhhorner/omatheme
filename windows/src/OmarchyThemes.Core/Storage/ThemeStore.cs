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

    private readonly AppPaths _paths;
    private readonly IDownloader _downloader;
    private readonly TimeProvider _time;

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
        var staging = Path.Combine(_paths.ThemesDir, $".staging-{slug}-{Guid.NewGuid():N}");
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

            if (Directory.Exists(finalDir))
                Directory.Delete(finalDir, recursive: true);
            Directory.Move(staging, finalDir);

            Changed?.Invoke(this, EventArgs.Empty);
            return theme with { Directory = finalDir };
        }
        finally
        {
            if (Directory.Exists(staging))
                TryDeleteDirectory(staging);
        }
    }

    public void Remove(string slug)
    {
        var dir = _paths.ThemeDir(slug);
        if (!Directory.Exists(dir))
            return;
        Directory.Delete(dir, recursive: true);
        Changed?.Invoke(this, EventArgs.Empty);
    }

    /// <summary>Deletes leftovers from interrupted downloads (e.g. the app was killed mid-download).</summary>
    public void CleanUpStaging()
    {
        if (!Directory.Exists(_paths.ThemesDir))
            return;
        foreach (var dir in Directory.EnumerateDirectories(_paths.ThemesDir, ".staging-*"))
            TryDeleteDirectory(dir);
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
