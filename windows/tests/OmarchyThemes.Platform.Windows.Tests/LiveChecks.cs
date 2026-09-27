using OmarchyThemes.Platform.Windows.Registry;
using OmarchyThemes.Platform.Windows.Wallpaper;

namespace OmarchyThemes.Platform.Windows.Tests;

/// <summary>Skipped unless <c>OMATHEME_LIVE=1</c>.</summary>
public sealed class LiveFactAttribute : FactAttribute
{
    public LiveFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("OMATHEME_LIVE") != "1")
            Skip = "Live check: set OMATHEME_LIVE=1 to read the real desktop (read-only).";
    }
}

/// <summary>
/// Read-only checks against this PC's real desktop: they only read (the current wallpaper, fit, fill
/// color and the registry values the backend would snapshot) and never set anything.
/// </summary>
[Trait("Category", "Live")]
public sealed class LiveChecks
{
    [LiveFact]
    public async Task Reads_the_current_wallpaper_through_IDesktopWallpaper()
    {
        var state = await new DesktopWallpaperApi().GetAsync();

        Assert.NotEmpty(state.Monitors);
        Assert.All(state.Monitors, m => Assert.False(string.IsNullOrEmpty(m.MonitorId)));
        Assert.NotNull(state.Fit);
        Assert.NotNull(state.Background);
    }

    [LiveFact]
    public void Reads_every_tracked_registry_value()
    {
        var registry = new CurrentUserRegistry();

        foreach (var (key, name) in WindowsDesktopBackend.TrackedValues)
        {
            // Absent is fine; anything present must survive a snapshot round trip.
            var value = registry.Read(key, name);
            Assert.Equal(value, RegValue.Deserialize(RegValue.Serialize(value)));
        }
    }
}
