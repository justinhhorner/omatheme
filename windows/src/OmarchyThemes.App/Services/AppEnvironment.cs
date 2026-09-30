using OmarchyThemes.Core;

namespace OmarchyThemes.App.Services;

/// <summary>
/// Test switches, read once at startup (same names as the macOS app):
/// <list type="bullet">
/// <item><c>OMARCHY_THEMES_DRY_RUN=1</c>: a desktop backend that changes nothing, with a badge in the title bar.</item>
/// <item><c>OMARCHY_THEMES_DATA_DIR=&lt;path&gt;</c>: another data folder instead of %LOCALAPPDATA%\OmarchyThemes
/// (fresh first-launch state, no risk to real downloads or the saved original desktop).</item>
/// </list>
/// Use both whenever the UI is driven by a script.
/// </summary>
/// <param name="GitHubToken"><c>GITHUB_TOKEN</c>, which raises GitHub's API limit (60 requests an hour without one).</param>
public sealed record AppEnvironment(bool IsDryRun, string? DataDir, string? GitHubToken = null)
{
    public static AppEnvironment FromProcess()
    {
        var dataDir = Environment.GetEnvironmentVariable("OMARCHY_THEMES_DATA_DIR");
        var token = Environment.GetEnvironmentVariable("GITHUB_TOKEN");
        return new AppEnvironment(
            Environment.GetEnvironmentVariable("OMARCHY_THEMES_DRY_RUN") == "1",
            string.IsNullOrWhiteSpace(dataDir) ? null : dataDir,
            string.IsNullOrWhiteSpace(token) ? null : token);
    }

    public AppPaths Paths => DataDir is null ? AppPaths.Default() : new AppPaths(DataDir);

    /// <summary>Shown as the title bar subtitle so a test session can't be mistaken for a real one.</summary>
    public string? Badge => (IsDryRun, DataDir is not null) switch
    {
        (true, true) => "Dry run · test data folder",
        (true, false) => "Dry run",
        (false, true) => "Test data folder",
        _ => null,
    };
}
