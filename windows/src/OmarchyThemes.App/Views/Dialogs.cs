using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace OmarchyThemes.App.Views;

public static class Dialogs
{
    /// <summary>Asks before deleting a downloaded theme; true if the user chose Remove.</summary>
    public static async Task<bool> ConfirmRemoveAsync(XamlRoot root, string themeName)
    {
        var confirm = new ContentDialog
        {
            XamlRoot = root,
            Style = (Style)Application.Current.Resources["DefaultContentDialogStyle"],
            Title = $"Remove {themeName}?",
            Content = "The downloaded wallpapers and colors will be deleted from this PC. Your current desktop won't change.",
            PrimaryButtonText = "Remove",
            CloseButtonText = "Cancel",
            DefaultButton = ContentDialogButton.Close,
        };
        return await confirm.ShowAsync() == ContentDialogResult.Primary;
    }
}
