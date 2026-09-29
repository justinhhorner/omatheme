using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

public sealed class ThemeStoreTests : IDisposable
{
    private readonly TempDir _dir = new();
    private readonly FakeDownloader _downloader = new();
    private readonly ManualTimeProvider _time = new(new DateTimeOffset(2026, 9, 26, 0, 0, 0, TimeSpan.Zero));
    private readonly ThemeStore _store;

    public ThemeStoreTests() => _store = new ThemeStore(_dir.AppPaths, _downloader, _time);

    public void Dispose() => _dir.Dispose();

    internal static ThemeDetails Details(string slug = "tokyo", params string[] wallpapers)
    {
        var repo = new RepoRef("o", slug);
        return new ThemeDetails(
            new CatalogEntry(slug, "Tokyo", $"https://github.com/o/{slug}", $"https://omarchy.org/assets/themes/{slug}.webp"),
            repo,
            new Palette
            {
                Background = RgbColor.Parse("#1a1b26"),
                Foreground = RgbColor.Parse("#a9b1d6"),
                Accent = RgbColor.Parse("#7aa2f7"),
                Source = PaletteSource.ColorsToml,
            },
            PaletteError: null,
            wallpapers.Select(w => new WallpaperRef($"backgrounds/{w}", 100, GitHubClient.RawUrl(repo, $"backgrounds/{w}"))).ToList(),
            AppearanceMode.Dark);
    }

    [Fact]
    public async Task Install_downloads_wallpapers_and_screenshot_and_writes_manifest()
    {
        var reports = new List<DownloadProgress>();

        var installed = await _store.InstallAsync(Details("tokyo", "1.png", "2.jpg"), new ListProgress<DownloadProgress>(reports));

        Assert.Equal(["1.png", "2.jpg"], installed.Wallpapers);
        Assert.True(File.Exists(installed.WallpaperPath("1.png")));
        Assert.True(File.Exists(installed.ScreenshotPath));
        Assert.Equal(_time.Now, installed.DownloadedAt);

        var reloaded = _store.Get("tokyo")!;
        Assert.Equal(installed.Directory, reloaded.Directory);
        Assert.Equal(("tokyo", "Tokyo", "https://github.com/o/tokyo", AppearanceMode.Dark, _time.Now),
            (reloaded.Slug, reloaded.Name, reloaded.RepoUrl, reloaded.Mode, reloaded.DownloadedAt));
        Assert.Equal(installed.Wallpapers, reloaded.Wallpapers);
        Assert.Equal(installed.ScreenshotFile, reloaded.ScreenshotFile);
        Assert.Equal(RgbColor.Parse("#7aa2f7"), reloaded.Palette!.Accent);
        Assert.Single(_store.List());
        Assert.Contains(reports, r => r.FileIndex == 1 && r.FileCount == 3 && r.FileName == "2.jpg");
    }

    [Fact]
    public async Task Failed_download_leaves_no_partial_theme_behind()
    {
        var details = Details("tokyo", "1.png", "2.png");
        _downloader.FailUrls.Add(details.Wallpapers[1].DownloadUrl.AbsoluteUri);

        await Assert.ThrowsAsync<HttpRequestException>(() => _store.InstallAsync(details));

        Assert.Null(_store.Get("tokyo"));
        Assert.Empty(Directory.EnumerateFileSystemEntries(_dir.AppPaths.ThemesDir));
    }

    [Fact]
    public async Task Cancelled_download_leaves_no_partial_theme_behind()
    {
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => _store.InstallAsync(Details("tokyo", "1.png"), ct: cts.Token));

