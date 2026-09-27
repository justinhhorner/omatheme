using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Core.Theming;

/// <summary>The user's per-aspect choices in the Apply dialog.</summary>
public sealed record ApplyOptions
{
    public bool Wallpaper { get; init; } = true;
    public bool AppearanceMode { get; init; } = true;
    public bool AccentColor { get; init; } = true;
    public WallpaperFit Fit { get; init; } = WallpaperFit.Fill;
}

/// <summary>What to apply. Built from a downloaded theme, so no network is involved.</summary>
public sealed record ApplyRequest(string ThemeName, string? WallpaperPath, AppearanceMode Mode, RgbColor? Accent, ApplyOptions Options)
{
    /// <param name="wallpaperFile">One of <see cref="InstalledTheme.Wallpapers"/>; defaults to the first.</param>
    public static ApplyRequest FromInstalled(InstalledTheme theme, string? wallpaperFile, ApplyOptions options)
    {
        var file = wallpaperFile is not null && theme.Wallpapers.Contains(wallpaperFile)
            ? wallpaperFile
            : theme.Wallpapers.FirstOrDefault();
        return new ApplyRequest(theme.Name, file is null ? null : theme.WallpaperPath(file), theme.Mode, theme.Palette?.Accent, options);
    }
}

public enum ApplyStep
{
    SaveOriginal,
    Wallpaper,
    AppearanceMode,
    AccentColor,
}

public enum StepOutcome
{
    Applied,
    /// <summary>The user unchecked it.</summary>
    SkippedByUser,
    /// <summary>This OS/backend can't change it.</summary>
    NotSupported,
    /// <summary>The theme has nothing for it (e.g. no wallpaper or palette).</summary>
    NoData,
    /// <summary>An earlier step failed in a way that made continuing unsafe.</summary>
    NotAttempted,
    Failed,
}

public sealed record StepResult(ApplyStep Step, StepOutcome Outcome, string? Error = null);

public sealed record ApplyResult(IReadOnlyList<StepResult> Steps)
{
    public bool AnyApplied => Steps.Any(s => s.Outcome == StepOutcome.Applied);
    public bool AnyFailed => Steps.Any(s => s.Outcome == StepOutcome.Failed);
    public bool Succeeded => AnyApplied && !AnyFailed;

    public StepResult? For(ApplyStep step) => Steps.FirstOrDefault(s => s.Step == step);
}

public interface ISnapshotStore
{
    DesktopSnapshot? Load();
    void Save(DesktopSnapshot snapshot);
    void Clear();
}

public sealed class FileSnapshotStore(AppPaths paths) : ISnapshotStore
{
    public DesktopSnapshot? Load() => JsonFile.TryRead<DesktopSnapshot>(paths.SnapshotFile);

    public void Save(DesktopSnapshot snapshot) => JsonFile.WriteAtomic(paths.SnapshotFile, snapshot);

    public void Clear() => File.Delete(paths.SnapshotFile);
}

/// <summary>
/// Applies a theme through an <see cref="IDesktopBackend"/>. Before the first change it saves
/// the user's current desktop (so "Restore my original desktop" can undo everything), then runs
/// each step independently: one failing step (e.g. accent) doesn't stop the others, and the
/// result reports exactly what happened per step.
/// </summary>
public sealed class ThemeApplier
{
    private readonly IDesktopBackend _backend;
    private readonly ISnapshotStore _snapshots;
    private readonly SemaphoreSlim _gate = new(1, 1);

    public ThemeApplier(IDesktopBackend backend, ISnapshotStore snapshots)
    {
        _backend = backend;
        _snapshots = snapshots;
    }

    public DesktopCapabilities Capabilities => _backend.Capabilities;

    public bool HasOriginalSnapshot => _snapshots.Load() is not null;

    public async Task<ApplyResult> ApplyAsync(ApplyRequest request, IProgress<ApplyStep>? progress = null, CancellationToken ct = default)
    {
        await _gate.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            var results = new List<StepResult>();

            progress?.Report(ApplyStep.SaveOriginal);
            if (_snapshots.Load() is null)
            {
                try
                {
                    _snapshots.Save(await _backend.CaptureAsync(ct).ConfigureAwait(false));
                    results.Add(new StepResult(ApplyStep.SaveOriginal, StepOutcome.Applied));
                }
                catch (Exception e) when (e is not OperationCanceledException)
                {
                    // Without a snapshot we couldn't undo, so change nothing.
                    results.Add(new StepResult(ApplyStep.SaveOriginal, StepOutcome.Failed,
                        $"Couldn't save your current desktop, so nothing was changed. {e.Message}"));
                    results.AddRange(new[] { ApplyStep.Wallpaper, ApplyStep.AppearanceMode, ApplyStep.AccentColor }
                        .Select(s => new StepResult(s, StepOutcome.NotAttempted)));
                    return new ApplyResult(results);
                }
            }

            var options = request.Options;

            results.Add(await RunStepAsync(ApplyStep.Wallpaper, options.Wallpaper, DesktopCapabilities.Wallpaper,
                hasData: request.WallpaperPath is not null,
                () =>
                {
                    if (!File.Exists(request.WallpaperPath))
                        throw new FileNotFoundException("The wallpaper file is missing. Try downloading the theme again.", request.WallpaperPath);
                    return _backend.SetWallpaperAsync(request.WallpaperPath!, options.Fit, ct);
                }, progress, ct).ConfigureAwait(false));

            results.Add(await RunStepAsync(ApplyStep.AppearanceMode, options.AppearanceMode, DesktopCapabilities.AppearanceMode,
                hasData: true,
                () => _backend.SetAppearanceModeAsync(request.Mode, ct), progress, ct).ConfigureAwait(false));

            results.Add(await RunStepAsync(ApplyStep.AccentColor, options.AccentColor, DesktopCapabilities.AccentColor,
                hasData: request.Accent is not null,
                () => _backend.SetAccentColorAsync(request.Accent!.Value, ct), progress, ct).ConfigureAwait(false));

            return new ApplyResult(results);
        }
        finally
        {
            _gate.Release();
        }
    }

    private async Task<StepResult> RunStepAsync(
        ApplyStep step, bool enabled, DesktopCapabilities capability, bool hasData,
        Func<Task> action, IProgress<ApplyStep>? progress, CancellationToken ct)
    {
        if (!enabled)
            return new StepResult(step, StepOutcome.SkippedByUser);
        if (!_backend.Capabilities.HasFlag(capability))
            return new StepResult(step, StepOutcome.NotSupported);
        if (!hasData)
            return new StepResult(step, StepOutcome.NoData);

        ct.ThrowIfCancellationRequested();
        progress?.Report(step);
        try
        {
            await action().ConfigureAwait(false);
            return new StepResult(step, StepOutcome.Applied);
        }
        catch (Exception e) when (e is not OperationCanceledException)
        {
            return new StepResult(step, StepOutcome.Failed, e.Message);
        }
    }

    /// <summary>Puts back the desktop saved before the first Apply, then forgets the snapshot.</summary>
    public async Task<bool> RestoreOriginalAsync(CancellationToken ct = default)
    {
        await _gate.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            if (_snapshots.Load() is not { } snapshot)
                return false;
            await _backend.RestoreAsync(snapshot, ct).ConfigureAwait(false);
            _snapshots.Clear();
            return true;
        }
        finally
        {
            _gate.Release();
        }
    }
}
