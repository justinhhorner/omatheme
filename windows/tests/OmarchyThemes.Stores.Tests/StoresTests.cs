using System.Net;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores.Tests;

public sealed class PreferencesTests : IDisposable
{
    private readonly Harness _harness = new();

    public void Dispose() => _harness.Dispose();

    [Fact]
    public void Updates_are_saved_and_raise_changed()
    {
        var preferences = _harness.MakeStores().Preferences;
        var changed = 0;
        preferences.Changed += (_, _) => changed++;

        preferences.Update(s => s with { ApplyDefaults = s.ApplyDefaults with { Fit = WallpaperFit.Center } });
        preferences.Update(s => s with { ApplyDefaults = s.ApplyDefaults with { Fit = WallpaperFit.Center } });

        Assert.Equal(1, changed); // the second update changed nothing
        Assert.Equal(WallpaperFit.Center, _harness.MakeStores().Preferences.Settings.ApplyDefaults.Fit);
    }

    [Fact]
    public void Welcome_stays_dismissed()
    {
        Assert.False(_harness.MakeStores().Preferences.Settings.WelcomeSeen);

        _harness.MakeStores().Preferences.DismissWelcome();

        Assert.True(_harness.MakeStores().Preferences.Settings.WelcomeSeen);
    }

    [Fact]
    public void A_settings_file_that_cannot_be_written_keeps_the_change_for_the_session()
    {
        var preferences = _harness.MakeStores().Preferences;
        Directory.CreateDirectory(_harness.Dir.AppPaths.SettingsFile); // a folder where the file goes

        preferences.DismissWelcome();

        Assert.True(preferences.Settings.WelcomeSeen);
    }
}

public sealed class CatalogStoreTests : IDisposable
{
    private readonly Harness _harness = new();

    public void Dispose() => _harness.Dispose();

    [Fact]
    public async Task First_load_fetches_and_lists_default_themes_first()
    {
        _harness.ServeCatalog();
        var catalog = _harness.MakeStores().Catalog;

        await catalog.LoadIfNeededAsync();

        Assert.Equal(7, catalog.Entries.Count);
        Assert.True(catalog.Entries[0].IsDefaultTheme);
        Assert.Equal(_harness.Time.Now, catalog.FetchedAt);
        Assert.False(catalog.IsLoading || catalog.IsRefreshing);
        Assert.Null(catalog.Error ?? catalog.Notice ?? catalog.DefaultThemesNotice);
    }

    [Fact]
    public async Task A_fresh_cached_catalog_loads_without_the_network()
    {
        _harness.ServeCatalog();
        await _harness.MakeStores().Catalog.LoadIfNeededAsync();
        var requests = _harness.Http.Requests.Count;

        var catalog = _harness.MakeStores().Catalog;
        await catalog.LoadIfNeededAsync();

        Assert.Equal(7, catalog.Entries.Count);
        Assert.Equal(requests, _harness.Http.Requests.Count);
    }

    [Fact]
    public async Task An_old_cached_catalog_is_refreshed()
    {
        _harness.ServeCatalog();
        await _harness.MakeStores().Catalog.LoadIfNeededAsync();
        _harness.Time.Advance(CatalogStore.AutoRefreshAfter + TimeSpan.FromMinutes(1));

        await _harness.MakeStores().Catalog.LoadIfNeededAsync();

        Assert.Equal(2, _harness.Http.CountFor(Harness.PageUrl));
    }

    [Fact]
    public async Task Concurrent_refreshes_share_one_request()
    {
        _harness.ServeCatalog();
        var gate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        _harness.HttpGate = gate;
        var catalog = _harness.MakeStores().Catalog;

        var first = catalog.RefreshAsync();
        var second = catalog.RefreshAsync();
        gate.SetResult();
        await Task.WhenAll(first, second);

        Assert.Same(first, second);
        Assert.Equal(1, _harness.Http.CountFor(Harness.PageUrl));
    }

    [Fact]
    public async Task Offline_first_launch_shows_an_error()
    {
        _harness.Http.On(Harness.PageUrl, _ => FakeHttpHandler.Throw());
        var catalog = _harness.MakeStores().Catalog;

        await catalog.LoadIfNeededAsync();

        Assert.Empty(catalog.Entries);
        Assert.StartsWith("Couldn't load themes from omarchy.org", catalog.Error);
    }

