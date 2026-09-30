using OmarchyThemes.Core.Palettes;

namespace OmarchyThemes.Core.Theming;

public enum SummaryKind
{
    Success,
    Info,
    Warning,
    Error,
}

/// <summary>Human-readable outcome of an Apply, for the UI's notification bar.</summary>
public sealed record ApplySummary(SummaryKind Kind, string Title, string Message)
{
    /// <param name="accentNote">The backend's <see cref="IDesktopBackend.AccentColorNote"/>, added when the accent color changed.</param>
    public static ApplySummary Describe(ApplyResult result, string themeName, AppearanceMode mode, string? accentNote = null)
    {
        if (result.For(ApplyStep.SaveOriginal) is { Outcome: StepOutcome.Failed, Error: var saveError })
            return new ApplySummary(SummaryKind.Error, "Theme not applied", saveError ?? "Couldn't save your current desktop.");

        var applied = result.Steps
            .Where(s => s.Outcome == StepOutcome.Applied && s.Step != ApplyStep.SaveOriginal)
            .Select(s => Label(s.Step, mode))
            .ToList();
        var failed = result.Steps.Where(s => s.Outcome == StepOutcome.Failed).ToList();
        var accentApplied = result.For(ApplyStep.AccentColor)?.Outcome == StepOutcome.Applied;

        if (failed.Count == 0)
        {
            if (applied.Count == 0)
                return new ApplySummary(SummaryKind.Info, "Nothing changed", $"All options were turned off, so {themeName} wasn't applied.");
            return new ApplySummary(
                SummaryKind.Success,
                $"{themeName} applied",
                $"Updated {JoinList(applied)}." + (accentApplied && accentNote is not null ? " " + accentNote : ""));
        }

        var problems = string.Join(" ", failed.Select(f => $"{Capitalize(Label(f.Step, mode))}: {f.Error}"));
        return applied.Count == 0
            ? new ApplySummary(SummaryKind.Error, $"Couldn't apply {themeName}", problems)
            : new ApplySummary(SummaryKind.Warning, $"{themeName} partly applied", $"Updated {JoinList(applied)}. {problems}");
    }

    private static string Label(ApplyStep step, AppearanceMode mode) => step switch
    {
        ApplyStep.Wallpaper => "the wallpaper",
        ApplyStep.AppearanceMode => mode == AppearanceMode.Light ? "light mode" : "dark mode",
        ApplyStep.AccentColor => "the accent color",
        _ => "your saved desktop",
    };

    private static string Capitalize(string s) => char.ToUpperInvariant(s[0]) + s[1..];

    internal static string JoinList(IReadOnlyList<string> items) => items.Count switch
    {
        0 => "",
        1 => items[0],
        2 => $"{items[0]} and {items[1]}",
        _ => string.Join(", ", items.Take(items.Count - 1)) + ", and " + items[^1],
    };
}
