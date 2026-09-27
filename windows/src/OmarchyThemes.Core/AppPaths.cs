namespace OmarchyThemes.Core;

/// <summary>
/// On-disk layout for everything the app stores locally. The app runs unpackaged, so it
/// uses %LOCALAPPDATA%\OmarchyThemes rather than packaged ApplicationData.
/// </summary>
public sealed class AppPaths
{
    public AppPaths(string root)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(root);
        Root = Path.GetFullPath(root);
    }

    public static AppPaths Default() => new(Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "OmarchyThemes"));

    public string Root { get; }

    /// <summary>HTTP/API response cache (catalog, GitHub trees, ETags).</summary>
    public string CacheDir => Path.Combine(Root, "cache");

    /// <summary>Downloaded themes, one folder per theme slug.</summary>
    public string ThemesDir => Path.Combine(Root, "themes");

    public string SettingsFile => Path.Combine(Root, "settings.json");

    /// <summary>Daily log files (errors and notable events), kept for a week.</summary>
    public string LogsDir => Path.Combine(Root, "logs");

    /// <summary>Snapshot of the user's desktop taken before the first Apply, for "Restore".</summary>
    public string SnapshotFile => Path.Combine(Root, "original-desktop.json");

    /// <summary>Folder for one downloaded theme. Rejects slugs that could escape <see cref="ThemesDir"/>.</summary>
    public string ThemeDir(string slug)
    {
        if (!IsValidSlug(slug))
            throw new ArgumentException($"Invalid theme slug '{slug}'.", nameof(slug));
        return Path.Combine(ThemesDir, slug);
    }

    public void EnsureCreated()
    {
        Directory.CreateDirectory(CacheDir);
        Directory.CreateDirectory(ThemesDir);
    }

    /// <summary>True for slugs that are safe as a single folder or file name.</summary>
    public static bool IsValidSlug(string? slug) =>
        !string.IsNullOrEmpty(slug)
        && slug.Length <= 100
        && slug is not "." and not ".."
        && slug.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_' or '.');
}
