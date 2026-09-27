using System.Text;
using AngleSharp.Dom;
using AngleSharp.Html.Parser;
using OmarchyThemes.Core.GitHub;

namespace OmarchyThemes.Core.Catalog;

/// <summary>One theme card from omarchy.org/themes.</summary>
/// <param name="Slug">Stable local id (screenshot file stem, else derived from the repo name).</param>
public sealed record CatalogEntry(string Slug, string Name, string RepoUrl, string? ScreenshotUrl)
{
    /// <summary>One of the themes that ship with Omarchy (see <see cref="DefaultThemes"/>).</summary>
    public bool IsDefaultTheme => DefaultThemes.IsDefault(Slug);

    /// <summary>"owner/repo" for community themes; default themes all live in Omarchy's repo.</summary>
    public string RepoDisplay => IsDefaultTheme
        ? "Included with Omarchy"
        : RepoRef.TryParse(RepoUrl, out var repo) ? repo.FullName : RepoUrl;
}

/// <summary>
/// Parses the omarchy.org/themes gallery. The live page (an Astro build) renders each theme as
/// <c>&lt;li&gt;&lt;a href="https://github.com/…"&gt;&lt;img src="/assets/themes/x.webp"&gt;&lt;span&gt;Name&lt;/span&gt;&lt;/a&gt;&lt;/li&gt;</c>.
/// Rather than depend on classes or exact nesting, it looks for GitHub repo links that have a
/// screenshot, either inside the link or in the same <c>li</c>/<c>figure</c>. That also covers
/// a <c>&lt;figure&gt;&lt;img&gt;&lt;figcaption&gt;&lt;a&gt;</c> layout.
/// </summary>
public static class CatalogParser
{
    public static readonly Uri DefaultPageUri = new("https://omarchy.org/themes/");

    public static IReadOnlyList<CatalogEntry> Parse(string html, Uri? pageUri = null)
    {
        pageUri ??= DefaultPageUri;
        using var document = new HtmlParser().ParseDocument(html);

        var entries = new List<CatalogEntry>();
        var seenRepos = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var seenSlugs = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var anchor in document.QuerySelectorAll("a[href]"))
        {
            if (!RepoRef.TryParse(anchor.GetAttribute("href"), out var repo))
                continue;

            var card = anchor.Closest("li, figure, article");
            var img = anchor.QuerySelector("img") ?? card?.QuerySelector("img");
            if (img is null)
                continue; // text links such as "Share your theme" are not theme cards

            var repoUrl = repo.HtmlUrl.AbsoluteUri;
            if (!seenRepos.Add(repoUrl))
                continue;

            var screenshot = ResolveUrl(img.GetAttribute("src"), pageUri);
            var name = FirstNonEmpty(
                    card?.QuerySelector("figcaption")?.TextContent,
                    anchor.TextContent,
                    StripScreenshotSuffix(img.GetAttribute("alt")))
                ?? repo.Name;

            var slug = Unique(SlugFor(screenshot, repo), seenSlugs);
            entries.Add(new CatalogEntry(slug, name, repoUrl, screenshot?.AbsoluteUri));
        }

        return entries;
    }

    private static Uri? ResolveUrl(string? src, Uri pageUri) =>
        !string.IsNullOrWhiteSpace(src) && Uri.TryCreate(pageUri, src.Trim(), out var u) && u.Scheme is "https" or "http"
            ? u
            : null;

    private static string? FirstNonEmpty(params string?[] candidates) =>
        candidates.Select(c => c is null ? null : CollapseWhitespace(c))
            .FirstOrDefault(c => !string.IsNullOrEmpty(c));

    private static string CollapseWhitespace(string s) =>
        string.Join(' ', s.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries));

    private static string? StripScreenshotSuffix(string? alt)
    {
        if (alt is null)
            return null;
        const string suffix = " theme screenshot";
        return alt.EndsWith(suffix, StringComparison.OrdinalIgnoreCase) ? alt[..^suffix.Length] : alt;
    }

    internal static string SlugFor(Uri? screenshot, RepoRef repo)
    {
        var fromScreenshot = screenshot is null ? null : Sanitize(Path.GetFileNameWithoutExtension(screenshot.AbsolutePath));
        if (!string.IsNullOrEmpty(fromScreenshot))
            return fromScreenshot;

        var name = repo.SubPath is null ? repo.Name : Path.GetFileName(repo.SubPath);
        var slug = Sanitize(name);
        if (slug.StartsWith("omarchy-") && slug.Length > 8) slug = slug[8..];
        if (slug.EndsWith("-theme") && slug.Length > 6) slug = slug[..^6];
        return slug.Length > 0 ? slug : "theme";
    }

    private static string Sanitize(string s)
    {
        var sb = new StringBuilder(s.Length);
        foreach (var c in s.ToLowerInvariant())
            sb.Append(char.IsAsciiLetterOrDigit(c) || c is '-' or '_' ? c : '-');
        return sb.ToString().Trim('-');
    }

    private static string Unique(string slug, HashSet<string> seen)
    {
        var candidate = slug;
        for (var i = 2; !seen.Add(candidate); i++)
            candidate = $"{slug}-{i}";
        return candidate;
    }
}
