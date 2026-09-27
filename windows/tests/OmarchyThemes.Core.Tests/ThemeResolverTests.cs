using System.Net;
using System.Text.Json;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Tests;

public sealed class ThemeResolverTests : IDisposable
{
    private const string TreeUrl = "https://api.github.com/repos/o/r/git/trees/HEAD?recursive=1";
    private const string Raw = "https://raw.githubusercontent.com/o/r/HEAD/";
    private static readonly CatalogEntry Entry = new("r", "My Theme", "https://github.com/o/r", "https://omarchy.org/assets/themes/r.webp");

    private readonly TempDir _dir = new();
    private readonly FakeHttpHandler _http = new();
    private readonly ManualTimeProvider _time = new(DateTimeOffset.UnixEpoch);

    public void Dispose() => _dir.Dispose();

    private ThemeResolver Resolver(string? token = null) =>
        new(new GitHubClient(new HttpCache(new HttpClient(_http), _dir.Path, _time), token));

    private static string Tree(params (string Path, string Type, long? Size)[] items) =>
        JsonSerializer.Serialize(new
        {
            sha = "abc",
            truncated = false,
            tree = items.Select(i => new { path = i.Path, type = i.Type, size = i.Size }),
        });

    [Fact]
    public async Task Resolves_palette_and_naturally_sorted_wallpapers()
    {
        _http.On(TreeUrl, Tree(
                ("colors.toml", "blob", 500),
                ("alacritty.toml", "blob", 500),
                ("backgrounds", "tree", null),
                ("backgrounds/10-night.png", "blob", 3000),
                ("backgrounds/2-dusk.jpg", "blob", 2000),
                ("backgrounds/1-day.webp", "blob", 1000),
                ("backgrounds/notes.txt", "blob", 10),
                ("preview.png", "blob", 999)))
            .On(Raw + "colors.toml", Fixture.Read("colors-ansi.toml"));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.NotNull(details.Palette);
        Assert.Null(details.PaletteError);
        Assert.Equal(PaletteSource.ColorsToml, details.Palette!.Source);
        Assert.Equal(AppearanceMode.Dark, details.Mode);
        Assert.Equal(["1-day.webp", "2-dusk.jpg", "10-night.png"], details.Wallpapers.Select(w => w.FileName));
        Assert.Equal(new Uri(Raw + "backgrounds/1-day.webp"), details.Wallpapers[0].DownloadUrl);
        Assert.Equal(1000, details.Wallpapers[0].Size);
        Assert.True(details.CanApply);
        Assert.Equal(0, _http.CountFor(Raw + "alacritty.toml"));
    }

    [Fact]
    public async Task Falls_back_to_alacritty_when_colors_toml_is_unusable()
    {
        _http.On(TreeUrl, Tree(("colors.toml", "blob", 5), ("alacritty.toml", "blob", 5), ("backgrounds/a.png", "blob", 5)))
            .On(Raw + "colors.toml", "# empty")
            .On(Raw + "alacritty.toml", Fixture.Read("alacritty.toml"));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.Equal(PaletteSource.Alacritty, details.Palette!.Source);
    }

    [Fact]
    public async Task Reports_a_clear_palette_error_but_keeps_wallpapers()
    {
        _http.On(TreeUrl, Tree(("README.md", "blob", 5), ("backgrounds/a.png", "blob", 5)));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.Null(details.Palette);
        Assert.StartsWith("Couldn't read this theme's palette", details.PaletteError);
        Assert.Single(details.Wallpapers);
        Assert.True(details.CanApply);
    }

    [Fact]
    public async Task Theme_with_nothing_usable_cannot_be_applied()
    {
        _http.On(TreeUrl, Tree(("README.md", "blob", 5)));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.False(details.CanApply);
    }

    [Fact]
    public async Task Light_mode_file_marks_theme_light()
    {
        _http.On(TreeUrl, Tree(("colors.toml", "blob", 5), ("light.mode", "blob", 0)))
            .On(Raw + "colors.toml", Fixture.Read("colors-ansi.toml")); // dark background

        var details = await Resolver().ResolveAsync(Entry);

        Assert.Equal(AppearanceMode.Light, details.Mode);
        Assert.Equal(AppearanceMode.Light, details.Palette!.DeclaredMode);
    }

