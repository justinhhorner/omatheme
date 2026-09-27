using Microsoft.UI.Windowing;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using OmarchyThemes.App.Views;
using Windows.Graphics;

namespace OmarchyThemes.App;

public sealed partial class MainWindow : Window
{
    public MainWindow()
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

        RootFrame.Navigate(typeof(WelcomePage));
    }

    public TitleBar TitleBar => AppTitleBar;

    /// <summary>Leaves the welcome screen and enters the main navigation shell.</summary>
    public void ShowShell() =>
        RootFrame.Navigate(typeof(ShellPage), null, new DrillInNavigationTransitionInfo());

    public void ShowWelcome() =>
        RootFrame.Navigate(typeof(WelcomePage), null, new DrillInNavigationTransitionInfo());
}
