using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media.Animation;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.Services;

namespace OmarchyThemes.App.Views;

public sealed partial class ShellPage : Page
{
    private static readonly Dictionary<string, Type> Pages = new()
    {
        ["Gallery"] = typeof(GalleryPage),
        ["Downloaded"] = typeof(DownloadedPage),
        ["About"] = typeof(AboutPage),
        ["Settings"] = typeof(SettingsPage),
    };

    private readonly NavigationService _navigation = App.GetService<NavigationService>();

    public ShellPage()
    {
        InitializeComponent();
        _navigation.Attach(ContentFrame);
        Loaded += OnLoaded;
        Unloaded += OnUnloaded;
    }

    private TitleBar? TitleBar => App.Current.MainWindow?.TitleBar;

    private void OnSectionRequested(object? sender, string tag) =>
        NavView.SelectedItem = NavView.MenuItems.Concat(NavView.FooterMenuItems)
            .OfType<NavigationViewItem>()
            .FirstOrDefault(i => (string)i.Tag == tag);

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        _navigation.SectionRequested += OnSectionRequested;
        if (TitleBar is { } titleBar)
        {
            titleBar.IsPaneToggleButtonVisible = true;
            titleBar.BackRequested += TitleBar_BackRequested;
            titleBar.PaneToggleRequested += TitleBar_PaneToggleRequested;
        }

        if (NavView.SelectedItem is null)
            NavView.SelectedItem = NavView.MenuItems[0];
    }

    private void OnUnloaded(object sender, RoutedEventArgs e)
    {
        _navigation.SectionRequested -= OnSectionRequested;
        if (TitleBar is { } titleBar)
        {
            titleBar.IsPaneToggleButtonVisible = false;
            titleBar.IsBackButtonVisible = false;
            titleBar.BackRequested -= TitleBar_BackRequested;
            titleBar.PaneToggleRequested -= TitleBar_PaneToggleRequested;
        }
    }

    private void NavView_SelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        var tag = args.IsSettingsSelected ? "Settings" : (args.SelectedItem as NavigationViewItem)?.Tag as string;
        if (tag is not null && Pages.TryGetValue(tag, out var pageType) && ContentFrame.CurrentSourcePageType != pageType)
            ContentFrame.Navigate(pageType, null, new EntranceNavigationTransitionInfo());
    }

    private void ContentFrame_Navigated(object sender, NavigationEventArgs e)
    {
        if (TitleBar is { } titleBar)
            titleBar.IsBackButtonVisible = ContentFrame.CanGoBack;

        // Keep the nav selection in sync after a back navigation.
        if (e.SourcePageType == typeof(SettingsPage))
        {
            NavView.SelectedItem = NavView.SettingsItem;
            return;
        }

        var tag = Pages.FirstOrDefault(p => p.Value == e.SourcePageType).Key;
        if (tag is null)
            return;
        NavView.SelectedItem = NavView.MenuItems.Concat(NavView.FooterMenuItems)
            .OfType<NavigationViewItem>()
            .FirstOrDefault(i => (string)i.Tag == tag);
    }

    private void TitleBar_BackRequested(TitleBar sender, object args)
    {
        if (ContentFrame.CanGoBack)
            ContentFrame.GoBack();
    }

    private void TitleBar_PaneToggleRequested(TitleBar sender, object args) =>
        NavView.IsPaneOpen = !NavView.IsPaneOpen;
}
