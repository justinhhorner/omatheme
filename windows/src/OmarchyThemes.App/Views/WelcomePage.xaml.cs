using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.App.Views;

public sealed partial class WelcomePage : Page
{
    public WelcomePage()
    {
        InitializeComponent();
    }

    private void BrowseButton_Click(object sender, RoutedEventArgs e)
    {
        App.GetService<SettingsStore>().Update(s => s with { WelcomeSeen = true });
        App.Current.MainWindow?.ShowShell();
    }
}
