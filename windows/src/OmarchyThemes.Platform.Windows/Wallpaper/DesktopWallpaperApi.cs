using System.Runtime.InteropServices;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Theming;
using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.Shell;
using Windows.Win32.UI.WindowsAndMessaging;

namespace OmarchyThemes.Platform.Windows.Wallpaper;

public sealed record MonitorWallpaper(string MonitorId, string Path);

/// <param name="Background">The desktop fill color shown around Fit/Center wallpapers (system-wide).</param>
public sealed record WallpaperState(IReadOnlyList<MonitorWallpaper> Monitors, WallpaperFit? Fit, RgbColor? Background = null);

public interface IWallpaperApi
{
    Task<WallpaperState> GetAsync();

    /// <summary>Sets <paramref name="path"/> on every monitor, and the fill color around it when given.</summary>
    Task SetAsync(string path, WallpaperFit fit, RgbColor? background);

    /// <summary>Puts back per-monitor wallpapers. An empty path means "no picture" (solid color).</summary>
    Task RestoreAsync(WallpaperState state);
}

/// <summary>
/// Wallpaper via the shell's IDesktopWallpaper COM API (per-monitor, with position), falling back
/// to SystemParametersInfo(SPI_SETDESKWALLPAPER) if the COM object is unavailable.
/// COM calls run on a dedicated STA thread.
/// </summary>
public sealed class DesktopWallpaperApi : IWallpaperApi
{
    /// <summary>Monitor id used for the single SystemParametersInfo wallpaper.</summary>
    internal const string AllMonitors = "*";

    public Task<WallpaperState> GetAsync() => Sta.RunAsync(() =>
    {
        if (TryCreate() is not { } wallpaper)
            return new WallpaperState([new MonitorWallpaper(AllMonitors, GetSystemParametersWallpaper())], null);
        try
        {
            wallpaper.GetMonitorDevicePathCount(out var count);
            var monitors = new List<MonitorWallpaper>();
            for (uint i = 0; i < count; i++)
            {
                wallpaper.GetMonitorDevicePathAt(i, out var idPtr);
                var id = TakeString(idPtr);
                if (string.IsNullOrEmpty(id))
                    continue;
                wallpaper.GetWallpaper(id, out var pathPtr);
                monitors.Add(new MonitorWallpaper(id, TakeString(pathPtr) ?? ""));
            }
            wallpaper.GetPosition(out var position);
            wallpaper.GetBackgroundColor(out var background);
            return new WallpaperState(monitors, FromPosition(position), FromColorRef(background));
        }
        finally
        {
            Marshal.ReleaseComObject(wallpaper);
        }
    });

    public Task SetAsync(string path, WallpaperFit fit, RgbColor? background) => Sta.RunAsync(() =>
    {
        if (TryCreate() is { } wallpaper)
        {
            try
            {
                if (background is { } color)
                    wallpaper.SetBackgroundColor(ToColorRef(color));
                wallpaper.SetPosition(ToPosition(fit));
                wallpaper.SetWallpaper((string?)null!, path); // null monitor = all monitors
                return true;
            }
            catch (COMException)
            {
                // Fall through to SystemParametersInfo.
            }
            finally
            {
                Marshal.ReleaseComObject(wallpaper);
            }
        }
        SetSystemParametersWallpaper(path);
        return true;
    });

    public Task RestoreAsync(WallpaperState state) => Sta.RunAsync(() =>
    {
        var wallpaper = TryCreate();
        try
        {
            if (wallpaper is not null && state.Background is { } background)
                wallpaper.SetBackgroundColor(ToColorRef(background));
            if (wallpaper is not null && state.Fit is { } fit)
                wallpaper.SetPosition(ToPosition(fit));

            foreach (var monitor in state.Monitors)
            {
                // "No picture" and the SPI fallback can only be expressed through SystemParametersInfo.
                if (wallpaper is null || monitor.MonitorId == AllMonitors || monitor.Path.Length == 0)
                {
                    SetSystemParametersWallpaper(monitor.Path);
                    continue;
                }
                try
                {
                    wallpaper.SetWallpaper(monitor.MonitorId, monitor.Path);
                }
                catch (COMException)
                {
                    // The monitor was disconnected since the snapshot; skip it.
                }
            }
            return true;
        }
        finally
        {
            if (wallpaper is not null)
                Marshal.ReleaseComObject(wallpaper);
        }
    });

