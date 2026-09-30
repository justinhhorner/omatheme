using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows.Registry;
using OmarchyThemes.Platform.Windows.Wallpaper;

namespace OmarchyThemes.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="IDesktopBackend"/>. Everything is per-user (HKCU) and
/// needs no elevation.
/// <list type="bullet">
/// <item>Wallpaper: IDesktopWallpaper (SystemParametersInfo fallback).</item>
/// <item>Light/dark: Themes\Personalize AppsUseLightTheme + SystemUsesLightTheme.</item>
/// <item>Accent: DWM AccentColor/ColorizationColor + Explorer\Accent AccentPalette. Windows has
/// no public API for the accent, so this mirrors what the Settings app stores; some surfaces
/// only refresh after sign-out.</item>
/// </list>
/// </summary>
public sealed class WindowsDesktopBackend : IDesktopBackend
{
    internal const string PersonalizeKey = @"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";
    internal const string DwmKey = @"Software\Microsoft\Windows\DWM";
    internal const string AccentKey = @"Software\Microsoft\Windows\CurrentVersion\Explorer\Accent";
    internal const string DesktopKey = @"Control Panel\Desktop";

    /// <summary>Every registry value this backend may change, and therefore snapshots.</summary>
    internal static readonly (string Key, string Name)[] TrackedValues =
    [
        (PersonalizeKey, "AppsUseLightTheme"),
        (PersonalizeKey, "SystemUsesLightTheme"),
        (DwmKey, "AccentColor"),
        (DwmKey, "ColorizationColor"),
        (DwmKey, "ColorizationAfterglow"),
        (AccentKey, "AccentPalette"),
        (AccentKey, "AccentColorMenu"),
        (AccentKey, "StartColorMenu"),
        (DesktopKey, "AutoColorization"),
    ];

    private const string RegPrefix = "reg:";
    private const string MonitorPrefix = "monitor:";
    private const string CopyPrefix = "monitor-copy:";
    private const string FitKey = "wallpaper-fit";
    private const string BackgroundKey = "wallpaper-background";

    private readonly IWallpaperApi _wallpaper;
    private readonly IRegistryAccess _registry;
    private readonly ISettingsBroadcaster _broadcaster;
    private readonly IImageConverter _images;
    private readonly string _snapshotAssetsDir;
    private readonly TimeProvider _time;

    public WindowsDesktopBackend(
        IWallpaperApi wallpaper,
        IRegistryAccess registry,
        ISettingsBroadcaster broadcaster,
        IImageConverter images,
        string snapshotAssetsDir,
        TimeProvider? time = null)
    {
        _wallpaper = wallpaper;
        _registry = registry;
        _broadcaster = broadcaster;
        _images = images;
        _snapshotAssetsDir = snapshotAssetsDir;
        _time = time ?? TimeProvider.System;
    }

    /// <param name="snapshotAssetsDir">Where copies of the user's original wallpapers are kept for Restore.</param>
    public static WindowsDesktopBackend CreateDefault(string snapshotAssetsDir) => new(
        new DesktopWallpaperApi(), new CurrentUserRegistry(), new Win32SettingsBroadcaster(), new WicImageConverter(), snapshotAssetsDir);

    /// <summary>What this backend changes; the dry-run backend reports the same.</summary>
    public const DesktopCapabilities Supported =
        DesktopCapabilities.Wallpaper | DesktopCapabilities.AppearanceMode | DesktopCapabilities.AccentColor;

    public const string AccentNote = "Some parts of Windows may only pick up the new accent color after you sign out.";

    public DesktopCapabilities Capabilities => Supported;

    public string? AccentColorNote => AccentNote;

    public async Task<DesktopSnapshot> CaptureAsync(CancellationToken ct = default)
    {
        var values = new Dictionary<string, string>();

        foreach (var (key, name) in TrackedValues)
            values[RegSnapshotKey(key, name)] = RegValue.Serialize(_registry.Read(key, name));

        var state = await _wallpaper.GetAsync().ConfigureAwait(false);
        if (state.Fit is { } fit)
            values[FitKey] = fit.ToString();
        // The fill color around Fit/Center wallpapers is system-wide, so it's restored too.
        if (state.Background is { } background)
            values[BackgroundKey] = background.ToHex();

        // Keep a private copy of each original wallpaper: Windows' own copy (TranscodedWallpaper)
        // is overwritten when we set a new one, and the user may delete the original file later.
        // The copies go to a staging folder that replaces the previous ones only once they're all made.
        var staging = _snapshotAssetsDir + ".new";
        DeleteDirectoryIfExists(staging);
        try
        {
            var copies = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            for (var i = 0; i < state.Monitors.Count; i++)
            {
                var monitor = state.Monitors[i];
                values[MonitorPrefix + monitor.MonitorId] = monitor.Path;
                if (monitor.Path.Length == 0 || !File.Exists(monitor.Path))
                    continue;
                if (!copies.TryGetValue(monitor.Path, out var copy))
                {
                    var fileName = $"{i}{ImageExtension(monitor.Path)}";
                    Directory.CreateDirectory(staging);
                    File.Copy(monitor.Path, Path.Combine(staging, fileName), overwrite: true);
                    copy = Path.Combine(_snapshotAssetsDir, fileName);
                    copies[monitor.Path] = copy;
                }
                values[CopyPrefix + monitor.MonitorId] = copy;
            }

            DeleteDirectoryIfExists(_snapshotAssetsDir);
            if (Directory.Exists(staging))
                Directory.Move(staging, _snapshotAssetsDir);
        }
        catch
        {
            try { DeleteDirectoryIfExists(staging); }
            catch (Exception e) when (e is IOException or UnauthorizedAccessException) { }
            throw;
        }

        return new DesktopSnapshot(_time.GetUtcNow(), values);
    }

