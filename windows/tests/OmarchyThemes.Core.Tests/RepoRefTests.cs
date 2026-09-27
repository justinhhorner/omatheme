using OmarchyThemes.Core.GitHub;

namespace OmarchyThemes.Core.Tests;

public class RepoRefTests
{
    [Theory]
    [InlineData("https://github.com/JJDizz1L/aetheria", "JJDizz1L", "aetheria", null, null)]
    [InlineData("https://github.com/owner/repo.git", "owner", "repo", null, null)]
    [InlineData("https://www.github.com/owner/repo/", "owner", "repo", null, null)]
    [InlineData("http://github.com/owner/repo", "owner", "repo", null, null)]
    [InlineData("  https://github.com/owner/repo  ", "owner", "repo", null, null)]
    [InlineData("https://github.com/owner/repo/tree/dev", "owner", "repo", "dev", null)]
    [InlineData("https://github.com/owner/mono/tree/main/themes/foo", "owner", "mono", "main", "themes/foo")]
    public void Parses_repo_links(string url, string owner, string name, string? gitRef, string? subPath)
    {
        Assert.True(RepoRef.TryParse(url, out var repo));
        Assert.Equal(new RepoRef(owner, name, gitRef, subPath), repo);
    }

    [Theory]
    [InlineData(null)]
    [InlineData("")]
    [InlineData("not a url")]
    [InlineData("https://gitlab.com/owner/repo")]
    [InlineData("https://github.com/owner")]
    [InlineData("https://github.com/omacom/omarchy-site/compare")]
    [InlineData("https://github.com/owner/repo/issues/1")]
    [InlineData("https://github.com/owner/repo/tree")]
    [InlineData("https://github.com/orgs/basecamp")]
    [InlineData("https://github.com/owner/repo/tree/main/../../etc")]
    [InlineData("ftp://github.com/owner/repo")]
    public void Rejects_non_repo_links(string? url)
    {
        Assert.False(RepoRef.TryParse(url, out _));
    }

    [Fact]
    public void Default_ref_is_HEAD_and_paths_are_prefixed_with_sub_path()
    {
        var plain = new RepoRef("o", "r");
        var nested = new RepoRef("o", "r", "main", "themes/foo");

        Assert.Equal("HEAD", plain.EffectiveRef);
        Assert.Equal("colors.toml", plain.RepoPath("colors.toml"));
        Assert.Equal("themes/foo/colors.toml", nested.RepoPath("colors.toml"));
        Assert.Equal("https://github.com/o/r", plain.HtmlUrl.AbsoluteUri);
        Assert.Equal("https://github.com/o/r/tree/main/themes/foo", nested.HtmlUrl.AbsoluteUri);
    }

    [Fact]
    public void Raw_urls_escape_path_segments()
    {
        var url = GitHubClient.RawUrl(new RepoRef("o", "r", SubPath: "my themes"), "backgrounds/1 dark.png");
        Assert.Equal("https://raw.githubusercontent.com/o/r/HEAD/my%20themes/backgrounds/1%20dark.png", url.AbsoluteUri);
    }
}
