using Microsoft.Win32;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows.Registry;
using OmarchyThemes.Platform.Windows.Wallpaper;
using static OmarchyThemes.Platform.Windows.WindowsDesktopBackend;

namespace OmarchyThemes.Platform.Windows.Tests;

public sealed class WindowsDesktopBackendTests : IDisposable
{
    private readonly string _dir = Path.Combine(Path.GetTempPath(), "omatheme-win-tests", Guid.NewGuid().ToString("N"));
    private readonly FakeRegistry _registry = new();
    private readonly FakeWallpaperApi _wallpaper = new();
    private readonly FakeBroadcaster _broadcaster = new();
    private readonly FakeConverter _converter = new();
    private readonly WindowsDesktopBackend _backend;

    public WindowsDesktopBackendTests()
    {
        Directory.CreateDirectory(_dir);
        _backend = new WindowsDesktopBackend(_wallpaper, _registry, _broadcaster, _converter, Path.Combine(_dir, "snapshot"));
    }

    public void Dispose()
    {
        try { Directory.Delete(_dir, recursive: true); }
        catch (IOException) { }
    }

    [Fact]
    public void Reports_all_three_capabilities()
    {
        Assert.Equal(
            DesktopCapabilities.Wallpaper | DesktopCapabilities.AppearanceMode | DesktopCapabilities.AccentColor,
            _backend.Capabilities);
    }

    [Theory]
    [InlineData(AppearanceMode.Light, 1u)]
    [InlineData(AppearanceMode.Dark, 0u)]
    public async Task Appearance_mode_sets_apps_and_system_then_broadcasts(AppearanceMode mode, uint expected)
    {
        await _backend.SetAppearanceModeAsync(mode);

        Assert.Equal(expected, _registry.Get(PersonalizeKey, "AppsUseLightTheme")!.AsDWord);
        Assert.Equal(expected, _registry.Get(PersonalizeKey, "SystemUsesLightTheme")!.AsDWord);
        Assert.Equal(["ImmersiveColorSet"], _broadcaster.Areas);
    }

    [Fact]
    public async Task Accent_writes_the_values_the_settings_app_uses()
    {
        await _backend.SetAccentColorAsync(RgbColor.Parse("#7aa2f7"));

        Assert.Equal(0xFFF7A27Au, _registry.Get(DwmKey, "AccentColor")!.AsDWord);
        Assert.Equal(0xC47AA2F7u, _registry.Get(DwmKey, "ColorizationColor")!.AsDWord);
        Assert.Equal(0xC47AA2F7u, _registry.Get(DwmKey, "ColorizationAfterglow")!.AsDWord);
        Assert.Equal(0xFFF7A27Au, _registry.Get(AccentKey, "AccentColorMenu")!.AsDWord);
        Assert.NotNull(_registry.Get(AccentKey, "StartColorMenu"));
        var palette = (byte[])_registry.Get(AccentKey, "AccentPalette")!.Data;
        Assert.Equal(32, palette.Length);
        Assert.Equal(new byte[] { 0x7a, 0xa2, 0xf7, 0x00 }, palette[12..16]);
        Assert.Equal(0u, _registry.Get(DesktopKey, "AutoColorization")!.AsDWord);
        Assert.Equal(["ImmersiveColorSet"], _broadcaster.Areas);
    }

    [Fact]
    public async Task Near_black_accents_are_lifted_before_writing()
    {
        await _backend.SetAccentColorAsync(RgbColor.Parse("#0a0a0a")); // Snow theme

        var written = AccentMathFromAbgr(_registry.Get(DwmKey, "AccentColor")!.AsDWord!.Value);
        Assert.InRange(written.ToHsl().L, 0.24, 0.76);
    }

    [Fact]
    public async Task Only_tracked_values_are_ever_written()
    {
        await _backend.SetAppearanceModeAsync(AppearanceMode.Light);
        await _backend.SetAccentColorAsync(RgbColor.Parse("#123456"));

        Assert.All(_registry.Values.Keys, k => Assert.Contains(k, TrackedValues));
    }