    public async Task RestoreAsync(DesktopSnapshot snapshot, CancellationToken ct = default)
    {
        foreach (var (key, name) in TrackedValues)
        {
            if (!snapshot.Values.TryGetValue(RegSnapshotKey(key, name), out var serialized))
                continue;
            if (RegValue.Deserialize(serialized) is { } value)
                _registry.Write(key, name, value);
            else
                _registry.Delete(key, name);
        }
        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);

        var monitors = snapshot.Values
            .Where(kv => kv.Key.StartsWith(MonitorPrefix, StringComparison.Ordinal))
            .Select(kv =>
            {
                var id = kv.Key[MonitorPrefix.Length..];
                var path = kv.Value;
                if (path.Length > 0 && !File.Exists(path)
                    && snapshot.Values.TryGetValue(CopyPrefix + id, out var copy) && File.Exists(copy))
                    path = copy;
                return new MonitorWallpaper(id, path);
            })
            .ToList();
        if (monitors.Count > 0)
        {
            WallpaperFit? fit = snapshot.Values.TryGetValue(FitKey, out var f) && Enum.TryParse<WallpaperFit>(f, out var parsed) ? parsed : null;
            RgbColor? background = snapshot.Values.TryGetValue(BackgroundKey, out var b) && RgbColor.TryParse(b, out var color) ? color : null;
            await _wallpaper.RestoreAsync(new WallpaperState(monitors, fit, background)).ConfigureAwait(false);
        }
    }

    public async Task SetWallpaperAsync(string imagePath, WallpaperFit fit, RgbColor? fillColor, CancellationToken ct = default)
    {
        var usable = await _images.EnsureWallpaperFormatAsync(imagePath, ct).ConfigureAwait(false);
        ct.ThrowIfCancellationRequested();
        await _wallpaper.SetAsync(usable, fit, fillColor).ConfigureAwait(false);
    }

    public Task SetAppearanceModeAsync(AppearanceMode mode, CancellationToken ct = default)
    {
        var light = RegValue.DWord(mode == AppearanceMode.Light ? 1u : 0u);
        _registry.Write(PersonalizeKey, "AppsUseLightTheme", light);
        _registry.Write(PersonalizeKey, "SystemUsesLightTheme", light);
        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);
        return Task.CompletedTask;
    }

    public Task SetAccentColorAsync(RgbColor accent, CancellationToken ct = default)
    {
        var color = AccentMath.NormalizeAccent(accent);
        var abgr = AccentMath.ToAbgr(color);
        var colorization = AccentMath.ToArgb(color, 0xC4);
        var startMenu = AccentMath.ToAbgr(AccentMath.Shades(color)[4]);

        // Stop Windows from re-deriving the accent from the wallpaper we may have just set.
        _registry.Write(DesktopKey, "AutoColorization", RegValue.DWord(0));

        _registry.Write(AccentKey, "AccentPalette", RegValue.Binary(AccentMath.ToAccentPaletteBytes(color)));
        _registry.Write(AccentKey, "AccentColorMenu", RegValue.DWord(abgr));
        _registry.Write(AccentKey, "StartColorMenu", RegValue.DWord(startMenu));

        _registry.Write(DwmKey, "AccentColor", RegValue.DWord(abgr));
        _registry.Write(DwmKey, "ColorizationColor", RegValue.DWord(colorization));
        _registry.Write(DwmKey, "ColorizationAfterglow", RegValue.DWord(colorization));

        _broadcaster.Broadcast(Win32SettingsBroadcaster.ImmersiveColorSet);
        return Task.CompletedTask;
    }

    /// <summary>"reg:&lt;key&gt;|&lt;name&gt;": the snapshot key of a tracked registry value (part of saved snapshots).</summary>
    private static string RegSnapshotKey(string key, string name) => $"{RegPrefix}{key}|{name}";

    private static void DeleteDirectoryIfExists(string path)
    {
        if (Directory.Exists(path))
            Directory.Delete(path, recursive: true);
    }

    private static string ImageExtension(string path)
    {
        // TranscodedWallpaper has no extension; Windows sniffs the content, so any image extension works.
        var ext = Path.GetExtension(path);
        return string.IsNullOrEmpty(ext) ? ".jpg" : ext;
    }
}
