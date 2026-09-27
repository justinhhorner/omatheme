using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.ViewModels;
using OmarchyThemes.Core.Catalog;

namespace OmarchyThemes.App.Views;

public sealed partial class ThemeDetailPage : Page
{
    public ThemeDetailPage()
    {
        InitializeComponent();
    }

    public ThemeDetailViewModel ViewModel { get; } = App.GetService<ThemeDetailViewModel>();

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        if (e.Parameter is CatalogEntry entry)
            await ViewModel.LoadAsync(entry);
    }

    private async void Apply_Click(object sender, RoutedEventArgs e)
    {
        if (!ViewModel.IsInstalled && !await ViewModel.DownloadAsync())
            return;
        if (ViewModel.Installed is not { } theme)
            return;

        var options = await ApplyDialog.ShowAsync(XamlRoot, theme);
        if (options is not null)
            await ViewModel.ApplyAsync(options);
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        var confirm = new ContentDialog
        {
            XamlRoot = XamlRoot,
            Style = (Style)Application.Current.Resources["DefaultContentDialogStyle"],
            Title = $"Remove {ViewModel.Name}?",
            Content = "The downloaded wallpapers and colors will be deleted from this PC. Your current desktop won't change.",
            PrimaryButtonText = "Remove",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        if (await confirm.ShowAsync() == ContentDialogResult.Primary)
            ViewModel.Remove();
    }

    private async void OpenRepo_Click(object sender, RoutedEventArgs e) =>
        await Windows.System.Launcher.LaunchUriAsync(ViewModel.RepoUri);

    private void WallpaperGrid_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (WallpaperGrid.SelectedItem is WallpaperItemViewModel item)
            ViewModel.SelectedWallpaper = item;
    }
}
