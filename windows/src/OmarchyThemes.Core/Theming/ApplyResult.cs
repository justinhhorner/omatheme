namespace OmarchyThemes.Core.Theming;

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
