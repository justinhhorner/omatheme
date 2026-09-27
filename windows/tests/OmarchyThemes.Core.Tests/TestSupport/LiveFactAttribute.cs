namespace OmarchyThemes.Core.Tests.TestSupport;

/// <summary>
/// A test against the real omarchy.org / GitHub, skipped unless <c>OMATHEME_LIVE=1</c>. Live checks
/// are read-only and keep to a handful of GitHub API calls (60/hour unauthenticated).
/// Run with: <c>$env:OMATHEME_LIVE=1; dotnet test --filter Category=Live</c>
/// </summary>
public sealed class LiveFactAttribute : FactAttribute
{
    public LiveFactAttribute()
    {
        if (Environment.GetEnvironmentVariable("OMATHEME_LIVE") != "1")
            Skip = "Live check: set OMATHEME_LIVE=1 to run against omarchy.org and GitHub.";
    }
}
