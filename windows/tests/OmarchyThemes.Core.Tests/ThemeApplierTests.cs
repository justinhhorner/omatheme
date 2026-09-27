using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

public sealed class ThemeApplierTests : IDisposable
{
    private readonly TempDir _dir = new();
    private readonly FakeDesktopBackend _backend = new();
    private readonly InMemorySnapshotStore _snapshots = new();
    private readonly ThemeApplier _applier;
    private readonly string _wallpaper;

    public ThemeApplierTests()
    {
        _applier = new ThemeApplier(_backend, _snapshots);
        _wallpaper = Path.Combine(_dir.Path, "wall.png");
        File.WriteAllBytes(_wallpaper, [1, 2, 3]);
    }

    public void Dispose() => _dir.Dispose();

    private ApplyRequest Request(ApplyOptions? options = null, string? wallpaper = "", RgbColor? accent = null) => new(
        "Tokyo",
        wallpaper == "" ? _wallpaper : wallpaper,
        AppearanceMode.Light,
        accent ?? RgbColor.Parse("#7aa2f7"),
        options ?? new ApplyOptions());

    [Fact]
    public async Task Saves_original_desktop_then_applies_every_aspect_in_order()
    {
        var result = await _applier.ApplyAsync(Request());

        Assert.Equal(["capture", "wallpaper:wall.png:Fill", "mode:Light", "accent:#7aa2f7"], _backend.Calls);
        Assert.True(result.Succeeded);
        Assert.All(result.Steps, s => Assert.Equal(StepOutcome.Applied, s.Outcome));
        Assert.Same(_backend.SnapshotToReturn, _snapshots.Snapshot);
    }

    [Fact]
    public async Task Passes_the_theme_background_as_the_wallpaper_fill_color()
    {
        await _applier.ApplyAsync(Request(new ApplyOptions { Fit = WallpaperFit.Fit }) with { Background = RgbColor.Parse("#1a1b26") });

        Assert.Contains("wallpaper:wall.png:Fit:#1a1b26", _backend.Calls);
    }

    [Fact]
    public async Task Only_the_first_apply_takes_a_snapshot()
    {
        await _applier.ApplyAsync(Request());
        _backend.Calls.Clear();

        var second = await _applier.ApplyAsync(Request());

        Assert.DoesNotContain("capture", _backend.Calls);
        Assert.Null(second.For(ApplyStep.SaveOriginal));
    }

    [Fact]
    public async Task Changes_nothing_if_the_original_desktop_cannot_be_saved()
    {
        _backend.FailOn.Add("capture");

        var result = await _applier.ApplyAsync(Request());

        Assert.Equal(["capture"], _backend.Calls);
        Assert.Equal(StepOutcome.Failed, result.For(ApplyStep.SaveOriginal)!.Outcome);
        Assert.Equal(StepOutcome.NotAttempted, result.For(ApplyStep.Wallpaper)!.Outcome);
        Assert.False(result.AnyApplied);
        Assert.Null(_snapshots.Snapshot);
    }

    [Fact]
    public async Task Respects_user_choices()
    {
        var result = await _applier.ApplyAsync(Request(new ApplyOptions { AppearanceMode = false, AccentColor = false, Fit = WallpaperFit.Span }));

        Assert.Equal(["capture", "wallpaper:wall.png:Span"], _backend.Calls);
        Assert.Equal(StepOutcome.SkippedByUser, result.For(ApplyStep.AppearanceMode)!.Outcome);
        Assert.Equal(StepOutcome.SkippedByUser, result.For(ApplyStep.AccentColor)!.Outcome);
        Assert.True(result.Succeeded);
    }

    [Fact]
    public async Task Skips_what_the_platform_cannot_do()
    {
        _backend.Capabilities = DesktopCapabilities.Wallpaper;

        var result = await _applier.ApplyAsync(Request());

        Assert.Equal(["capture", "wallpaper:wall.png:Fill"], _backend.Calls);
        Assert.Equal(StepOutcome.NotSupported, result.For(ApplyStep.AppearanceMode)!.Outcome);
        Assert.Equal(StepOutcome.NotSupported, result.For(ApplyStep.AccentColor)!.Outcome);
    }

    [Fact]
    public async Task Skips_aspects_the_theme_has_no_data_for()
    {
        var request = Request(wallpaper: null) with { Accent = null };

        var result = await _applier.ApplyAsync(request);

        Assert.Equal(["capture", "mode:Light"], _backend.Calls);
        Assert.Equal(StepOutcome.NoData, result.For(ApplyStep.Wallpaper)!.Outcome);
        Assert.Equal(StepOutcome.NoData, result.For(ApplyStep.AccentColor)!.Outcome);
    }

