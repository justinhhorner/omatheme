using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.ViewModels;

namespace OmarchyThemes.App.Views;

public sealed partial class DownloadedPage : Page
{
    public DownloadedPage()
    {
        InitializeComponent();
    }

    public DownloadedViewModel ViewModel { get; } = App.GetService<DownloadedViewModel>();

    protected override void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        ViewModel.Reload();
    }

    private static InstalledThemeViewModel? ItemOf(object sender) =>
        Ui.ItemOf<InstalledThemeViewModel>(sender);

    private void SetAsDesktop_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            ViewModel.SetAsDesktopCommand.Execute(item);
    }

    private async void ApplyWithOptions_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { } item)
            return;
        var options = await ApplyDialog.ShowAsync(XamlRoot, item.Theme);
        if (options is not null)
            await ViewModel.ApplyAsync(item, options);
    }

    private void Open_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            ViewModel.Open(item);
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is not { } item)
            return;
        var confirm = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Style = (Style)Application.Current.Resources["DefaultContentDialogStyle"],
            Title = $"Remove {item.Name}?",
            Content = "The downloaded wallpapers and colors will be deleted from this PC. Your current desktop won't change.",
            PrimaryButtonText = "Remove",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await confirm.ShowAsync() == ContentDialogResult.Primary)
            ViewModel.Remove(item);
    }
}