    [Fact]
    public async Task Uses_root_background_image_when_there_is_no_backgrounds_folder()
    {
        _http.On(TreeUrl, Tree(("background.jpg", "blob", 5), ("preview.png", "blob", 5), ("assets/wallpaper.png", "blob", 5)));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.Equal(["background.jpg"], details.Wallpapers.Select(w => w.Path));
    }

    [Fact]
    public async Task Resolves_themes_inside_a_repo_sub_folder()
    {
        var entry = Entry with { RepoUrl = "https://github.com/o/mono/tree/main/themes/foo" };
        _http.On("https://api.github.com/repos/o/mono/git/trees/main?recursive=1", Tree(
                ("themes/foo/colors.toml", "blob", 5),
                ("themes/foo/backgrounds/1.png", "blob", 5),
                ("themes/bar/backgrounds/1.png", "blob", 5)))
            .On("https://raw.githubusercontent.com/o/mono/main/themes/foo/colors.toml", Fixture.Read("colors-ansi.toml"));

        var details = await Resolver().ResolveAsync(entry);

        Assert.NotNull(details.Palette);
        Assert.Equal(
            new Uri("https://raw.githubusercontent.com/o/mono/main/themes/foo/backgrounds/1.png"),
            Assert.Single(details.Wallpapers).DownloadUrl);
    }

    [Fact]
    public async Task Rate_limit_surfaces_reset_time()
    {
        _http.On(TreeUrl, _ => FakeHttpHandler.Status(HttpStatusCode.Forbidden,
            ("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1790482111")));

        var e = await Assert.ThrowsAsync<GitHubRateLimitException>(() => Resolver().ResolveAsync(Entry));
        Assert.Equal(DateTimeOffset.FromUnixTimeSeconds(1790482111), e.ResetsAt);
    }

    [Fact]
    public async Task Rate_limited_revisit_serves_cached_tree_as_stale()
    {
        _http.On(TreeUrl, Tree(("backgrounds/a.png", "blob", 5)), etag: "\"t\"");
        await Resolver().ResolveAsync(Entry);
        _time.Advance(TimeSpan.FromHours(2));
        _http.On(TreeUrl, _ => FakeHttpHandler.Status(HttpStatusCode.TooManyRequests));

        var details = await Resolver().ResolveAsync(Entry);

        Assert.True(details.IsStale);
        Assert.IsType<GitHubRateLimitException>(details.StaleReason);
        Assert.Single(details.Wallpapers);
    }

    [Fact]
    public async Task Missing_repo_is_reported_as_not_found()
    {
        _http.On(TreeUrl, _ => FakeHttpHandler.Status(HttpStatusCode.NotFound));
        await Assert.ThrowsAsync<GitHubNotFoundException>(() => Resolver().ResolveAsync(Entry));
    }

    [Fact]
    public async Task Non_github_entries_are_rejected()
    {
        var entry = Entry with { RepoUrl = "https://gitlab.com/o/r" };
        await Assert.ThrowsAsync<ThemeResolveException>(() => Resolver().ResolveAsync(entry));
    }

    [Fact]
    public async Task Token_is_sent_to_the_api_only()
    {
        _http.On(TreeUrl, Tree(("colors.toml", "blob", 5)))
            .On(Raw + "colors.toml", Fixture.Read("colors-ansi.toml"));

        await Resolver(token: "secret").ResolveAsync(Entry);

        var api = _http.Requests.Single(r => r.RequestUri!.Host == "api.github.com");
        var raw = _http.Requests.Single(r => r.RequestUri!.Host == "raw.githubusercontent.com");
        Assert.Equal("Bearer secret", api.Headers.Authorization?.ToString());
        Assert.NotEmpty(api.Headers.UserAgent);
        Assert.Null(raw.Headers.Authorization);
    }

    [Fact]
    public void Natural_sort_orders_numbers_by_value()
    {
        string[] input = ["b10.png", "b2.png", "a.png", "b1.png", "B3.png"];
        Assert.Equal(["a.png", "b1.png", "b2.png", "B3.png", "b10.png"], input.Order(NaturalStringComparer.Instance));
    }
}