    private static IDesktopWallpaper? TryCreate()
    {
        try
        {
            return (IDesktopWallpaper)new DesktopWallpaper();
        }
        catch (Exception e) when (e is COMException or InvalidCastException)
        {
            return null;
        }
    }

    private static unsafe string? TakeString(PWSTR value)
    {
        if (value.Value is null)
            return null;
        try
        {
            return value.ToString();
        }
        finally
        {
            PInvoke.CoTaskMemFree(value.Value);
        }
    }

    private static unsafe void SetSystemParametersWallpaper(string path)
    {
        fixed (char* p = path)
        {
            if (!PInvoke.SystemParametersInfo(
                    SYSTEM_PARAMETERS_INFO_ACTION.SPI_SETDESKWALLPAPER, 0, p,
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS.SPIF_UPDATEINIFILE | SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS.SPIF_SENDCHANGE))
                throw new InvalidOperationException($"Windows refused to set the wallpaper (error {Marshal.GetLastPInvokeError()}).");
        }
    }

    private static unsafe string GetSystemParametersWallpaper()
    {
        var buffer = new char[1024];
        fixed (char* p = buffer)
        {
            if (!PInvoke.SystemParametersInfo(SYSTEM_PARAMETERS_INFO_ACTION.SPI_GETDESKWALLPAPER, (uint)buffer.Length, p, 0))
                return "";
            return new string(p);
        }
    }

    /// <summary>COLORREF is 0x00BBGGRR: ABGR with a zero alpha byte.</summary>
    internal static COLORREF ToColorRef(RgbColor color) => (COLORREF)AccentMath.ToAbgr(color, alpha: 0);

    internal static RgbColor FromColorRef(COLORREF value) => AccentMath.FromAbgr(value);

    internal static DESKTOP_WALLPAPER_POSITION ToPosition(WallpaperFit fit) => fit switch
    {
        WallpaperFit.Fill => DESKTOP_WALLPAPER_POSITION.DWPOS_FILL,
        WallpaperFit.Fit => DESKTOP_WALLPAPER_POSITION.DWPOS_FIT,
        WallpaperFit.Stretch => DESKTOP_WALLPAPER_POSITION.DWPOS_STRETCH,
        WallpaperFit.Center => DESKTOP_WALLPAPER_POSITION.DWPOS_CENTER,
        WallpaperFit.Tile => DESKTOP_WALLPAPER_POSITION.DWPOS_TILE,
        WallpaperFit.Span => DESKTOP_WALLPAPER_POSITION.DWPOS_SPAN,
        _ => DESKTOP_WALLPAPER_POSITION.DWPOS_FILL,
    };

    internal static WallpaperFit? FromPosition(DESKTOP_WALLPAPER_POSITION position) => position switch
    {
        DESKTOP_WALLPAPER_POSITION.DWPOS_FILL => WallpaperFit.Fill,
        DESKTOP_WALLPAPER_POSITION.DWPOS_FIT => WallpaperFit.Fit,
        DESKTOP_WALLPAPER_POSITION.DWPOS_STRETCH => WallpaperFit.Stretch,
        DESKTOP_WALLPAPER_POSITION.DWPOS_CENTER => WallpaperFit.Center,
        DESKTOP_WALLPAPER_POSITION.DWPOS_TILE => WallpaperFit.Tile,
        DESKTOP_WALLPAPER_POSITION.DWPOS_SPAN => WallpaperFit.Span,
        _ => null,
    };
}

/// <summary>Runs a function on a fresh STA thread (the shell's COM objects expect one).</summary>
internal static class Sta
{
    public static Task<T> RunAsync<T>(Func<T> func)
    {
        var tcs = new TaskCompletionSource<T>(TaskCreationOptions.RunContinuationsAsynchronously);
        var thread = new Thread(() =>
        {
            try { tcs.SetResult(func()); }
            catch (Exception e) { tcs.SetException(e); }
        })
        {
            IsBackground = true,
            Name = "OmarchyThemes STA",
        };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        return tcs.Task;
    }
}
