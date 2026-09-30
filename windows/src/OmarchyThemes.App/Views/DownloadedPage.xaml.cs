using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.ViewModels;
using OmarchyThemes.Stores;

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
        if (await ApplyDialog.ShowAsync(XamlRoot, item.Theme, App.GetService<DesktopStore>()) is { } choice)
            await ViewModel.ApplyAsync(item, choice.Options, choice.Remember);
    }

    private void Open_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item)
            ViewModel.Open(item);
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (ItemOf(sender) is { } item && await Dialogs.ConfirmRemoveAsync(XamlRoot, item.Name))
            ViewModel.Remove(item);
    }
}
