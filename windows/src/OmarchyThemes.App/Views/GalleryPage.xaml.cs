using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Navigation;
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

    private void ThemesView_ItemInvoked(ItemsView sender, ItemsViewItemInvokedEventArgs args)
    {
        if (args.InvokedItem is ThemeCardViewModel card)
            ViewModel.Open(card);
    }

    private void SearchAccelerator_Invoked(KeyboardAccelerator sender, KeyboardAcceleratorInvokedEventArgs args)
    {
        SearchBox.Focus(FocusState.Keyboard);
        args.Handled = true;
    }
}
