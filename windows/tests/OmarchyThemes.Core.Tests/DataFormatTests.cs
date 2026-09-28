using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

/// <summary>
/// The saved files match docs/data-format.md, the format shared with the macOS app. The macOS tests
/// check the same fixtures (fixtures/data), so the two apps write identical files.
/// </summary>
public sealed class DataFormatTests : IDisposable
{
    private readonly TempDir _dir = new();

    public void Dispose() => _dir.Dispose();

    private static void AssertSameJson(string expected, string actual) =>
        Assert.True(JsonNode.DeepEquals(JsonNode.Parse(expected), JsonNode.Parse(actual)),
            $"Expected JSON equivalent to:\n{expected}\nActual:\n{actual}");

    private static string Write<T>(T value) => JsonSerializer.Serialize(value, JsonFile.Options);

    private static T Read<T>(string fixture) => JsonSerializer.Deserialize<T>(Fixture.Read(fixture), JsonFile.Options)!;

    private static readonly DateTimeOffset DownloadedAt = new(2026, 9, 27, 9, 0, 0, 123, TimeSpan.Zero);

    private static InstalledTheme TokyoNight() => new()
    {
        Slug = "omarchy.tokyo-night",
        Name = "Tokyo Night",
        RepoUrl = "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night",
        Palette = new Palette
        {
            Background = RgbColor.Parse("#1a1b26"),
            Foreground = RgbColor.Parse("#a9b1d6"),
            Accent = RgbColor.Parse("#7aa2f7"),
            Selection = RgbColor.Parse("#292e42"),
            Muted = RgbColor.Parse("#414868"),
            BrightForeground = RgbColor.Parse("#c0caf5"),
            DeclaredMode = AppearanceMode.Dark,
            Swatches = [new NamedColor("Red", RgbColor.Parse("#f7768e")), new NamedColor("Bright red", RgbColor.Parse("#ff7a93"))],
            Source = PaletteSource.ColorsToml,
        },
        Mode = AppearanceMode.Dark,
        Wallpapers = ["0-winding-road.webp", "1-quattro.webp"],
        ScreenshotFile = "screenshot.png",
        DownloadedAt = DownloadedAt,
        Directory = @"C:\Users\me\AppData\Local\OmarchyThemes\themes\omarchy.tokyo-night",
    };

    [Fact]
    public void Theme_manifest_is_written_in_the_shared_format()
    {
        // No nulls, no derived palette mode, no absolute screenshot path, UTC date with "Z".
        AssertSameJson(Fixture.Read("data/theme.json"), Write(TokyoNight()));
    }

    [Theory]
    [InlineData("data/theme.json")]
    [InlineData("data/legacy/windows-theme.json")]
    [InlineData("data/legacy/macos-theme.json")]
    public void Theme_manifests_read_in_every_format_rewrite_in_the_shared_one(string file)
    {
        var theme = Read<InstalledTheme>(file);

        Assert.Equal("https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night", theme.RepoUrl);
        Assert.Equal(DownloadedAt, theme.DownloadedAt);
        Assert.Null(theme.Palette!.Cursor);
        Assert.Equal(RgbColor.Parse("#c0caf5"), theme.Palette.BrightForeground);
        AssertSameJson(Fixture.Read("data/theme.json"), Write(theme));
    }

    [Fact]
    public void Settings_are_written_in_the_shared_format()
    {
        var settings = new AppSettings
        {
            WelcomeSeen = true,
            ApplyDefaults = new ApplyOptions { AppearanceMode = false, Fit = WallpaperFit.Center },
            LastAppliedSlug = "omarchy.tokyo-night",
            LastAppliedWallpaper = "1-quattro.webp",
            TerminalApp = "ghostty",
        };

        AssertSameJson(Fixture.Read("data/settings.json"), Write(settings));
        AssertSameJson(Fixture.Read("data/settings-default.json"), Write(new AppSettings()));
        AssertSameJson(Fixture.Read("data/settings.json"), Write(Read<AppSettings>("data/settings.json")));
    }

