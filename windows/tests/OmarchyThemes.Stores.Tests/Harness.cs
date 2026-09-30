using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores.Tests;

/// <summary>Every store, wired together as the app does.</summary>
internal sealed record AppStores(
    Preferences Preferences, CatalogStore Catalog, ThemeLibrary Library, DesktopStore Desktop, TerminalStore Terminal);

/// <summary>
/// The stores wired to fakes: routed HTTP (optionally held until released), a downloader that writes
/// the URL into each file, a desktop backend that records calls, an in-memory terminal and a temp
/// data folder.
/// </summary>
internal sealed class Harness : IDisposable
{
    public const string TreeUrl = "https://api.github.com/repos/omacom/omarchy/git/trees/HEAD?recursive=1";
    public const string Raw = "https://raw.githubusercontent.com/omacom/omarchy/HEAD/themes/";
    public static readonly string PageUrl = CatalogParser.DefaultPageUri.AbsoluteUri;

    public static readonly CatalogEntry Tokyo = new(
        "omarchy.tokyo-night", "Tokyo Night", "https://github.com/omacom/omarchy/tree/HEAD/themes/tokyo-night",
        Raw + "tokyo-night/preview.png");

    public TempDir Dir { get; } = new();
    public FakeHttpHandler Http { get; } = new();
    public FakeDownloader Downloader { get; } = new();
    public FakeDesktopBackend Backend { get; } = new();
    public InMemorySnapshotStore Snapshots { get; } = new();
    public ManualTimeProvider Time { get; } = new(new DateTimeOffset(2026, 9, 27, 0, 0, 0, TimeSpan.Zero));
    public FakeTerminalSchemes Terminal { get; } = new();

    /// <summary>When set, HTTP responses wait for it (to overlap two refreshes).</summary>
    public TaskCompletionSource? HttpGate { get; set; }

    /// <summary>Makes the theme store's folder moves fail (a file in use), for removal failures.</summary>
    public bool FailMoves { get; set; }

    public void Dispose() => Dir.Dispose();

    /// <summary>The catalog page and Omarchy's repo tree, with Tokyo Night's palette.</summary>
    public void ServeCatalog() =>
        Http.On(PageUrl, Fixture.Read("catalog-live-structure.html"))
            .On(TreeUrl, Fixture.Read("omarchy-tree.json"))
            .On(Raw + "tokyo-night/colors.toml", Fixture.Read("colors-named.toml"));

    /// <summary>A fresh set of stores over the same data folder (like relaunching the app).</summary>
    public AppStores MakeStores()
    {
        var paths = Dir.AppPaths;
        var cache = new HttpCache(new HttpClient(new Gated(this)), paths.CacheDir, Time);
        var github = new GitHubClient(cache);
        var details = new ThemeDetailsService(new ThemeResolver(github));
        var themes = new ThemeStore(paths, Downloader, Time)
        {
            MoveDirectory = (from, to) =>
            {
                if (FailMoves)
                    throw new IOException("The folder is in use.");
                Directory.Move(from, to);
            },
        };

        var preferences = new Preferences(new SettingsStore(paths));
        var library = new ThemeLibrary(themes, details, new ThemeDownloads(details, themes), paths);
        return new AppStores(
            preferences,
            new CatalogStore(new CatalogService(cache, github: github), Time),
            library,
            new DesktopStore(new ThemeApplier(Backend, Snapshots), preferences, library),
            new TerminalStore(Terminal, library));
    }

    /// <summary>Downloads Tokyo Night (three wallpapers in the fixture tree) through the library.</summary>
    public async Task<InstalledTheme> InstallTokyoAsync(AppStores stores)
    {
        ServeCatalog();
        var outcome = await stores.Library.Download(Tokyo).Completion;
        return outcome.Theme ?? throw new InvalidOperationException($"Download failed: {outcome.Error}");
    }

    private sealed class Gated(Harness harness) : DelegatingHandler(harness.Http)
    {
        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            if (harness.HttpGate is { } gate)
                await gate.Task.WaitAsync(ct);
            return await base.SendAsync(request, ct);
        }
    }
}

/// <summary>A terminal that keeps schemes in memory; can be told to fail.</summary>
internal sealed class FakeTerminalSchemes : ITerminalSchemes
{
    public Dictionary<string, TerminalColors> Schemes { get; } = new();

    public bool Fail { get; set; }

    public string DisplayName => "Test Terminal";

    public bool IsAvailable { get; set; } = true;

    public string SchemeName(string themeName) => $"{themeName} (Omarchy)";

    public bool IsAdded(string slug) => Schemes.ContainsKey(slug);

    public void Add(string slug, string themeName, TerminalColors colors)
    {
        ThrowIfFailing();
        Schemes[slug] = colors;
    }

    public void Remove(string slug)
    {
        ThrowIfFailing();
        Schemes.Remove(slug);
    }

    public string AddedMessage(string schemeName) => $"Pick {schemeName}.";

    public string RemovedMessage(string schemeName) => $"{schemeName} is gone.";

    private void ThrowIfFailing()
    {
        if (Fail)
            throw new UnauthorizedAccessException("The terminal's folder is read-only.");
    }
}
