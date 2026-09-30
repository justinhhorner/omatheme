using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.ViewModels;

namespace OmarchyThemes.App.Views;

public sealed partial class SettingsPage : Page
{
    public SettingsPage()
    {
        InitializeComponent();
        FitChoices.Fill(FitCombo, ViewModel.Fit);
    }

    public SettingsViewModel ViewModel { get; } = App.GetService<SettingsViewModel>();

    private void FitCombo_SelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (FitChoices.Selected(FitCombo) is { } fit)
            ViewModel.Fit = fit;
    }

    private void ShowWelcome_Click(object sender, RoutedEventArgs e) =>
        App.Current.MainWindow?.ShowWelcome();
}