    [Fact]
    public async Task Going_offline_later_keeps_the_catalog_with_a_notice()
    {
        _harness.ServeCatalog();
        var catalog = _harness.MakeStores().Catalog;
        await catalog.RefreshAsync();
        _harness.Http.On(Harness.PageUrl, _ => FakeHttpHandler.Throw());

        await catalog.RefreshAsync();

        Assert.Equal(7, catalog.Entries.Count);
        Assert.Null(catalog.Error);
        Assert.StartsWith("Couldn't reach omarchy.org", catalog.Notice);
    }

    [Fact]
    public async Task A_page_without_themes_keeps_the_catalog_and_says_why()
    {
        _harness.ServeCatalog();
        var catalog = _harness.MakeStores().Catalog;
        await catalog.RefreshAsync();
        _harness.Http.On(Harness.PageUrl, "<html><body>Under maintenance</body></html>");

        await catalog.RefreshAsync();

        Assert.Equal(7, catalog.Entries.Count);
        Assert.StartsWith("No themes were found", catalog.Notice);
    }

    [Fact]
    public async Task Missing_default_themes_get_their_own_notice()
    {
        _harness.Http.On(Harness.PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(Harness.TreeUrl, _ => FakeHttpHandler.Status(HttpStatusCode.Forbidden, ("x-ratelimit-remaining", "0")));
        var catalog = _harness.MakeStores().Catalog;

        await catalog.RefreshAsync();

        Assert.Equal(4, catalog.Entries.Count);
        Assert.Contains("rate limit", catalog.DefaultThemesNotice);
    }

    [Fact]
    public void Entry_for_a_downloaded_theme_falls_back_to_its_manifest()
    {
        var catalog = _harness.MakeStores().Catalog;
        var theme = new InstalledTheme { Slug = "gone", Name = "Gone", RepoUrl = "https://github.com/o/gone" };

        Assert.Equal(new CatalogEntry("gone", "Gone", "https://github.com/o/gone", ScreenshotUrl: null), catalog.EntryFor(theme));
    }

    [Fact]
    public void Filter_matches_name_or_repo_and_can_keep_downloaded_themes_only()
    {
        CatalogEntry[] entries =
        [
            new("tokyo", "Tokyo Night", "https://github.com/someone/omarchy-tokyo", null),
            new("snow", "Snow", "https://github.com/other/snow-theme", null),
        ];
        var none = new HashSet<string>();

        Assert.Equal(["tokyo"], CatalogStore.Filter(entries, " tokyo ", false, none).Select(e => e.Slug));
        Assert.Equal(["snow"], CatalogStore.Filter(entries, "OTHER/", false, none).Select(e => e.Slug));
        Assert.Equal(["tokyo", "snow"], CatalogStore.Filter(entries, "", false, none).Select(e => e.Slug));
        Assert.Equal(["snow"], CatalogStore.Filter(entries, "", true, new HashSet<string> { "snow" }).Select(e => e.Slug));
    }
}

public sealed class ThemeLibraryTests : IDisposable
{
    private readonly Harness _harness = new();

    public void Dispose() => _harness.Dispose();

    [Fact]
    public async Task Download_installs_and_lists_the_theme()
    {
        var stores = _harness.MakeStores();
        var changed = 0;
        stores.Library.Changed += (_, _) => Interlocked.Increment(ref changed);

        var theme = await _harness.InstallTokyoAsync(stores);

        Assert.Equal(["0-winding-road.webp", "2-swirl-buck.webp", "10-oma.webp"], theme.Wallpapers);
        Assert.Equal(["omarchy.tokyo-night"], stores.Library.Installed.Select(t => t.Slug));
        Assert.Null(stores.Library.RunningDownload(Harness.Tokyo.Slug));
        Assert.Equal(1, changed);
    }

    [Fact]
    public async Task A_failed_download_says_why_and_saves_nothing()
    {
        _harness.ServeCatalog();
        _harness.Downloader.FailUrls.Add(Harness.Raw + "tokyo-night/backgrounds/2-swirl-buck.webp");
        var stores = _harness.MakeStores();

        var outcome = await stores.Library.Download(Harness.Tokyo).Completion;

        Assert.Null(outcome.Theme);
        Assert.Empty(stores.Library.Installed);
        Assert.Equal(new Banner(SummaryKind.Error, "Download failed", "Couldn't reach GitHub. Check your internet connection and try again."),
            ThemeLibrary.DescribeFailedDownload(outcome));
    }

    [Fact]
    public void A_cancelled_download_is_not_an_error()
    {
        var banner = ThemeLibrary.DescribeFailedDownload(new DownloadOutcome(null, null, Cancelled: true, Error: null));

        Assert.Equal(new Banner(SummaryKind.Info, "Download cancelled", "Nothing was saved."), banner);
    }

