namespace OmarchyThemes.Core.Theming;

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
                    ApplyStep[] notAttempted = [ApplyStep.Wallpaper, ApplyStep.AppearanceMode, ApplyStep.AccentColor];
                    results.AddRange(notAttempted.Select(s => new StepResult(s, StepOutcome.NotAttempted)));
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
                    return _backend.SetWallpaperAsync(request.WallpaperPath!, options.Fit, request.Background, ct);
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
