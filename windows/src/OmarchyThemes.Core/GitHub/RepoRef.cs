using System.Diagnostics.CodeAnalysis;

namespace OmarchyThemes.Core.GitHub;

/// <summary>
/// A GitHub repository, optionally pinned to a branch/tag and a sub-folder
/// (from links like https://github.com/owner/repo/tree/main/themes/foo).
/// </summary>
public sealed record RepoRef(string Owner, string Name, string? Ref = null, string? SubPath = null)
{
    private static readonly HashSet<string> ReservedOwners = new(StringComparer.OrdinalIgnoreCase)
    {
        "orgs", "settings", "sponsors", "topics", "marketplace", "explore", "features", "login", "search", "about",
    };

    public string FullName => $"{Owner}/{Name}";

    /// <summary>Git ref used for API and raw URLs; HEAD resolves to the default branch.</summary>
    public string EffectiveRef => Ref ?? "HEAD";

    public Uri HtmlUrl => SubPath is null && Ref is null
        ? new Uri($"https://github.com/{Owner}/{Name}")
        : new Uri($"https://github.com/{Owner}/{Name}/tree/{EffectiveRef}/{SubPath}".TrimEnd('/'));

    /// <summary>Maps a path relative to the theme root to a path relative to the repo root.</summary>
    public string RepoPath(string themeRelativePath) =>
        SubPath is null ? themeRelativePath : $"{SubPath}/{themeRelativePath}";

    public static bool TryParse([NotNullWhen(true)] string? url, [NotNullWhen(true)] out RepoRef? repo)
    {
        repo = null;
        if (!Uri.TryCreate(url?.Trim(), UriKind.Absolute, out var uri)
            || uri.Scheme is not ("https" or "http")
            || !(uri.Host.Equals("github.com", StringComparison.OrdinalIgnoreCase)
                 || uri.Host.Equals("www.github.com", StringComparison.OrdinalIgnoreCase)))
            return false;

        var segments = uri.AbsolutePath.Split('/', StringSplitOptions.RemoveEmptyEntries)
            .Select(Uri.UnescapeDataString)
            .ToArray();
        if (segments.Length < 2)
            return false;

        var owner = segments[0];
        var name = segments[1].EndsWith(".git", StringComparison.OrdinalIgnoreCase) ? segments[1][..^4] : segments[1];
        if (!IsValidName(owner) || !IsValidName(name) || ReservedOwners.Contains(owner))
            return false;

        if (segments.Length == 2)
        {
            repo = new RepoRef(owner, name);
            return true;
        }

        // Only /tree/<ref>[/<sub/path>] is a repo view we understand; /compare, /issues etc. are not themes.
        if (segments.Length >= 4 && segments[2] == "tree")
        {
            var subPath = segments.Length > 4 ? string.Join('/', segments[4..]) : null;
            if (subPath is not null && segments[4..].Any(s => s is "." or ".."))
                return false;
            repo = new RepoRef(owner, name, segments[3], subPath);
            return true;
        }

        return false;
    }

    private static bool IsValidName(string s) =>
        s.Length is > 0 and <= 100
        && s is not "." and not ".."
        && s.All(c => char.IsAsciiLetterOrDigit(c) || c is '-' or '_' or '.');
}