    [Fact]
    public async Task Clear_cache_forgets_lookups_and_deletes_the_cache()
    {
        _harness.ServeCatalog();
        var stores = _harness.MakeStores();
        await stores.Library.ResolveAsync(Harness.Tokyo);
        Assert.NotNull(stores.Library.CachedDetails(Harness.Tokyo.Slug));

        var banner = stores.Library.ClearCache();

        Assert.Equal("Cache cleared", banner.Title);
        Assert.Null(stores.Library.CachedDetails(Harness.Tokyo.Slug));
        Assert.Empty(Directory.EnumerateFileSystemEntries(_harness.Dir.AppPaths.CacheDir));
    }

    [Fact]
    public async Task Remove_reports_its_outcome()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);

        var banner = stores.Library.Remove(theme);

        Assert.Equal(new Banner(SummaryKind.Info, "Download removed", "Tokyo Night was removed from this PC."), banner);
        Assert.Empty(stores.Library.Installed);
    }

    [Fact]
    public async Task A_theme_in_use_is_not_removed_and_says_why()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        _harness.FailMoves = true;

        var banner = stores.Library.Remove(theme);

        Assert.Equal(new Banner(SummaryKind.Error, "Couldn't remove Tokyo Night", "The folder is in use."), banner);
        Assert.Single(stores.Library.Installed);
    }
}

public sealed class DesktopStoreTests : IDisposable
{
    private readonly Harness _harness = new();

    public void Dispose() => _harness.Dispose();

    [Fact]
    public async Task Apply_records_the_current_theme_and_snapshot()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        Assert.False(stores.Desktop.HasOriginalSnapshot);

        var summary = await stores.Desktop.ApplyAsync(theme, "2-swirl-buck.webp", new ApplyOptions());

        Assert.Equal(SummaryKind.Success, summary.Kind);
        Assert.True(stores.Desktop.HasOriginalSnapshot);
        Assert.Equal(theme.Slug, stores.Desktop.CurrentTheme?.Slug);
        Assert.Equal("2-swirl-buck.webp", stores.Desktop.CurrentWallpaper);
        Assert.Equal("2-swirl-buck.webp", stores.Desktop.PreferredWallpaper(theme));
        Assert.Null(stores.Desktop.ApplyingSlug);
        // The current theme is remembered across launches.
        Assert.Equal(theme.Slug, _harness.MakeStores().Desktop.ActiveSlug);
    }

    [Fact]
    public async Task One_click_apply_uses_the_saved_defaults_and_the_preferred_wallpaper()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        stores.Preferences.Update(s => s with { ApplyDefaults = new ApplyOptions { AppearanceMode = false, AccentColor = false } });

        await stores.Desktop.ApplyWithDefaultsAsync(theme);

        Assert.Equal(["capture", "wallpaper:0-winding-road.webp:Fill:#e1e2e7"], _harness.Backend.Calls);
    }

    [Fact]
    public async Task Choices_from_the_apply_dialog_can_become_the_defaults()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        var choices = new ApplyOptions { AccentColor = false, Fit = WallpaperFit.Fit };

        await stores.Desktop.ApplyAsync(theme, null, choices with { Fit = WallpaperFit.Stretch });
        Assert.Equal(new ApplyOptions(), stores.Desktop.ApplyDefaults);

        await stores.Desktop.ApplyAsync(theme, null, choices, rememberOptions: true);
        Assert.Equal(choices, _harness.MakeStores().Desktop.ApplyDefaults);
    }

    [Fact]
    public async Task Only_one_apply_runs_at_a_time()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        var gate = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        _harness.Backend.Gate = gate;

        var first = stores.Desktop.ApplyAsync(theme, null, new ApplyOptions());
        var second = await stores.Desktop.ApplyAsync(theme, null, new ApplyOptions());
        Assert.Equal(theme.Slug, stores.Desktop.ApplyingSlug);
        gate.SetResult();

        Assert.Equal("Already applying a theme", second.Title);
        Assert.Equal("Tokyo Night applied", (await first).Title);
    }

    [Fact]
    public async Task Switching_the_current_wallpaper_sets_only_the_wallpaper_with_the_saved_fit()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        stores.Preferences.Update(s => s with { ApplyDefaults = s.ApplyDefaults with { Fit = WallpaperFit.Center } });
        await stores.Desktop.ApplyAsync(theme, "0-winding-road.webp", new ApplyOptions());
        _harness.Backend.Calls.Clear();

        var banner = await stores.Desktop.SetCurrentWallpaperAsync("10-oma.webp");

        Assert.Null(banner);
        Assert.Equal(["wallpaper:10-oma.webp:Center:#e1e2e7"], _harness.Backend.Calls);
        Assert.Equal("10-oma.webp", stores.Desktop.CurrentWallpaper);
        Assert.Null(stores.Desktop.SettingWallpaper);
    }

    [Fact]
    public async Task A_failed_wallpaper_switch_says_so_and_keeps_the_current_one()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        await stores.Desktop.ApplyAsync(theme, "0-winding-road.webp", new ApplyOptions());
        _harness.Backend.FailOn.Add("wallpaper");

        var banner = await stores.Desktop.SetCurrentWallpaperAsync("10-oma.webp");

        Assert.Equal("Couldn't change the wallpaper", banner?.Title);
        Assert.Equal(SummaryKind.Error, banner?.Kind);
        Assert.Equal("0-winding-road.webp", stores.Desktop.CurrentWallpaper);
    }

    [Fact]
    public async Task An_apply_whose_settings_cannot_be_saved_still_reports_what_changed()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        Directory.CreateDirectory(_harness.Dir.AppPaths.SettingsFile); // a folder where the file goes

        var summary = await stores.Desktop.ApplyAsync(theme, null, new ApplyOptions());

        Assert.Equal(SummaryKind.Success, summary.Kind);
        Assert.Equal(theme.Slug, stores.Desktop.ActiveSlug);
    }

    [Fact]
    public async Task Restore_forgets_the_current_theme()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        await stores.Desktop.ApplyAsync(theme, null, new ApplyOptions());

        var banner = await stores.Desktop.RestoreOriginalAsync();

        Assert.Equal(SummaryKind.Success, banner.Kind);
        Assert.Null(stores.Desktop.CurrentTheme);
        Assert.False(stores.Desktop.HasOriginalSnapshot);
    }

    [Fact]
    public async Task A_failed_restore_keeps_everything()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        await stores.Desktop.ApplyAsync(theme, null, new ApplyOptions());
        _harness.Backend.FailOn.Add("restore");

        var banner = await stores.Desktop.RestoreOriginalAsync();

        Assert.Equal(new Banner(SummaryKind.Error, "Couldn't restore your desktop", "restore exploded"), banner);
        Assert.True(stores.Desktop.HasOriginalSnapshot);
        Assert.Equal(theme.Slug, stores.Desktop.CurrentTheme?.Slug);
    }

    [Fact]
    public async Task Nothing_to_restore_before_the_first_apply()
    {
        var banner = await _harness.MakeStores().Desktop.RestoreOriginalAsync();

        Assert.Equal("Nothing to restore", banner.Title);
        Assert.Empty(_harness.Backend.Calls);
    }
}

