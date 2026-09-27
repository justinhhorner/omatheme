using System.Net;

namespace OmarchyThemes.Core.GitHub;

public class GitHubException(string message, HttpStatusCode? status = null, Exception? inner = null)
    : Exception(message, inner)
{
    public HttpStatusCode? Status { get; } = status;
}

/// <summary>The repository doesn't exist, was made private, or is empty.</summary>
public sealed class GitHubNotFoundException(string repo)
    : GitHubException($"The GitHub repository {repo} couldn't be found. It may have been renamed, deleted or made private.", HttpStatusCode.NotFound)
{
    public string Repo { get; } = repo;
}

/// <summary>The (unauthenticated: 60/hour) API rate limit is exhausted.</summary>
public sealed class GitHubRateLimitException(DateTimeOffset? resetsAt)
    : GitHubException(resetsAt is { } r
        ? $"GitHub's rate limit was reached. It resets at {r.ToLocalTime():t}."
        : "GitHub's rate limit was reached. Try again in a few minutes.", HttpStatusCode.Forbidden)
{
    public DateTimeOffset? ResetsAt { get; } = resetsAt;
}
