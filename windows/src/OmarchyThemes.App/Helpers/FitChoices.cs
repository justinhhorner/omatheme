using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.Helpers;

/// <summary>A wallpaper fit combo box (Apply dialog, Settings): one item per fit, by name.</summary>
public static class FitChoices
{
    public static void Fill(ComboBox combo, WallpaperFit selected)
    {
        foreach (var fit in Enum.GetValues<WallpaperFit>())
            combo.Items.Add(fit.ToString());
        combo.SelectedItem = selected.ToString();
    }

    public static WallpaperFit? Selected(ComboBox combo) =>
        Enum.TryParse<WallpaperFit>(combo.SelectedItem as string, out var fit) ? fit : null;
}
