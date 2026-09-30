using System.Net;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests;

public sealed class ExpectedErrorsTests
{
    public static TheoryData<Exception> Expected() =>
    [
        new HttpRequestException("No network"),
        new IOException("The file is in use."),
        new UnauthorizedAccessException("Access denied."),
        new GitHubNotFoundException("o/r"),
        new GitHubRateLimitException(null),
        new ThemeResolveException("No palette."),
        new CatalogFormatException("No themes."),
        new SnapshotUnreadableException("Can't read it."),
        new TaskCanceledException("Timed out."),
    ];

    [Theory]
    [MemberData(nameof(Expected))]
    public void Network_disk_and_github_failures_are_expected(Exception e) =>
        Assert.True(ExpectedErrors.IsExpected(e));

    [Fact]
    public void Programming_errors_are_not_expected()
    {
        Assert.False(ExpectedErrors.IsExpected(new NullReferenceException()));
        Assert.False(ExpectedErrors.IsExpected(new InvalidOperationException()));
    }

    [Fact]
    public void A_cancellation_the_caller_asked_for_is_not_a_failure()
    {
        using var cts = new CancellationTokenSource();
        cts.Cancel();

        Assert.False(ExpectedErrors.IsExpected(new TaskCanceledException(), cts.Token));
        Assert.False(ExpectedErrors.IsExpected(new OperationCanceledException()));
    }

    [Fact]
    public void Connection_failures_get_one_message_and_everything_else_keeps_its_own()
    {
        const string unreachable = "Couldn't reach GitHub. Check your internet connection and try again.";
        Assert.Equal(unreachable, ExpectedErrors.Describe(new HttpRequestException("No such host is known.")));
        Assert.Equal(unreachable, ExpectedErrors.Describe(new TaskCanceledException()));

        var serverError = new HttpRequestException("api.github.com returned 500 Internal Server Error.", null, HttpStatusCode.InternalServerError);
        Assert.Equal(serverError.Message, ExpectedErrors.Describe(serverError));
        Assert.Equal("The file is in use.", ExpectedErrors.Describe(new IOException("The file is in use.")));
    }
}
