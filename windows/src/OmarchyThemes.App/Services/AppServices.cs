using System.Net;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.App.Views;

namespace OmarchyThemes.App.Services;

public static class HttpClients
{
    private const string UserAgent = "OmarchyThemes/0.1 (+https://github.com/basecamp/omarchy)";

    /// <summary>For pages and API calls: short timeout.</summary>
    public static HttpClient CreateApi() => Create(TimeSpan.FromSeconds(30));

    /// <summary>For wallpaper downloads (can be tens of MB): no overall timeout, cancelled by the user instead.</summary>
    public static HttpClient CreateDownloads() => Create(Timeout.InfiniteTimeSpan);

    private static HttpClient Create(TimeSpan timeout)
    {
        var client = new HttpClient(new SocketsHttpHandler
        {
            AutomaticDecompression = DecompressionMethods.All,
            PooledConnectionLifetime = TimeSpan.FromMinutes(5),
            ConnectTimeout = TimeSpan.FromSeconds(15),
        })
        {
            Timeout = timeout,
        };
        client.DefaultRequestHeaders.UserAgent.ParseAdd(UserAgent);
        return client;
    }
}

/// <summary>Resolved theme details, memoized for the session so revisiting a theme is instant.</summary>
public sealed class ThemeDetailsService(ThemeResolver resolver)
{
    private readonly Dictionary<string, ThemeDetails> _cache = new();

    public ThemeDetails? TryGetCached(string slug) => _cache.GetValueOrDefault(slug);

    public async Task<ThemeDetails> ResolveAsync(CatalogEntry entry, bool force = false, CancellationToken ct = default)
    {
        if (!force && _cache.TryGetValue(entry.Slug, out var cached))
            return cached;
        var details = await resolver.ResolveAsync(entry, ct);
        _cache[entry.Slug] = details;
        return details;
    }
}

/// <summary>Applies downloaded themes and remembers which one is active.</summary>
public sealed class ApplyService(ThemeApplier applier, SettingsStore settings)
{
    public event EventHandler? ActiveThemeChanged;

    public DesktopCapabilities Capabilities => applier.Capabilities;

    public bool HasOriginalSnapshot => applier.HasOriginalSnapshot;

    public string? ActiveSlug => settings.Load().LastAppliedSlug;

    public async Task<ApplySummary> ApplyAsync(InstalledTheme theme, string? wallpaperFile, ApplyOptions options)
    {
        var request = ApplyRequest.FromInstalled(theme, wallpaperFile, options);
        var result = await applier.ApplyAsync(request);
        if (result.AnyApplied)
        {
            settings.Update(s => s with
            {
                LastAppliedSlug = theme.Slug,
                LastAppliedWallpaper = request.WallpaperPath is null ? null : Path.GetFileName(request.WallpaperPath),
            });
            ActiveThemeChanged?.Invoke(this, EventArgs.Empty);
        }
        return ApplySummary.Describe(result, theme.Name, theme.Mode);
    }

    public async Task<bool> RestoreOriginalAsync()
    {
        var restored = await applier.RestoreOriginalAsync();
        if (restored)
        {
            settings.Update(s => s with { LastAppliedSlug = null, LastAppliedWallpaper = null });
            ActiveThemeChanged?.Invoke(this, EventArgs.Empty);
        }
        return restored;
    }
}

/// <summary>Navigation inside the shell's content frame, usable from view models.</summary>
public sealed class NavigationService
{
    private Frame? _frame;

    /// <summary>Raised when a top-level section (nav item tag) should be selected.</summary>
    public event EventHandler<string>? SectionRequested;

    public void Attach(Frame frame) => _frame = frame;

    public void OpenTheme(CatalogEntry entry) =>
        _frame?.Navigate(typeof(ThemeDetailPage), entry, new DrillInNavigationTransitionInfo());

    public void GoBack()
    {
        if (_frame?.CanGoBack == true)
            _frame.GoBack();
    }

    public void ShowSection(string tag) => SectionRequested?.Invoke(this, tag);
}

/// <summary>Snapshot of local storage use, for Settings.</summary>
public static class StorageInfo
{
    public static long DirectorySize(string path) =>
        Directory.Exists(path)
            ? new DirectoryInfo(path).EnumerateFiles("*", SearchOption.AllDirectories).Sum(f => f.Length)
            : 0;

    public static string? GitHubToken() =>
        Environment.GetEnvironmentVariable("OMARCHY_THEMES_GITHUB_TOKEN")
        ?? Environment.GetEnvironmentVariable("GITHUB_TOKEN");
}
