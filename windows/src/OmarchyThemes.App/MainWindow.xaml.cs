using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using OmarchyThemes.App.Services;
using OmarchyThemes.App.Views;
using Windows.Graphics;

namespace OmarchyThemes.App;

public sealed partial class MainWindow : Window
{
    public MainWindow(bool showWelcome)
    {
        InitializeComponent();

        ExtendsContentIntoTitleBar = true;
        SetTitleBar(AppTitleBar);
        AppWindow.TitleBar.PreferredHeightOption = TitleBarHeightOption.Tall;
        AppWindow.SetIcon(Path.Combine(AppContext.BaseDirectory, "Assets", "AppIcon.ico"));
        AppWindow.Resize(new SizeInt32(1280, 840));
        if (AppWindow.Presenter is OverlappedPresenter presenter)
        {
            presenter.PreferredMinimumWidth = 760;
            presenter.PreferredMinimumHeight = 540;
        }

        // Test switches (OMARCHY_THEMES_DRY_RUN / _DATA_DIR) are always visible, so a test session
        // can't be mistaken for one that changes the real desktop.
        if (App.GetService<AppEnvironment>().Badge is { } badge)
            AppTitleBar.Subtitle = badge;

        RootFrame.Navigate(showWelcome ? typeof(WelcomePage) : typeof(ShellPage));
    }

    public TitleBar TitleBar => AppTitleBar;

    /// <summary>Leaves the welcome screen and enters the main navigation shell.</summary>
    public void ShowShell() =>
        RootFrame.Navigate(typeof(ShellPage), null, new DrillInNavigationTransitionInfo());

    public void ShowWelcome() =>
        RootFrame.Navigate(typeof(WelcomePage), null, new DrillInNavigationTransitionInfo());
}
