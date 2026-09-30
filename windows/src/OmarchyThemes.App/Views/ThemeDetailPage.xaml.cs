using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.ViewModels;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.Views;

public sealed partial class ThemeDetailPage : Page
{
    public ThemeDetailPage()
    {
        InitializeComponent();
        ViewModel.PropertyChanged += OnViewModelPropertyChanged;
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

        if (await ApplyDialog.ShowAsync(XamlRoot, theme, App.GetService<DesktopStore>()) is { } choice)
            await ViewModel.ApplyAsync(choice.Options, choice.Remember);
    }

    private async void Remove_Click(object sender, RoutedEventArgs e)
    {
        if (await Dialogs.ConfirmRemoveAsync(XamlRoot, ViewModel.Name))
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
