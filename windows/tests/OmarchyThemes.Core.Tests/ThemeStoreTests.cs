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
        // Both wallpapers, then the screenshot.
        Assert.Equal([0, 1, 2], reports.Select(r => r.FileIndex).Distinct().Order());
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

    /// <summary>A store whose folder moves fail while <paramref name="fail"/> says so for the source folder.</summary>
    private ThemeStore StoreWithMoves(Func<string, bool> fail) => new(_dir.AppPaths, _downloader, _time)
    {
        MoveDirectory = (from, to) =>
        {
            if (fail(Path.GetFileName(from)))
                throw new IOException("The folder is in use.");
            Directory.Move(from, to);
        },
    };

    [Fact]
    public async Task Reinstall_keeps_the_previous_copy_when_the_new_one_cannot_be_moved_in()
    {
        var failStaging = false;
        var store = StoreWithMoves(name => failStaging && name.StartsWith(".staging-"));
        await store.InstallAsync(Details("tokyo", "old.png"));

        failStaging = true;
        await Assert.ThrowsAsync<IOException>(() => store.InstallAsync(Details("tokyo", "new.png")));

        var kept = store.Get("tokyo")!;
        Assert.Equal(["old.png"], kept.Wallpapers);
        Assert.True(File.Exists(kept.WallpaperPath("old.png")));
        Assert.Equal([kept.Directory], Directory.GetDirectories(_dir.AppPaths.ThemesDir));
    }

    [Fact]
    public async Task Reinstall_changes_nothing_when_the_installed_copy_is_in_use()
    {
        var store = StoreWithMoves(name => name == "tokyo");
        await store.InstallAsync(Details("tokyo", "old.png"));

        await Assert.ThrowsAsync<IOException>(() => store.InstallAsync(Details("tokyo", "new.png")));

        Assert.Equal(["old.png"], store.Get("tokyo")!.Wallpapers);
        Assert.Single(Directory.GetDirectories(_dir.AppPaths.ThemesDir));
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
    public async Task Remove_leaves_the_theme_whole_when_it_is_in_use()
    {
        var store = StoreWithMoves(name => name == "tokyo");
        await store.InstallAsync(Details("tokyo", "1.png"));
        var changed = 0;
        store.Changed += (_, _) => changed++;

        Assert.Throws<IOException>(() => store.Remove("tokyo"));

        Assert.True(File.Exists(store.Get("tokyo")!.WallpaperPath("1.png")));
        Assert.Equal(0, changed);
    }

    [Fact]
    public async Task Clean_up_puts_back_a_previous_copy_a_failed_reinstall_left_aside()
    {
        await _store.InstallAsync(Details("tokyo", "1.png"));
        await _store.InstallAsync(Details("kanagawa", "k.png"));
        var themes = _dir.AppPaths.ThemesDir;
        var guid = Guid.NewGuid().ToString("N");
        // Tokyo is only aside; kanagawa's aside copy is outdated because it's installed.
        Directory.Move(Path.Combine(themes, "tokyo"), Path.Combine(themes, $".old-tokyo-{guid}"));
        Directory.CreateDirectory(Path.Combine(themes, $".old-kanagawa-{guid}"));
        Directory.CreateDirectory(Path.Combine(themes, $".removed-nord-{guid}"));
        Directory.CreateDirectory(Path.Combine(themes, ".old-not-ours"));

        _store.CleanUpStaging();

        Assert.Equal(["1.png"], _store.Get("tokyo")!.Wallpapers);
        Assert.NotNull(_store.Get("kanagawa"));
        Assert.Equal(["kanagawa", "tokyo"], Directory.GetDirectories(themes).Select(Path.GetFileName).Order());
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
