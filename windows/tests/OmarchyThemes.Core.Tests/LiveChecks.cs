using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Tests;

/// <summary>
/// Read-only checks against the real services, off by default (see <see cref="LiveFactAttribute"/>).
/// They catch what fixtures can't: markup changes on omarchy.org, repo layout drift, and real
/// download behaviour. About 3 GitHub API calls per run; file bytes come from raw.githubusercontent.com.
/// </summary>
[Trait("Category", "Live")]
public sealed class LiveChecks : IDisposable
{
    private readonly TempDir _dir = new();
    private readonly HttpClient _http = new() { Timeout = TimeSpan.FromSeconds(60) };
    private readonly HttpCache _cache;
    private readonly GitHubClient _github;

    public LiveChecks()
    {
        _http.DefaultRequestHeaders.UserAgent.ParseAdd("OmarchyThemes-LiveChecks/1.0");
        _cache = new HttpCache(_http, Path.Combine(_dir.Path, "cache"));
        _github = new GitHubClient(_cache, Environment.GetEnvironmentVariable("GITHUB_TOKEN"));
    }

    public void Dispose()
    {
        _http.Dispose();
        _dir.Dispose();
    }

    [LiveFact]
    public async Task Catalog_lists_default_and_community_themes()
    {
        var catalog = await new CatalogService(_cache, github: _github).RefreshAsync();

        Assert.False(catalog.IsStale);
        Assert.Null(catalog.DefaultThemesError);
        var defaults = catalog.Entries.Where(e => e.IsDefaultTheme).ToList();
        var community = catalog.Entries.Where(e => !e.IsDefaultTheme).ToList();
        Assert.InRange(defaults.Count, 10, 100);
        Assert.InRange(community.Count, 100, 1000);
        Assert.Contains(defaults, e => e.Slug == "omarchy.tokyo-night" && e.Name == "Tokyo Night");
        Assert.All(community, e => Assert.True(RepoRef.TryParse(e.RepoUrl, out _), e.RepoUrl));
        Assert.All(community, e => Assert.NotNull(e.ScreenshotUrl));
        Assert.Equal(catalog.Entries.Count, catalog.Entries.Select(e => e.Slug).Distinct().Count());
    }

    [LiveFact]
    public async Task Resolves_a_default_theme_and_two_community_themes()
    {
        var catalog = await new CatalogService(_cache, github: _github).RefreshAsync();
        var resolver = new ThemeResolver(_github);

        // A default theme: named colors.toml with a mode, from Omarchy's repo (tree already cached).
        var tokyo = await resolver.ResolveAsync(catalog.Entries.Single(e => e.Slug == "omarchy.tokyo-night"));
        Assert.Equal(PaletteSource.ColorsToml, tokyo.Palette!.Source);
        Assert.Equal(AppearanceMode.Dark, tokyo.Mode);
        Assert.NotEmpty(tokyo.Wallpapers);

        // Community themes in the two colors.toml shapes; Vulkanite's wallpapers are WebP.
        var aetheria = await resolver.ResolveAsync(catalog.Entries.Single(e => e.Slug == "aetheria"));
        Assert.Equal(16, aetheria.Palette!.Swatches.Count);
        Assert.NotEmpty(aetheria.Wallpapers);

        var vulkanite = await resolver.ResolveAsync(catalog.Entries.Single(e => e.Slug == "vulkanite"));
        Assert.NotNull(vulkanite.Palette);
        Assert.Contains(vulkanite.Wallpapers, w => w.FileName.EndsWith(".webp", StringComparison.OrdinalIgnoreCase));
    }

    [LiveFact]
    public async Task Downloads_a_wallpaper_reporting_progress_in_bytes()
    {
        // Tokyo Night's first wallpaper, via Omarchy's (cached) tree: no extra API call.
        var tree = await _github.GetTreeAsync(DefaultThemes.Repo);
        var wallpaper = tree.Files.First(f => f.Path.StartsWith("themes/tokyo-night/backgrounds/", StringComparison.Ordinal));
        var target = Path.Combine(_dir.Path, wallpaper.FileName);
        var reports = new List<long>();

        await new HttpDownloader(_http).DownloadAsync(
            GitHubClient.RawUrl(DefaultThemes.Repo, wallpaper.Path), target, new Collector(reports), CancellationToken.None);

        var length = new FileInfo(target).Length;
        Assert.Equal(wallpaper.Size, length);
        Assert.NotEmpty(reports);
        Assert.Equal(reports.Order(), reports);  // monotonic
        Assert.Equal(length, reports[^1]);       // ends at the file size, in bytes (not a percentage)
    }

    private sealed class Collector(List<long> into) : IProgress<long>
    {
        public void Report(long value)
        {
            lock (into)
                into.Add(value);
        }
    }
}
