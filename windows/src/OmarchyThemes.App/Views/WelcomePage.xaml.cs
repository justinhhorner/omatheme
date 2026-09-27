using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace OmarchyThemes.App.Views;

public sealed partial class WelcomePage : Page
{
    public WelcomePage()
    {
        InitializeComponent();
    }

    private void BrowseButton_Click(object sender, RoutedEventArgs e) =>
        App.Current.MainWindow?.ShowShell();
}
