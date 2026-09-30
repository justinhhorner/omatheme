using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.Views;

/// <summary>What the user chose in the Apply dialog.</summary>
/// <param name="Remember">"Use these choices for one-click apply" was checked.</param>
public sealed record ApplyChoice(ApplyOptions Options, bool Remember);

/// <summary>Per-aspect Apply choices. Options the OS or the theme can't provide are shown disabled.</summary>
public sealed partial class ApplyDialog : ContentDialog
{
    private ApplyDialog(InstalledTheme theme, ApplyOptions defaults, DesktopCapabilities capabilities, bool hasSnapshot)
    {
        InitializeComponent();
        Title = $"Apply {theme.Name}";

        var unavailable = new List<string>();

        FitChoices.Fill(FitCombo, defaults.Fit);
        var canWallpaper = capabilities.HasFlag(DesktopCapabilities.Wallpaper) && theme.Wallpapers.Count > 0;
        WallpaperCheck.IsEnabled = canWallpaper;
        WallpaperCheck.IsChecked = canWallpaper && defaults.Wallpaper;
        if (theme.Wallpapers.Count == 0)
            unavailable.Add("This theme has no wallpaper.");

        ModeCheck.Content = $"Switch Windows to {Ui.ModeName(theme.Mode).ToLowerInvariant()} mode";
        var canMode = capabilities.HasFlag(DesktopCapabilities.AppearanceMode);
        ModeCheck.IsEnabled = canMode;
        ModeCheck.IsChecked = canMode && defaults.AppearanceMode;

        var canAccent = capabilities.HasFlag(DesktopCapabilities.AccentColor) && theme.Palette is not null;
        AccentCheck.IsEnabled = canAccent;
        AccentCheck.IsChecked = canAccent && defaults.AccentColor;
        if (theme.Palette is { } palette)
        {
            AccentSwatch.Background = Ui.Brush(palette.Accent);
            AccentText.Text = $"Use the theme's accent color ({palette.Accent.ToHex()})";
        }
        else
        {
            AccentSwatch.Visibility = Visibility.Collapsed;
            unavailable.Add("The theme's palette couldn't be read, so its accent color isn't available.");
        }

        if (unavailable.Count > 0)
        {
            UnavailableText.Text = string.Join(" ", unavailable);
            UnavailableText.Visibility = Visibility.Visible;
        }

        Footnote.Text = hasSnapshot
            ? "You can go back to your original desktop any time from Settings."
            : "Your current wallpaper and colors will be saved first, so you can restore them from Settings.";

        foreach (var check in new[] { WallpaperCheck, ModeCheck, AccentCheck })
        {
            check.Checked += (_, _) => UpdateState();
            check.Unchecked += (_, _) => UpdateState();
        }
        UpdateState();
    }

    private void UpdateState()
    {
        IsPrimaryButtonEnabled = AnyChecked;
        FitCombo.IsEnabled = WallpaperCheck.IsChecked == true;
    }

    private bool AnyChecked => WallpaperCheck.IsChecked == true || ModeCheck.IsChecked == true || AccentCheck.IsChecked == true;

    private ApplyOptions Options => new()
    {
        Wallpaper = WallpaperCheck.IsChecked == true,
        AppearanceMode = ModeCheck.IsChecked == true,
        AccentColor = AccentCheck.IsChecked == true,
        Fit = FitChoices.Selected(FitCombo) ?? WallpaperFit.Fill,
    };

    /// <summary>Shows the dialog, starting from the one-click defaults; null if cancelled.</summary>
    public static async Task<ApplyChoice?> ShowAsync(XamlRoot root, InstalledTheme theme, DesktopStore desktop)
    {
        var dialog = new ApplyDialog(theme, desktop.ApplyDefaults, desktop.Capabilities, desktop.HasOriginalSnapshot)
        {
            XamlRoot = root,
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary
            ? new ApplyChoice(dialog.Options, dialog.RememberCheck.IsChecked == true)
            : null;
    }
}
