using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;
using System.Text.Json.Serialization;
using OmarchyThemes.Core.Net;

namespace OmarchyThemes.Core.GitHub;

/// <summary>
/// Minimal GitHub access for theme repos. Uses exactly one REST call per repo (the recursive
/// tree) and fetches file contents from raw.githubusercontent.com, which isn't API-metered.
/// Everything goes through <see cref="HttpCache"/> so revisits are free and work offline.
/// </summary>
public sealed class GitHubClient
{
    /// <summary>Repo contents are re-checked at most this often (ETag revalidation after that).</summary>
    public static readonly TimeSpan FreshFor = TimeSpan.FromHours(1);

    private readonly HttpCache _cache;
    private readonly string? _token;

    public GitHubClient(HttpCache cache, string? token = null)
    {
        _cache = cache;
        _token = string.IsNullOrWhiteSpace(token) ? null : token.Trim();
    }

    public async Task<RepoTree> GetTreeAsync(RepoRef repo, CancellationToken ct = default)
    {
        var response = await _cache.GetAsync(TreeUri(repo), new CacheOptions
        {
            MaxAge = FreshFor,
            ConfigureRequest = r =>
            {
                // GitHub rejects API requests without a User-Agent.
                r.Headers.UserAgent.ParseAdd(AppInfo.UserAgent);
                r.Headers.Accept.Add(new MediaTypeWithQualityHeaderValue("application/vnd.github+json"));
                r.Headers.Add("X-GitHub-Api-Version", "2022-11-28");
                if (_token is not null)
                    r.Headers.Authorization = new AuthenticationHeaderValue("Bearer", _token);
            },
            MapError = r => MapApiError(r, repo),
        }, ct).ConfigureAwait(false);

        return ParseTree(response, repo);
    }

    /// <summary>The cached file list for <paramref name="repo"/> without touching the network, or null.</summary>
    public RepoTree? TryGetCachedTree(RepoRef repo)
    {
        var cached = _cache.TryGetCached(TreeUri(repo));
        if (cached is null)
            return null;
        try
        {
            return ParseTree(cached, repo);
        }
        catch (GitHubException)
        {
            return null;
        }
    }

    private static Uri TreeUri(RepoRef repo) =>
        new($"https://api.github.com/repos/{Esc(repo.Owner)}/{Esc(repo.Name)}/git/trees/{Esc(repo.EffectiveRef)}?recursive=1");

    private static RepoTree ParseTree(CachedResponse response, RepoRef repo)
    {
        TreeResponse? tree;
        try
        {
            tree = JsonSerializer.Deserialize<TreeResponse>(response.Body);
        }
        catch (JsonException e)
        {
            throw new GitHubException($"GitHub returned an unreadable file list for {repo.FullName}.", inner: e);
        }
        if (tree?.Tree is null)
            throw new GitHubException($"GitHub returned an unreadable file list for {repo.FullName}.");

        var prefix = repo.SubPath is null ? null : repo.SubPath.TrimEnd('/') + "/";
        var items = tree.Tree
            .Where(i => i.Path is not null && (prefix is null || i.Path.StartsWith(prefix, StringComparison.Ordinal)))
            .Select(i => new RepoTreeItem(prefix is null ? i.Path! : i.Path![prefix.Length..], i.Type == "blob", i.Size))
            .ToList();

        return new RepoTree(items, tree.Truncated, response.IsStale, response.Error);
    }

    /// <summary>raw.githubusercontent.com URL for a path relative to the theme root.</summary>
    public static Uri RawUrl(RepoRef repo, string themeRelativePath)
    {
        var path = string.Join('/', repo.RepoPath(themeRelativePath).Split('/').Select(Esc));
        return new Uri($"https://raw.githubusercontent.com/{Esc(repo.Owner)}/{Esc(repo.Name)}/{Esc(repo.EffectiveRef)}/{path}");
    }

    public async Task<string> GetRawTextAsync(RepoRef repo, string themeRelativePath, CancellationToken ct = default)
    {
        var response = await _cache.GetAsync(RawUrl(repo, themeRelativePath), new CacheOptions
        {
            MaxAge = FreshFor,
            MapError = r => r.StatusCode == HttpStatusCode.NotFound
                ? new GitHubException($"{themeRelativePath} wasn't found in {repo.FullName}.", HttpStatusCode.NotFound)
                : null,
        }, ct).ConfigureAwait(false);
        return response.Text;
    }

    internal static Exception MapApiError(HttpResponseMessage response, RepoRef repo)
    {
        var remaining = Header(response, "x-ratelimit-remaining");
        if (response.StatusCode == HttpStatusCode.TooManyRequests
            || (response.StatusCode == HttpStatusCode.Forbidden && remaining == "0"))
        {
            DateTimeOffset? resetsAt = long.TryParse(Header(response, "x-ratelimit-reset"), out var epoch)
                ? DateTimeOffset.FromUnixTimeSeconds(epoch)
                : null;
            return new GitHubRateLimitException(resetsAt);
        }

        // 409 = empty repository.
        if (response.StatusCode is HttpStatusCode.NotFound or HttpStatusCode.Conflict)
            return new GitHubNotFoundException(repo.FullName);

        return new GitHubException(
            $"GitHub returned {(int)response.StatusCode} {response.ReasonPhrase} for {repo.FullName}.",
            response.StatusCode);
    }

    private static string? Header(HttpResponseMessage response, string name) =>
        response.Headers.TryGetValues(name, out var values) ? values.FirstOrDefault() : null;

    private static string Esc(string s) => Uri.EscapeDataString(s);

    private sealed record TreeResponse(
        [property: JsonPropertyName("tree")] List<TreeItem>? Tree,
        [property: JsonPropertyName("truncated")] bool Truncated);

    private sealed record TreeItem(
        [property: JsonPropertyName("path")] string? Path,
        [property: JsonPropertyName("type")] string? Type,
        [property: JsonPropertyName("size")] long? Size);
}

public sealed record RepoTreeItem(string Path, bool IsFile, long? Size)
{
    public string FileName => System.IO.Path.GetFileName(Path);
}

/// <param name="Items">Paths relative to the theme root (the repo's sub-folder if the link had one).</param>
/// <param name="IsStale">True when served from cache because GitHub couldn't be reached.</param>
public sealed record RepoTree(IReadOnlyList<RepoTreeItem> Items, bool Truncated, bool IsStale = false, Exception? StaleReason = null)
{
    public IEnumerable<RepoTreeItem> Files => Items.Where(i => i.IsFile);

    public RepoTreeItem? FindFile(string path) =>
        Files.FirstOrDefault(f => string.Equals(f.Path, path, StringComparison.OrdinalIgnoreCase));
}