        Assert.Empty(Directory.EnumerateFileSystemEntries(_dir.AppPaths.ThemesDir));
    }

    [Fact]
    public async Task Screenshot_failure_does_not_fail_install()
    {
        var details = Details("tokyo", "1.png");
        _downloader.FailUrls.Add(details.Entry.ScreenshotUrl!);

        var installed = await _store.InstallAsync(details);

        Assert.Null(installed.ScreenshotFile);
        Assert.NotNull(_store.Get("tokyo"));
    }

    [Fact]
    public async Task Reinstall_replaces_the_previous_copy()
    {
        await _store.InstallAsync(Details("tokyo", "old.png"));
        var installed = await _store.InstallAsync(Details("tokyo", "new.png"));

        Assert.Equal(["new.png"], installed.Wallpapers);
        Assert.False(File.Exists(Path.Combine(installed.Directory, ThemeStore.WallpapersFolder, "old.png")));
    }

    [Fact]
    public async Task Duplicate_wallpaper_names_are_made_unique()
    {
        var details = Details("tokyo", "a.png");
        details = details with
        {
            Wallpapers = [details.Wallpapers[0], details.Wallpapers[0] with { Path = "backgrounds/dark/a.png" }],
        };

        var installed = await _store.InstallAsync(details);

        Assert.Equal(["a.png", "a-2.png"], installed.Wallpapers);
    }

    [Fact]
    public async Task Remove_deletes_theme_and_raises_changed()
    {
        await _store.InstallAsync(Details("tokyo", "1.png"));
        var changed = 0;
        _store.Changed += (_, _) => changed++;

        _store.Remove("tokyo");

        Assert.Null(_store.Get("tokyo"));
        Assert.Empty(_store.List());
        Assert.Equal(1, changed);
    }

    [Fact]
    public async Task Theme_without_wallpapers_or_palette_cannot_be_installed()
    {
        var details = Details("tokyo") with { Palette = null };
        await Assert.ThrowsAsync<InvalidOperationException>(() => _store.InstallAsync(details));
    }

    [Fact]
    public void Clean_up_removes_interrupted_staging_folders()
    {
        var staging = Path.Combine(_dir.AppPaths.ThemesDir, ".staging-tokyo-123");
        Directory.CreateDirectory(staging);

        _store.CleanUpStaging();

        Assert.False(Directory.Exists(staging));
    }

    [Fact]
    public void Settings_round_trip_and_default_when_missing_or_corrupt()
    {
        var settings = new SettingsStore(_dir.AppPaths);
        Assert.False(settings.Load().WelcomeSeen);

        settings.Update(s => s with { WelcomeSeen = true, ApplyDefaults = new ApplyOptions { AccentColor = false } });
        var loaded = settings.Load();
        Assert.True(loaded.WelcomeSeen);
        Assert.False(loaded.ApplyDefaults.AccentColor);
        Assert.True(loaded.ApplyDefaults.Wallpaper);

        File.WriteAllText(_dir.AppPaths.SettingsFile, "{ not json");
        Assert.Equal(new AppSettings().WelcomeSeen, settings.Load().WelcomeSeen);
    }

    private static ApplyResult Result(StepOutcome wallpaper, StepOutcome mode = StepOutcome.Applied) =>
        new([new StepResult(ApplyStep.Wallpaper, wallpaper), new StepResult(ApplyStep.AppearanceMode, mode)]);

    [Fact]
    public void Applying_records_the_theme_and_the_wallpaper_that_was_set()
    {
        var after = new AppSettings().AfterApply("tokyo", "2.png", Result(StepOutcome.Applied));

        Assert.Equal("tokyo", after.LastAppliedSlug);
        Assert.Equal("2.png", after.LastAppliedWallpaper);
    }

    [Fact]
    public void Applying_without_the_wallpaper_does_not_claim_its_wallpaper_is_on_the_desktop()
    {
        var before = new AppSettings { LastAppliedSlug = "tokyo", LastAppliedWallpaper = "2.png" };

        // Same theme, wallpaper unchecked: the desktop still shows 2.png.
        Assert.Equal("2.png", before.AfterApply("tokyo", "1.png", Result(StepOutcome.SkippedByUser)).LastAppliedWallpaper);

        // Another theme, wallpaper unchecked or failed: none of its wallpapers is on the desktop.
        var other = before.AfterApply("snow", "1.png", Result(StepOutcome.Failed));
        Assert.Equal("snow", other.LastAppliedSlug);
        Assert.Null(other.LastAppliedWallpaper);
    }

    [Fact]
    public void Applying_nothing_changes_nothing()
    {
        var before = new AppSettings { LastAppliedSlug = "tokyo", LastAppliedWallpaper = "2.png" };

        Assert.Same(before, before.AfterApply("snow", "1.png", Result(StepOutcome.Failed, StepOutcome.Failed)));
    }
}
