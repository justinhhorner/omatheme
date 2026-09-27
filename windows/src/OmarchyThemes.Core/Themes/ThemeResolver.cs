using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Palettes;

namespace OmarchyThemes.Core.Themes;

/// <summary>A wallpaper image in a theme repo.</summary>
/// <param name="Path">Path relative to the theme root, e.g. "backgrounds/1.png".</param>
public sealed record WallpaperRef(string Path, long? Size, Uri DownloadUrl)
{
    public string FileName => System.IO.Path.GetFileName(Path);
}

/// <summary>Everything the app needs from a theme's repo to preview, download and apply it.</summary>
/// <param name="Palette">Null when no supported palette file could be read; see <paramref name="PaletteError"/>.</param>
/// <param name="IsStale">Served from cache because GitHub was unreachable or rate-limited.</param>
public sealed record ThemeDetails(
    CatalogEntry Entry,
    RepoRef Repo,
    Palette? Palette,
    string? PaletteError,
    IReadOnlyList<WallpaperRef> Wallpapers,
    AppearanceMode Mode,
    bool IsStale = false,
    Exception? StaleReason = null)
{
    public bool CanApply => Wallpapers.Count > 0 || Palette is not null;
}

public sealed class ThemeResolveException(string message, Exception? inner = null) : Exception(message, inner);

/// <summary>
/// Resolves a catalog entry to its palette and wallpapers. Called only when the user opens a
/// theme, and costs one GitHub API call (usually a free 304 on revisits).
/// </summary>
public sealed class ThemeResolver
{
    /// <summary>Palette files in priority order.</summary>
    private static readonly (string File, Func<string, Palette> Parse)[] PaletteFiles =
    [
        ("colors.toml", ColorsTomlParser.Parse),
        ("alacritty.toml", AlacrittyParser.Parse),
    ];

    private static readonly string[] WallpaperDirs = ["backgrounds/", "wallpapers/"];
    private static readonly HashSet<string> ImageExtensions = new(StringComparer.OrdinalIgnoreCase)
        { ".png", ".jpg", ".jpeg", ".webp", ".bmp" };

    private readonly GitHubClient _github;

    public ThemeResolver(GitHubClient github) => _github = github;

    public async Task<ThemeDetails> ResolveAsync(CatalogEntry entry, CancellationToken ct = default)
    {
        if (!RepoRef.TryParse(entry.RepoUrl, out var repo))
            throw new ThemeResolveException($"{entry.Name} doesn't link to a GitHub repository, so it can't be read.");

        var tree = await _github.GetTreeAsync(repo, ct).ConfigureAwait(false);

        var (palette, paletteError) = await ReadPaletteAsync(repo, tree, ct).ConfigureAwait(false);
        var wallpapers = FindWallpapers(tree)
            .Select(f => new WallpaperRef(f.Path, f.Size, GitHubClient.RawUrl(repo, f.Path)))
            .ToList();

        var hasLightModeFile = tree.FindFile("light.mode") is not null;
        if (palette is { DeclaredMode: null } && hasLightModeFile)
            palette = palette with { DeclaredMode = AppearanceMode.Light };
        var mode = palette?.Mode ?? (hasLightModeFile ? AppearanceMode.Light : AppearanceMode.Dark);

        return new ThemeDetails(entry, repo, palette, paletteError, wallpapers, mode, tree.IsStale, tree.StaleReason);
    }

    private async Task<(Palette?, string?)> ReadPaletteAsync(RepoRef repo, RepoTree tree, CancellationToken ct)
    {
        var problems = new List<string>();
        foreach (var (file, parse) in PaletteFiles)
        {
            if (tree.FindFile(file) is not { } item)
                continue;
            try
            {
                var text = await _github.GetRawTextAsync(repo, item.Path, ct).ConfigureAwait(false);
                return (parse(text), null);
            }
            catch (PaletteParseException e)
            {
                problems.Add(e.Message);
            }
            catch (Exception e) when (e is GitHubException or HttpRequestException)
            {
                problems.Add($"{file} couldn't be downloaded: {e.Message}");
            }
        }

        return problems.Count > 0
            ? (null, "Couldn't read this theme's palette. " + string.Join(" ", problems))
            : (null, "Couldn't read this theme's palette: it has no colors.toml or alacritty.toml.");
    }

    internal static IEnumerable<RepoTreeItem> FindWallpapers(RepoTree tree)
    {
        var images = tree.Files.Where(f => ImageExtensions.Contains(Path.GetExtension(f.Path))).ToList();

        foreach (var dir in WallpaperDirs)
        {
            var inDir = images.Where(f => f.Path.StartsWith(dir, StringComparison.OrdinalIgnoreCase)).ToList();
            if (inDir.Count > 0)
                return inDir.OrderBy(f => f.Path, NaturalStringComparer.Instance);
        }

        // Some repos keep a single background at the root.
        return images
            .Where(f => !f.Path.Contains('/')
                && (f.FileName.StartsWith("background", StringComparison.OrdinalIgnoreCase)
                    || f.FileName.StartsWith("wallpaper", StringComparison.OrdinalIgnoreCase)))
            .OrderBy(f => f.Path, NaturalStringComparer.Instance);
    }
}

/// <summary>Orders "2.png" before "10.png".</summary>
internal sealed class NaturalStringComparer : IComparer<string>
{
    public static readonly NaturalStringComparer Instance = new();

    public int Compare(string? x, string? y)
    {
        if (x is null || y is null)
            return string.CompareOrdinal(x, y);

        int i = 0, j = 0;
        while (i < x.Length && j < y.Length)
        {
            if (char.IsAsciiDigit(x[i]) && char.IsAsciiDigit(y[j]))
            {
                var si = i; while (i < x.Length && char.IsAsciiDigit(x[i])) i++;
                var sj = j; while (j < y.Length && char.IsAsciiDigit(y[j])) j++;
                var a = x.AsSpan(si, i - si).TrimStart('0');
                var b = y.AsSpan(sj, j - sj).TrimStart('0');
                var cmp = a.Length != b.Length ? a.Length.CompareTo(b.Length) : a.CompareTo(b, StringComparison.Ordinal);
                if (cmp != 0)
                    return cmp;
            }
            else
            {
                var cmp = char.ToLowerInvariant(x[i]).CompareTo(char.ToLowerInvariant(y[j]));
                if (cmp != 0)
                    return cmp;
                i++;
                j++;
            }
        }
        return (x.Length - i).CompareTo(y.Length - j);
    }
}