    [Fact]
    public async Task Wallpaper_is_converted_when_needed_then_set_on_all_monitors()
    {
        await _backend.SetWallpaperAsync(@"C:\themes\vulkanite\wallpapers\1.webp", WallpaperFit.Span, null);

        Assert.Equal((@"C:\themes\vulkanite\wallpapers\1.webp.png", WallpaperFit.Span, (RgbColor?)null), _wallpaper.LastSet);
    }

    [Fact]
    public async Task Theme_background_is_passed_as_the_desktop_fill_color()
    {
        await _backend.SetWallpaperAsync(@"C:\themes\tokyo\wallpapers\1.png", WallpaperFit.Fit, RgbColor.Parse("#1a1b26"));

        Assert.Equal((@"C:\themes\tokyo\wallpapers\1.png", WallpaperFit.Fit, (RgbColor?)RgbColor.Parse("#1a1b26")), _wallpaper.LastSet);
    }

    [Fact]
    public void Colorref_is_0x00BBGGRR()
    {
        Assert.Equal(0x00F7A27Au, (uint)DesktopWallpaperApi.ToColorRef(RgbColor.Parse("#7aa2f7")));
        Assert.Equal(RgbColor.Parse("#7aa2f7"), DesktopWallpaperApi.FromColorRef(DesktopWallpaperApi.ToColorRef(RgbColor.Parse("#7aa2f7"))));
    }

    [Fact]
    public async Task Capture_then_restore_puts_back_registry_and_per_monitor_wallpapers()
    {
        var original = Path.Combine(_dir, "original.jpg");
        File.WriteAllBytes(original, [1, 2, 3]);
        _registry.Values[(PersonalizeKey, "AppsUseLightTheme")] = RegValue.DWord(1);
        _registry.Values[(DwmKey, "AccentColor")] = RegValue.DWord(0xFFD47800);
        _registry.Values[(AccentKey, "AccentPalette")] = RegValue.Binary([9, 9, 9]);
        _wallpaper.State = new WallpaperState(
            [new MonitorWallpaper("MON1", original), new MonitorWallpaper("MON2", "")],
            WallpaperFit.Fit,
            Background: RgbColor.Parse("#000000"));

        var snapshot = await _backend.CaptureAsync();

        // Apply a theme, which changes values and adds ones that didn't exist before.
        await _backend.SetAppearanceModeAsync(AppearanceMode.Dark);
        await _backend.SetAccentColorAsync(RgbColor.Parse("#ff0000"));
        await _backend.SetWallpaperAsync(Path.Combine(_dir, "theme.png"), WallpaperFit.Fill, RgbColor.Parse("#1a1b26"));

        await _backend.RestoreAsync(snapshot);

        Assert.Equal(1u, _registry.Get(PersonalizeKey, "AppsUseLightTheme")!.AsDWord);
        Assert.Equal(0xFFD47800u, _registry.Get(DwmKey, "AccentColor")!.AsDWord);
        Assert.Equal(new byte[] { 9, 9, 9 }, (byte[])_registry.Get(AccentKey, "AccentPalette")!.Data);
        // Values that were absent before are removed again.
        Assert.Null(_registry.Get(PersonalizeKey, "SystemUsesLightTheme"));
        Assert.Null(_registry.Get(DesktopKey, "AutoColorization"));
        Assert.Null(_registry.Get(DwmKey, "ColorizationColor"));

        var restored = _wallpaper.Restored!;
        Assert.Equal(WallpaperFit.Fit, restored.Fit);
        Assert.Equal(RgbColor.Parse("#000000"), restored.Background);
        Assert.Equal([new MonitorWallpaper("MON1", original), new MonitorWallpaper("MON2", "")], restored.Monitors);
    }

