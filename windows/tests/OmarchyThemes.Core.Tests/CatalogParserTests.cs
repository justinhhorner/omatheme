using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Tests.TestSupport;

namespace OmarchyThemes.Core.Tests;

public class CatalogParserTests
{
    [Fact]
    public void Parses_live_omarchy_org_card_structure()
    {
        var entries = CatalogParser.Parse(Fixture.Read("catalog-live-structure.html"));

        Assert.Equal(["aetheria", "amberbyte", "arc-blueberry", "all-hallows-eve"], entries.Select(e => e.Slug));

        var aetheria = entries[0];
        Assert.Equal("Aetheria", aetheria.Name);
        Assert.Equal("https://github.com/JJDizz1L/aetheria", aetheria.RepoUrl);
        Assert.Equal("https://omarchy.org/assets/themes/aetheria.webp", aetheria.ScreenshotUrl);
    }

    [Fact]
    public void Normalizes_repo_urls_names_and_relative_screenshots()
    {
        var entries = CatalogParser.Parse(Fixture.Read("catalog-live-structure.html")).ToDictionary(e => e.Slug);

        Assert.Equal("https://github.com/tahfizhabib/omarchy-amberbyte-theme", entries["amberbyte"].RepoUrl);
        Assert.Equal("Arc Blueberry", entries["arc-blueberry"].Name);
        Assert.Equal("https://omarchy.org/assets/themes/arc-blueberry.webp", entries["arc-blueberry"].ScreenshotUrl);
        Assert.Equal("All Hallow's Eve", entries["all-hallows-eve"].Name);
        Assert.Equal("https://github.com/someone/monorepo/tree/main/themes/all-hallows-eve", entries["all-hallows-eve"].RepoUrl);
        Assert.Equal("https://cdn.example.com/shots/all-hallows-eve.png", entries["all-hallows-eve"].ScreenshotUrl);
    }

    [Fact]
    public void Skips_nav_links_non_github_links_cards_without_screenshots_and_duplicates()
    {
        var entries = CatalogParser.Parse(Fixture.Read("catalog-live-structure.html"));

        Assert.DoesNotContain(entries, e => e.RepoUrl.Contains("basecamp/omarchy"));
        Assert.DoesNotContain(entries, e => e.RepoUrl.Contains("omarchy-site"));
        Assert.DoesNotContain(entries, e => e.Name == "Not GitHub");
        Assert.DoesNotContain(entries, e => e.Name == "No Screenshot");
        Assert.Single(entries, e => e.RepoUrl.EndsWith("/aetheria"));
    }

    [Fact]
    public void Supports_figure_layout()
    {
        var entries = CatalogParser.Parse(Fixture.Read("catalog-figure.html"));

        Assert.Collection(entries,
            e =>
            {
                Assert.Equal("tokyo-night", e.Slug);
                Assert.Equal("Tokyo Night", e.Name);
                Assert.Equal("https://github.com/someone/omarchy-tokyo-night-theme", e.RepoUrl);
            },
            e =>
            {
                Assert.Equal("rose-pine", e.Slug);
                Assert.Equal("Rosé Pine", e.Name);
            },
            e =>
            {
                // No screenshot URL: slug falls back to the repo name minus omarchy-/-theme.
                Assert.Equal("no-src", e.Slug);
                Assert.Null(e.ScreenshotUrl);
            });
    }

    [Fact]
    public void Makes_slugs_unique()
    {
        const string html = """
            <ul>
              <li><a href="https://github.com/a/one"><img src="/assets/themes/same.webp"><span>One</span></a></li>
              <li><a href="https://github.com/b/two"><img src="/assets/themes/same.webp"><span>Two</span></a></li>
            </ul>
            """;

        Assert.Equal(["same", "same-2"], CatalogParser.Parse(html).Select(e => e.Slug));
    }

    [Theory]
    [InlineData("")]
    [InlineData("<html><body><p>Maintenance</p></body></html>")]
    [InlineData("<ul><li><a href=\"https://github.com/x/y\"")]
    public void Returns_empty_for_pages_without_theme_cards(string html)
    {
        Assert.Empty(CatalogParser.Parse(html));
    }
}
