using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.ViewModels;

namespace OmarchyThemes.App.Views;

public sealed partial class GalleryPage : Page
{
    public GalleryPage()
    {
        InitializeComponent();
    }

    public GalleryViewModel ViewModel { get; } = App.GetService<GalleryViewModel>();

    protected override async void OnNavigatedTo(NavigationEventArgs e)
    {
        base.OnNavigatedTo(e);
        await ViewModel.EnsureLoadedAsync();
    }

    private async void CurrentWallpaper_Click(object sender, RoutedEventArgs e)
    {
        if (Ui.ItemOf<CurrentWallpaperViewModel>(sender) is { } wallpaper && ViewModel.CurrentTheme is { } current)
            await current.SetWallpaperAsync(wallpaper);
    }

    private void ViewCurrentTheme_Click(object sender, RoutedEventArgs e) => ViewModel.OpenCurrentTheme();

    private void Card_Click(object sender, RoutedEventArgs e)
    {
        if (Ui.ItemOf<ThemeCardViewModel>(sender) is { } card)
            ViewModel.Open(card);
    }

    private void SearchAccelerator_Invoked(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        SearchBox.Focus(FocusState.Keyboard);
        args.Handled = true;
    }
}