    [Fact]
    public async Task Restore_uses_the_private_copy_when_the_original_wallpaper_is_gone()
    {
        var original = Path.Combine(_dir, "TranscodedWallpaper");
        File.WriteAllBytes(original, [1, 2, 3]);
        _wallpaper.State = new WallpaperState([new MonitorWallpaper("MON1", original)], WallpaperFit.Fill);

        var snapshot = await _backend.CaptureAsync();
        File.Delete(original);
        await _backend.RestoreAsync(snapshot);

        var path = Assert.Single(_wallpaper.Restored!.Monitors).Path;
        Assert.NotEqual(original, path);
        Assert.StartsWith(Path.Combine(_dir, "snapshot"), path);
        Assert.Equal(new byte[] { 1, 2, 3 }, File.ReadAllBytes(path));
    }

    [Fact]
    public async Task Snapshot_survives_json_round_trip()
    {
        _registry.Values[(AccentKey, "AccentPalette")] = RegValue.Binary([1, 2, 3, 4]);
        _registry.Values[(DesktopKey, "AutoColorization")] = new RegValue(RegistryValueKind.String, "1");
        var snapshot = await _backend.CaptureAsync();
        var path = Path.Combine(_dir, "snap.json");

        Core.Storage.JsonFile.WriteAtomic(path, snapshot);
        var loaded = Core.Storage.JsonFile.TryRead<DesktopSnapshot>(path)!;
        await _backend.SetAccentColorAsync(RgbColor.Parse("#00ff00"));
        await _backend.RestoreAsync(loaded);

        Assert.Equal(new byte[] { 1, 2, 3, 4 }, (byte[])_registry.Get(AccentKey, "AccentPalette")!.Data);
        Assert.Equal(new RegValue(RegistryValueKind.String, "1"), _registry.Get(DesktopKey, "AutoColorization"));
    }

    [Theory]
    [InlineData(null)]
    [InlineData(RegistryValueKind.DWord)]
    [InlineData(RegistryValueKind.Binary)]
    [InlineData(RegistryValueKind.String)]
    [InlineData(RegistryValueKind.ExpandString)]
    [InlineData(RegistryValueKind.QWord)]
    public void Registry_values_round_trip_through_snapshot_strings(RegistryValueKind? kind)
    {
        RegValue? value = kind switch
        {
            null => null,
            RegistryValueKind.DWord => RegValue.DWord(0xFFD47800),
            RegistryValueKind.Binary => RegValue.Binary([0, 255, 7]),
            RegistryValueKind.QWord => new RegValue(RegistryValueKind.QWord, 1234567890123L),
            _ => new RegValue(kind.Value, "%USERPROFILE%\\x: y"),
        };

        Assert.Equal(value, RegValue.Deserialize(RegValue.Serialize(value)));
    }

    private static RgbColor AccentMathFromAbgr(uint v) => AccentMath.FromAbgr(v);

    private sealed class FakeRegistry : IRegistryAccess
    {
        public Dictionary<(string, string), RegValue> Values { get; } = new();
        public RegValue? Get(string key, string name) => Values.GetValueOrDefault((key, name));
        public RegValue? Read(string key, string name) => Get(key, name);
        public void Write(string key, string name, RegValue value) => Values[(key, name)] = value;
        public void Delete(string key, string name) => Values.Remove((key, name));
    }

    private sealed class FakeWallpaperApi : IWallpaperApi
    {
        public WallpaperState State { get; set; } = new([], null);
        public (string Path, WallpaperFit Fit, RgbColor? Background)? LastSet { get; private set; }
        public WallpaperState? Restored { get; private set; }

        public Task<WallpaperState> GetAsync() => Task.FromResult(State);

        public Task SetAsync(string path, WallpaperFit fit, RgbColor? background)
        {
            LastSet = (path, fit, background);
            return Task.CompletedTask;
        }

        public Task RestoreAsync(WallpaperState state)
        {
            Restored = state;
            return Task.CompletedTask;
        }
    }

    private sealed class FakeBroadcaster : ISettingsBroadcaster
    {
        public List<string> Areas { get; } = [];
        public void Broadcast(string area) => Areas.Add(area);
    }

    private sealed class FakeConverter : IImageConverter
    {
        public Task<string> EnsureWallpaperFormatAsync(string path, CancellationToken ct = default) =>
            Task.FromResult(path.EndsWith(".webp") ? path + ".png" : path);
    }
}
