using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core;

/// <summary>
/// The one place that decides which failures are part of normal use (the network is down, a file is
/// locked, GitHub says no) and so are reported in the UI rather than crashing the app, and how to
/// word them. A new kind of expected failure is added here, not at every catch.
/// </summary>
public static class ExpectedErrors
{
    /// <summary>Network, disk and GitHub failures the UI reports instead of crashing (not user cancellation).</summary>
    /// <param name="ct">The caller's token: a cancellation it asked for isn't a failure.</param>
    public static bool IsExpected(Exception e, CancellationToken ct = default) =>
        e is HttpRequestException or IOException or UnauthorizedAccessException
            or GitHubException or ThemeResolveException or CatalogFormatException
        || (e is TaskCanceledException && !ct.IsCancellationRequested);

    /// <summary>No connection, or it timed out: the server never answered.</summary>
    public static bool IsConnectionFailure(Exception e) =>
        e is TaskCanceledException or HttpRequestException { StatusCode: null };

    /// <summary>"Couldn't reach GitHub…" for connection failures, else the exception's own message.</summary>
    public static string Describe(Exception e) => IsConnectionFailure(e)
        ? "Couldn't reach GitHub. Check your internet connection and try again."
        : e.Message;
}
