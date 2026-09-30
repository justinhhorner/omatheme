using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores;

/// <summary>Applying themes to the desktop, the theme currently on it, and restoring the original.</summary>
public sealed class DesktopStore
{
    private readonly ThemeApplier _applier;
    private readonly Preferences _preferences;
    private readonly ThemeLibrary _library;
    private readonly ILogger _log;

    public DesktopStore(ThemeApplier applier, Preferences preferences, ThemeLibrary library, ILogger<DesktopStore>? log = null)
    {
        _applier = applier;
        _preferences = preferences;
        _library = library;
        _log = log ?? NullLogger<DesktopStore>.Instance;
        HasOriginalSnapshot = applier.HasOriginalSnapshot;
    }

    /// <summary>
    /// Raised when an apply or restore starts or ends (and so when the current theme may have changed),
    /// on the thread that called into the store.
    /// </summary>
    public event EventHandler? Changed;

    public DesktopCapabilities Capabilities => _applier.Capabilities;

    /// <summary>The saved one-click choices (Settings), which the Apply dialog starts from too.</summary>
    public ApplyOptions ApplyDefaults => _preferences.Settings.ApplyDefaults;

    public bool HasOriginalSnapshot { get; private set; }

    /// <summary>The theme being applied right now, if any; only one apply runs at a time.</summary>
    public string? ApplyingSlug { get; private set; }

    /// <summary>The current theme's wallpaper being set from the Current theme card, for its spinner.</summary>
    public string? SettingWallpaper { get; private set; }

    // Current theme

    /// <summary>The slug of the theme on the desktop (the last one applied).</summary>
    public string? ActiveSlug => _preferences.Settings.LastAppliedSlug;

    /// <summary>Which of the current theme's wallpapers is on the desktop, if any.</summary>
    public string? CurrentWallpaper => _preferences.Settings.LastAppliedWallpaper;

    /// <summary>The theme on the desktop, if it's still downloaded (null after Restore, which clears it).</summary>
    public InstalledTheme? CurrentTheme => ActiveSlug is { } slug ? _library.Get(slug) : null;

    /// <summary>The wallpaper to preselect for a downloaded theme: the one last applied, else the first.</summary>
    public string? PreferredWallpaper(InstalledTheme theme) =>
        ActiveSlug == theme.Slug && CurrentWallpaper is { } last && theme.Wallpapers.Contains(last)
            ? last
            : theme.Wallpapers.FirstOrDefault();

    // Apply

    /// <param name="rememberOptions">Also save <paramref name="options"/> as the one-click defaults.</param>
    public async Task<ApplySummary> ApplyAsync(InstalledTheme theme, string? wallpaperFile, ApplyOptions options, bool rememberOptions = false)
    {
        if (ApplyingSlug is not null)
            return new ApplySummary(SummaryKind.Info, "Already applying a theme", "Wait for it to finish, then try again.");
        if (rememberOptions)
            _preferences.Update(s => s with { ApplyDefaults = options });

        ApplyingSlug = theme.Slug;
        OnChanged();
        try
        {
            var request = ApplyRequest.FromInstalled(theme, wallpaperFile, options);
            var result = await _applier.ApplyAsync(request);
            _log.LogInformation("Applied {Slug}: {Steps}", theme.Slug,
                string.Join(", ", result.Steps.Select(s => $"{s.Step}={s.Outcome}")));
            foreach (var failed in result.Steps.Where(s => s.Outcome == StepOutcome.Failed))
                _log.LogError("Applying {Slug}: {Step} failed: {Error}", theme.Slug, failed.Step, failed.Error);

            var file = request.WallpaperPath is null ? null : Path.GetFileName(request.WallpaperPath);
            _preferences.Update(s => s.AfterApply(theme.Slug, file, result));
            return ApplySummary.Describe(result, theme.Name, theme.Mode, _applier.AccentColorNote);
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            // ThemeApplier reports each step's failure in its result; this is anything else.
            _log.LogError(e, "Applying {Slug} failed", theme.Slug);
            return new ApplySummary(SummaryKind.Error, $"Couldn't apply {theme.Name}", e.Message);
        }
        finally
        {
            ApplyingSlug = null;
            HasOriginalSnapshot = _applier.HasOriginalSnapshot;
            OnChanged();
        }
    }

    /// <summary>One-click apply with the defaults from Settings.</summary>
    public Task<ApplySummary> ApplyWithDefaultsAsync(InstalledTheme theme) =>
        ApplyAsync(theme, PreferredWallpaper(theme), ApplyDefaults);

    /// <summary>
    /// Sets another of the current theme's wallpapers: wallpaper only (the theme's light/dark mode and
    /// accent are already applied), with the saved fit and the theme's background as fill color.
    /// Success shows as the "current" mark moving, so only a problem returns a banner.
    /// </summary>
    public async Task<Banner?> SetCurrentWallpaperAsync(string file)
    {
        if (CurrentTheme is not { } theme || file == CurrentWallpaper || ApplyingSlug is not null)
            return null;

        SettingWallpaper = file;
        try
        {
            var options = new ApplyOptions
            {
                Wallpaper = true,
                AppearanceMode = false,
                AccentColor = false,
                Fit = ApplyDefaults.Fit,
            };
            var summary = await ApplyAsync(theme, file, options);
            return summary.Kind switch
            {
                SummaryKind.Success => null,
                SummaryKind.Error => Banner.From(summary) with { Title = "Couldn't change the wallpaper" },
                _ => Banner.From(summary),
            };
        }
        finally
        {
            SettingWallpaper = null;
            OnChanged();
        }
    }

    // Restore

    /// <summary>Puts back the desktop saved before the first Apply and forgets the current theme.</summary>
    public async Task<Banner> RestoreOriginalAsync()
    {
        try
        {
            var restored = await _applier.RestoreOriginalAsync();
            _log.LogInformation("Restore original desktop: {Result}", restored ? "restored" : "nothing saved");
            if (!restored)
                return new Banner(SummaryKind.Info, "Nothing to restore", "No saved desktop was found.");

            _preferences.Update(s => s with { LastAppliedSlug = null, LastAppliedWallpaper = null });
            return new Banner(SummaryKind.Success, "Original desktop restored", "Your previous wallpaper and colors are back.");
        }
        catch (Exception e) when (e is not OperationCanceledException)
        {
            // The OS calls behind a restore can fail in more ways than ExpectedErrors lists (COM errors);
            // the user asked for it, so say it failed rather than crash.
            _log.LogError(e, "Restoring the original desktop failed");
            return new Banner(SummaryKind.Error, "Couldn't restore your desktop", e.Message);
        }
        finally
        {
            HasOriginalSnapshot = _applier.HasOriginalSnapshot;
            OnChanged();
        }
    }

    private void OnChanged() => Changed?.Invoke(this, EventArgs.Empty);
}