    [Fact]
    public async Task One_failing_step_does_not_stop_the_others()
    {
        _backend.FailOn.Add("mode");

        var result = await _applier.ApplyAsync(Request());

        Assert.Contains("accent:#7aa2f7", _backend.Calls);
        var mode = result.For(ApplyStep.AppearanceMode)!;
        Assert.Equal(StepOutcome.Failed, mode.Outcome);
        Assert.Equal("mode exploded", mode.Error);
        Assert.True(result.AnyApplied);
        Assert.True(result.AnyFailed);
        Assert.False(result.Succeeded);
    }

    [Fact]
    public async Task Missing_wallpaper_file_fails_that_step_without_calling_the_os()
    {
        var result = await _applier.ApplyAsync(Request(wallpaper: Path.Combine(_dir.Path, "gone.png")));

        Assert.DoesNotContain(_backend.Calls, c => c.StartsWith("wallpaper"));
        Assert.Equal(StepOutcome.Failed, result.For(ApplyStep.Wallpaper)!.Outcome);
        Assert.Equal(StepOutcome.Applied, result.For(ApplyStep.AccentColor)!.Outcome);
    }

    [Fact]
    public async Task Cancellation_propagates()
    {
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => _applier.ApplyAsync(Request(), ct: cts.Token));
        Assert.Empty(_backend.Calls);
    }

    [Fact]
    public async Task Reports_progress_for_each_step_it_runs()
    {
        var steps = new List<ApplyStep>();

        await _applier.ApplyAsync(Request(new ApplyOptions { AccentColor = false }), new Collector(steps));

        Assert.Equal([ApplyStep.SaveOriginal, ApplyStep.Wallpaper, ApplyStep.AppearanceMode], steps);
    }

    [Fact]
    public async Task Restore_puts_back_the_snapshot_once()
    {
        Assert.False(await _applier.RestoreOriginalAsync());

        await _applier.ApplyAsync(Request());
        Assert.True(_applier.HasOriginalSnapshot);

        Assert.True(await _applier.RestoreOriginalAsync());
        Assert.Same(_backend.SnapshotToReturn, _backend.Restored);
        Assert.False(_applier.HasOriginalSnapshot);
        Assert.False(await _applier.RestoreOriginalAsync());
    }

    [Fact]
    public void Request_from_installed_theme_uses_chosen_or_first_wallpaper()
    {
        var theme = new InstalledTheme
        {
            Slug = "t",
            Name = "T",
            RepoUrl = "https://github.com/o/t",
            Mode = AppearanceMode.Dark,
            Wallpapers = ["1.png", "2.png"],
            Directory = _dir.Path,
        };
        var options = new ApplyOptions();

        Assert.EndsWith("2.png", ApplyRequest.FromInstalled(theme, "2.png", options).WallpaperPath);
        Assert.EndsWith("1.png", ApplyRequest.FromInstalled(theme, "missing.png", options).WallpaperPath);
        Assert.EndsWith("1.png", ApplyRequest.FromInstalled(theme, null, options).WallpaperPath);
        Assert.Null(ApplyRequest.FromInstalled(theme, null, options).Accent);
        Assert.Null(ApplyRequest.FromInstalled(theme, null, options).Background);
        Assert.Null(ApplyRequest.FromInstalled(theme with { Wallpapers = [] }, null, options).WallpaperPath);

        var withPalette = theme with
        {
            Palette = new Palette
            {
                Background = RgbColor.Parse("#1a1b26"),
                Foreground = RgbColor.Parse("#a9b1d6"),
                Accent = RgbColor.Parse("#7aa2f7"),
                Source = PaletteSource.ColorsToml,
            },
        };
        var request = ApplyRequest.FromInstalled(withPalette, null, options);
        Assert.Equal(RgbColor.Parse("#1a1b26"), request.Background);
        Assert.Equal(RgbColor.Parse("#7aa2f7"), request.Accent);
    }

    [Fact]
    public void File_snapshot_store_round_trips()
    {
        var store = new FileSnapshotStore(_dir.AppPaths);
        var snapshot = new DesktopSnapshot(DateTimeOffset.UnixEpoch, new Dictionary<string, string> { ["k"] = "v" });

        store.Save(snapshot);
        var loaded = store.Load()!;
        Assert.Equal("v", loaded.Values["k"]);

        store.Clear();
        Assert.Null(store.Load());
    }

    private sealed class Collector(List<ApplyStep> into) : IProgress<ApplyStep>
    {
        public void Report(ApplyStep value) => into.Add(value);
    }
}
