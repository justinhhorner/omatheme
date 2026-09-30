using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.Views;

public sealed partial class WelcomePage : Page
{
    public WelcomePage()
    {
        InitializeComponent();
    }

    private void BrowseButton_Click(object sender, RoutedEventArgs e)
    {
        App.GetService<Preferences>().DismissWelcome();
        App.Current.MainWindow?.ShowShell();
    }
}
