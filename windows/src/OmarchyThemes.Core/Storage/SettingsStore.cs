using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Storage;

public sealed record AppSettings
{
    /// <summary>The welcome screen shows until the user continues past it once.</summary>
    public bool WelcomeSeen { get; init; }

    /// <summary>Defaults for the Apply dialog's per-aspect checkboxes.</summary>
    public ApplyOptions ApplyDefaults { get; init; } = new();

    public string? LastAppliedSlug { get; init; }
    public string? LastAppliedWallpaper { get; init; }
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
