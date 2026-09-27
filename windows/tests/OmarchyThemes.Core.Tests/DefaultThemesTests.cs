using System.Net;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Tests;

public sealed class DefaultThemesTests : IDisposable
{
    private const string TreeUrl = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1";
    private const string Raw = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/";
    private static readonly string PageUrl = CatalogParser.DefaultPageUri.AbsoluteUri;

    private readonly TempDir _dir = new();
    private readonly FakeHttpHandler _http = new();
    private readonly ManualTimeProvider _time = new(new DateTimeOffset(2026, 9, 27, 0, 0, 0, TimeSpan.Zero));
    private readonly HttpCache _cache;
    private readonly GitHubClient _github;

    public DefaultThemesTests()
    {
        _cache = new HttpCache(new HttpClient(_http), _dir.Path, _time);
        _github = new GitHubClient(_cache);
    }

    public void Dispose() => _dir.Dispose();

    private CatalogService Catalog() => new(_cache, github: _github);

    [Fact]
    public async Task Lists_each_folder_under_themes_with_omarchys_naming()
    {
        _http.On(TreeUrl, Fixture.Read("omarchy-tree.json"));

        var entries = DefaultThemes.FromTree(await _github.GetTreeAsync(DefaultThemes.Repo));

        Assert.Equal(["omarchy.catppuccin-latte", "omarchy.retro-82", "omarchy.tokyo-night"], entries.Select(e => e.Slug));
        Assert.Equal(["Catppuccin Latte", "Retro 82", "Tokyo Night"], entries.Select(e => e.Name));

        var tokyo = entries[2];
        Assert.Equal("https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night", tokyo.RepoUrl);
        Assert.Equal(Raw + "tokyo-night/preview.png", tokyo.ScreenshotUrl);
        Assert.True(tokyo.IsDefaultTheme);
        Assert.Equal("Included with Omarchy", tokyo.RepoDisplay);

        // No preview.png in the folder: no screenshot rather than a broken link.
        Assert.Null(entries[1].ScreenshotUrl);
    }

    [Theory]
    [InlineData("tokyo-night", "Tokyo Night")]
    [InlineData("retro-82", "Retro 82")]
    [InlineData("white", "White")]
    [InlineData("flexoki-light", "Flexoki Light")]
    public void Display_names_follow_omarchy_theme_list(string folder, string name)
    {
        Assert.Equal(name, DefaultThemes.DisplayName(folder));
    }

    [Fact]
    public void Community_entries_are_not_default_themes()
    {
        var entry = new CatalogEntry("aetheria", "Aetheria", "https://github.com/JJDizz1L/aetheria", null);

        Assert.False(entry.IsDefaultTheme);
        Assert.Equal("JJDizz1L/aetheria", entry.RepoDisplay);
    }

    [Fact]
    public async Task Catalog_lists_default_themes_first_then_the_community_gallery()
    {
        _http.On(PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(TreeUrl, Fixture.Read("omarchy-tree.json"));

        var catalog = await Catalog().RefreshAsync();

        Assert.Equal(7, catalog.Entries.Count);
        Assert.All(catalog.Entries.Take(3), e => Assert.True(e.IsDefaultTheme));
        Assert.Equal("aetheria", catalog.Entries[3].Slug);
        Assert.Null(catalog.DefaultThemesError);
        Assert.Equal(catalog.Entries.Count, catalog.Entries.Select(e => e.Slug).Distinct().Count());
    }

    [Fact]
    public async Task Catalog_still_loads_when_github_fails_and_nothing_is_cached()
    {
        _http.On(PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(TreeUrl, _ => FakeHttpHandler.Status(HttpStatusCode.Forbidden, ("x-ratelimit-remaining", "0")));

        var catalog = await Catalog().RefreshAsync();

        Assert.Equal(4, catalog.Entries.Count);
        Assert.IsType<GitHubRateLimitException>(catalog.DefaultThemesError);
        Assert.False(catalog.IsStale);
    }

    [Fact]
    public async Task Default_themes_load_from_cache_offline()
    {
        _http.On(PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(TreeUrl, Fixture.Read("omarchy-tree.json"), etag: "\"tree\"");
        await Catalog().RefreshAsync();

        Assert.Equal(7, Catalog().LoadCached()!.Entries.Count);

        _time.Advance(TimeSpan.FromHours(2));
        _http.On(PageUrl, _ => FakeHttpHandler.Throw())
            .On(TreeUrl, _ => FakeHttpHandler.Throw());
        var offline = await Catalog().RefreshAsync();

        Assert.True(offline.IsStale);
        Assert.Equal(7, offline.Entries.Count);
        Assert.IsType<HttpRequestException>(offline.DefaultThemesError);
    }

    [Fact]
    public async Task Catalog_without_a_github_client_is_community_only()
    {
        _http.On(PageUrl, Fixture.Read("catalog-live-structure.html"));

        var catalog = await new CatalogService(_cache).RefreshAsync();

        Assert.Equal(4, catalog.Entries.Count);
        Assert.Equal(0, _http.CountFor(TreeUrl));
    }

    [Fact]
    public async Task Resolving_a_default_theme_reuses_the_cached_tree()
    {
        _http.On(PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(TreeUrl, Fixture.Read("omarchy-tree.json"))
            .On(Raw + "tokyo-night/colors.toml", Fixture.Read("colors-named.toml"));
        var tokyo = (await Catalog().RefreshAsync()).Entries.Single(e => e.Slug == "omarchy.tokyo-night");

        var details = await new ThemeResolver(_github).ResolveAsync(tokyo);

        Assert.Equal(1, _http.CountFor(TreeUrl));
        Assert.Equal(PaletteSource.ColorsToml, details.Palette!.Source);
        Assert.Equal(AppearanceMode.Light, details.Mode);
        // backgrounds/ only: not preview.png or unlock.png, naturally sorted.
        Assert.Equal(["0-winding-road.webp", "2-swirl-buck.webp", "10-oma.webp"], details.Wallpapers.Select(w => w.FileName));
        Assert.Equal(new Uri(Raw + "tokyo-night/backgrounds/0-winding-road.webp"), details.Wallpapers[0].DownloadUrl);
    }

    [Fact]
    public void Default_theme_slugs_are_valid_folder_names()
    {
        Assert.EndsWith("omarchy.tokyo-night", _dir.AppPaths.ThemeDir("omarchy.tokyo-night"));
    }
}
