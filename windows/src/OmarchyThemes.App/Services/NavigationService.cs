using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using OmarchyThemes.App.Views;
using OmarchyThemes.Core.Catalog;

namespace OmarchyThemes.App.Services;

/// <summary>Navigation inside the shell's content frame, usable from view models.</summary>
public sealed class NavigationService
{
    private Frame? _frame;

    /// <summary>Raised when a top-level section (nav item tag, see <see cref="Sections"/>) should be selected.</summary>
    public event EventHandler<string>? SectionRequested;

    public void Attach(Frame frame) => _frame = frame;

    public void OpenTheme(CatalogEntry entry) =>
        _frame?.Navigate(typeof(ThemeDetailPage), entry, new DrillInNavigationTransitionInfo());

    public void GoBack()
    {
        if (_frame?.CanGoBack == true)
            _frame.GoBack();
    }

    public void ShowSection(string tag) => SectionRequested?.Invoke(this, tag);
}

/// <summary>The shell's top-level sections: the nav items' tags (literal in ShellPage.xaml too).</summary>
public static class Sections
{
    public const string Gallery = "Gallery";
    public const string Downloaded = "Downloaded";
    public const string About = "About";
    public const string Settings = "Settings";
}
