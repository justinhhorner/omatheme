using OmarchyThemes.Core.GitHub;

namespace OmarchyThemes.Core.Catalog;

/// <summary>
/// The themes that ship with Omarchy, in the themes/ folder of its repo. They aren't listed on
/// omarchy.org/themes, so they're discovered from the repo's file tree: the same cached API call
/// that resolving any one of them uses, so opening a default theme costs no extra request.
/// </summary>
public static class DefaultThemes
{
    /// <summary>Omarchy's repo. Its default branch (HEAD) is the current release line.</summary>
    public static readonly RepoRef Repo = new("omacom", "omarchy");

    /// <summary>
    /// Prefixes default-theme slugs. Community slugs never contain a '.', so the two can't
    /// collide, and a downloaded default theme keeps its folder name.
    /// </summary>
    public const string SlugPrefix = "omarchy.";

    private const string Folder = "themes/";

    public static bool IsDefault(string slug) => slug.StartsWith(SlugPrefix, StringComparison.Ordinal);

    /// <summary>One entry per folder directly under themes/, sorted by name.</summary>
    public static IReadOnlyList<CatalogEntry> FromTree(RepoTree tree)
    {
        var files = tree.Files.Select(f => f.Path).ToHashSet(StringComparer.OrdinalIgnoreCase);

        return tree.Items
            .Where(i => !i.IsFile && i.Path.StartsWith(Folder, StringComparison.Ordinal) && i.Path.IndexOf('/', Folder.Length) < 0)
            .Select(i => i.Path[Folder.Length..])
            .Where(folder => AppPaths.IsValidSlug(SlugPrefix + folder))
            .OrderBy(folder => folder, StringComparer.Ordinal)
            .Select(folder =>
            {
                var theme = new RepoRef(Repo.Owner, Repo.Name, "HEAD", Folder + folder);
                var preview = files.Contains($"{Folder}{folder}/preview.png")
                    ? GitHubClient.RawUrl(theme, "preview.png").AbsoluteUri
                    : null;
                return new CatalogEntry(SlugPrefix + folder, DisplayName(folder), theme.HtmlUrl.AbsoluteUri, preview);
            })
            .ToList();
    }

    /// <summary>Omarchy's own naming (omarchy-theme-list): "retro-82" → "Retro 82".</summary>
    public static string DisplayName(string folder) =>
        string.Join(' ', folder.Split('-', StringSplitOptions.RemoveEmptyEntries)
            .Select(word => char.ToUpperInvariant(word[0]) + word[1..]));
}
