using OmarchyThemes.Core;

namespace OmarchyThemes.Core.Tests;

public class AppPathsTests
{
    private readonly AppPaths _paths = new(Path.Combine(Path.GetTempPath(), "omatheme-tests"));

    [Theory]
    [InlineData("tokyo-night")]
    [InlineData("omarchy_arc.blueberry")]
    public void ThemeDir_accepts_simple_slugs(string slug)
    {
        Assert.Equal(Path.Combine(_paths.ThemesDir, slug), _paths.ThemeDir(slug));
    }

    [Theory]
    [InlineData("")]
    [InlineData("..")]
    [InlineData("../evil")]
    [InlineData("a/b")]
    [InlineData("a\\b")]
    [InlineData("C:")]
    public void ThemeDir_rejects_path_escapes(string slug)
    {
        Assert.Throws<ArgumentException>(() => _paths.ThemeDir(slug));
    }
}