    [Fact]
    public void Older_windows_settings_still_read()
    {
        var settings = Read<AppSettings>("data/legacy/windows-settings.json");

        Assert.True(settings.WelcomeSeen);
        Assert.Equal(WallpaperFit.Center, settings.ApplyDefaults.Fit);
        Assert.Equal("omarchy.tokyo-night", settings.LastAppliedSlug);
        Assert.Null(settings.LastAppliedWallpaper);
        Assert.DoesNotContain("null", Write(settings));
    }

    [Fact]
    public void Snapshot_envelope_is_shared()
    {
        var snapshot = new DesktopSnapshot(new DateTimeOffset(2026, 9, 27, 9, 0, 0, 250, TimeSpan.Zero),
            new Dictionary<string, string> { ["screens"] = "1", ["screen.url.1"] = "/Users/me/Pictures/beach.jpg" });

        AssertSameJson(Fixture.Read("data/original-desktop.json"), Write(snapshot));
        Assert.Equal(snapshot.TakenAt, Read<DesktopSnapshot>("data/original-desktop.json").TakenAt);
    }

    [Fact]
    public async Task Http_cache_metadata_is_written_in_the_shared_format()
    {
        const string url = "https://example.com/data.json";
        var http = new FakeHttpHandler().On(url, _ =>
        {
            var response = FakeHttpHandler.Text("v1", etag: "\"a\"");
            response.Content.Headers.LastModified = new DateTimeOffset(2026, 9, 26, 11, 0, 0, TimeSpan.Zero);
            return response;
        });
        var time = new ManualTimeProvider(new DateTimeOffset(2026, 9, 26, 12, 0, 0, TimeSpan.Zero));

        await new HttpCache(new HttpClient(http), _dir.Path, time).GetAsync(new Uri(url));

        var meta = Assert.Single(Directory.GetFiles(_dir.Path, "*.json", SearchOption.AllDirectories));
        Assert.Equal(CachePath(url) + ".json", meta);
        AssertSameJson(Fixture.Read("data/http-cache-meta.json"), File.ReadAllText(meta));
    }

    [Theory]
    [InlineData("data/http-cache-meta.json")]
    [InlineData("data/legacy/windows-cache-meta.json")]
    [InlineData("data/legacy/macos-cache-meta.json")]
    public void Http_cache_metadata_reads_in_every_format(string file)
    {
        const string url = "https://example.com/data.json";
        var path = CachePath(url);
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        File.WriteAllText(path + ".json", Fixture.Read(file));
        File.WriteAllText(path + ".body", "v1");

        var cached = new HttpCache(new HttpClient(new FakeHttpHandler()), _dir.Path).TryGetCached(new Uri(url))!;

        Assert.Equal("v1", cached.Text);
        Assert.Equal("\"a\"", cached.ETag);
        Assert.Equal(new DateTimeOffset(2026, 9, 26, 11, 0, 0, TimeSpan.Zero), cached.LastModified);
        Assert.Equal(new DateTimeOffset(2026, 9, 26, 12, 0, 0, TimeSpan.Zero), cached.FetchedAt);
    }

    [Theory]
    [InlineData("\"2026-09-27T09:00:00.123Z\"")]
    [InlineData("\"2026-09-27T09:00:00.1230000+00:00\"")]
    [InlineData("\"2026-09-27T11:00:00.123+02:00\"")]
    [InlineData("\"2026-09-27T09:00:00.123\"")] // older macOS files: no zone meant UTC
    public void Dates_read_in_any_iso_form_and_write_as_utc_milliseconds(string json)
    {
        var date = JsonSerializer.Deserialize<DateTimeOffset>(json, JsonFile.Options);

        Assert.Equal(DownloadedAt, date);
        Assert.Equal("\"2026-09-27T09:00:00.123Z\"", JsonSerializer.Serialize(date, JsonFile.Options));
    }

    /// <summary>cache/&lt;xx&gt;/&lt;sha256 of the URL&gt;, as the format document specifies.</summary>
    private string CachePath(string url)
    {
        var key = Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(url)));
        return Path.Combine(_dir.Path, key[..2], key);
    }
}
