using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

public class ApplySummaryTests
{
    private static ApplyResult Result(params (ApplyStep Step, StepOutcome Outcome, string? Error)[] steps) =>
        new(steps.Select(s => new StepResult(s.Step, s.Outcome, s.Error)).ToList());

    [Fact]
    public void Success_lists_what_changed_and_mentions_sign_out_for_accent()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.SaveOriginal, StepOutcome.Applied, null),
            (ApplyStep.Wallpaper, StepOutcome.Applied, null),
            (ApplyStep.AppearanceMode, StepOutcome.Applied, null),
            (ApplyStep.AccentColor, StepOutcome.Applied, null)), "Tokyo Night", AppearanceMode.Dark);

        Assert.Equal(SummaryKind.Success, summary.Kind);
        Assert.Equal("Tokyo Night applied", summary.Title);
        Assert.StartsWith("Updated the wallpaper, dark mode, and the accent color.", summary.Message);
        Assert.Contains("sign out", summary.Message);
    }

    [Fact]
    public void Wallpaper_only_success_has_no_sign_out_note()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.Wallpaper, StepOutcome.Applied, null),
            (ApplyStep.AppearanceMode, StepOutcome.SkippedByUser, null),
            (ApplyStep.AccentColor, StepOutcome.NotSupported, null)), "Snow", AppearanceMode.Light);

        Assert.Equal("Updated the wallpaper.", summary.Message);
    }

    [Fact]
    public void Partial_failure_is_a_warning_with_the_reason()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.Wallpaper, StepOutcome.Applied, null),
            (ApplyStep.AppearanceMode, StepOutcome.Applied, null),
            (ApplyStep.AccentColor, StepOutcome.Failed, "Access denied.")), "Snow", AppearanceMode.Light);

        Assert.Equal(SummaryKind.Warning, summary.Kind);
        Assert.Equal("Updated the wallpaper and light mode. The accent color: Access denied.", summary.Message);
    }

    [Fact]
    public void Total_failure_is_an_error()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.Wallpaper, StepOutcome.Failed, "File missing."),
            (ApplyStep.AppearanceMode, StepOutcome.SkippedByUser, null)), "Snow", AppearanceMode.Light);

        Assert.Equal(SummaryKind.Error, summary.Kind);
        Assert.Equal("Couldn't apply Snow", summary.Title);
    }

    [Fact]
    public void Snapshot_failure_explains_nothing_changed()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.SaveOriginal, StepOutcome.Failed, "Couldn't save your current desktop, so nothing was changed."),
            (ApplyStep.Wallpaper, StepOutcome.NotAttempted, null)), "Snow", AppearanceMode.Light);

        Assert.Equal(SummaryKind.Error, summary.Kind);
        Assert.Contains("nothing was changed", summary.Message);
    }

    [Fact]
    public void Everything_skipped_is_informational()
    {
        var summary = ApplySummary.Describe(Result(
            (ApplyStep.Wallpaper, StepOutcome.SkippedByUser, null),
            (ApplyStep.AppearanceMode, StepOutcome.SkippedByUser, null)), "Snow", AppearanceMode.Light);

        Assert.Equal(SummaryKind.Info, summary.Kind);
    }
}
