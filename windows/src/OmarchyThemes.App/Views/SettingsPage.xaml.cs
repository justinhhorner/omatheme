using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.ViewModels;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.Views;

public sealed partial class SettingsPage : Page
{
    public SettingsPage()
    {
        InitializeComponent();
        foreach (var fit in SettingsViewModel.FitOptions)
            FitCombo.Items.Add(fit.ToString());
        FitCombo.SelectedItem = ViewModel.Fit.ToString();
    }

    public SettingsViewModel ViewModel { get; } = App.GetService<SettingsViewModel>();

    private void FitCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (Enum.TryParse<WallpaperFit>(FitCombo.SelectedItem as string, out var fit))
            ViewModel.Fit = fit;
    }

    private void ShowWelcome_Click(object sender, RoutedEventArgs e) =>
        App.Current.MainWindow?.ShowWelcome();
}