public sealed class TerminalStoreTests : IDisposable
{
    private readonly Harness _harness = new();

    public void Dispose() => _harness.Dispose();

    [Fact]
    public async Task Add_and_remove()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);

        var added = await stores.Terminal.AddAsync(Harness.Tokyo, theme.Palette!);

        Assert.Equal(new Banner(SummaryKind.Success, "Added to Test Terminal", "Pick Tokyo Night (Omarchy)."), added);
        Assert.True(stores.Terminal.IsAdded(Harness.Tokyo));

        var removed = stores.Terminal.Remove(Harness.Tokyo);

        Assert.Equal(new Banner(SummaryKind.Info, "Removed from Test Terminal", "Tokyo Night (Omarchy) is gone."), removed);
        Assert.False(stores.Terminal.IsAdded(Harness.Tokyo));
    }

    [Fact]
    public async Task Failures_are_reported()
    {
        var stores = _harness.MakeStores();
        var theme = await _harness.InstallTokyoAsync(stores);
        _harness.Terminal.Fail = true;

        var banner = await stores.Terminal.AddAsync(Harness.Tokyo, theme.Palette!);

        Assert.Equal(new Banner(SummaryKind.Error, "Couldn't update Test Terminal", "The terminal's folder is read-only."), banner);
    }

    [Fact]
    public async Task Old_downloads_are_looked_up_again_for_their_bright_colors()
    {
        var stores = _harness.MakeStores();
        await _harness.InstallTokyoAsync(stores);
        // A palette saved before `muted` was read.
        var saved = new Palette
        {
            Background = RgbColor.Parse("#e1e2e7"),
            Foreground = RgbColor.Parse("#3760bf"),
            Accent = RgbColor.Parse("#2e7de9"),
            Source = PaletteSource.ColorsToml,
        };

        await stores.Terminal.AddAsync(Harness.Tokyo, saved);

        Assert.Equal(RgbColor.Parse("#8990b3"), _harness.Terminal.Schemes[Harness.Tokyo.Slug].Ansi[8]); // colors-named.toml's muted
    }
}
