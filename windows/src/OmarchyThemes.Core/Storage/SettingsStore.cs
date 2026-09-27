using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Storage;

public sealed record AppSettings
{
    /// <summary>The welcome screen shows until the user continues past it once.</summary>
    public bool WelcomeSeen { get; init; }

    /// <summary>Defaults for the Apply dialog's per-aspect checkboxes.</summary>
    public ApplyOptions ApplyDefaults { get; init; } = new();

    /// <summary>The theme on the desktop (the last one applied), or null.</summary>
    public string? LastAppliedSlug { get; init; }

    /// <summary>Which of that theme's wallpapers is on the desktop, or null if none of them is.</summary>
    public string? LastAppliedWallpaper { get; init; }

    /// <summary>
    /// The settings after applying <paramref name="slug"/>. The theme becomes current if any step
    /// applied; its wallpaper only if the wallpaper step itself applied (with the wallpaper unchecked,
    /// or failing, the desktop keeps showing whatever it showed before).
    /// </summary>
    public AppSettings AfterApply(string slug, string? wallpaperFile, ApplyResult result)
    {
        if (!result.AnyApplied)
            return this;
        var wallpaperApplied = result.For(ApplyStep.Wallpaper)?.Outcome == StepOutcome.Applied;
        return this with
        {
            LastAppliedSlug = slug,
            LastAppliedWallpaper = wallpaperApplied ? wallpaperFile
                : LastAppliedSlug == slug ? LastAppliedWallpaper
                : null,
        };
    }
}

public sealed class SettingsStore(AppPaths paths)
{
    public AppSettings Load() => JsonFile.TryRead<AppSettings>(paths.SettingsFile) ?? new AppSettings();

    public void Save(AppSettings settings) => JsonFile.WriteAtomic(paths.SettingsFile, settings);

    public AppSettings Update(Func<AppSettings, AppSettings> change)
    {
        var updated = change(Load());
        Save(updated);
        return updated;
    }
}
