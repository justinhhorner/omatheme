using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace OmarchyThemes.App.Views;

public sealed partial class SettingsPage : Page
{
    public SettingsPage()
    {
        InitializeComponent();
    }

    private void ShowWelcome_Click(object sender, RoutedEventArgs e) =>
        App.Current.MainWindow?.ShowWelcome();
}
