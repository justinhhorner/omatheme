using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.App.Services;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.Views;

/// <summary>Per-aspect Apply choices. Options the OS or the theme can't provide are shown disabled.</summary>
public sealed partial class ApplyDialog : ContentDialog
{
    private ApplyDialog(InstalledTheme theme, ApplyOptions defaults, DesktopCapabilities capabilities, bool hasSnapshot)
    {
        InitializeComponent();
        Title = $"Apply {theme.Name}";

        var unavailable = new List<string>();

        foreach (var fit in Enum.GetValues<WallpaperFit>())
            FitCombo.Items.Add(fit.ToString());
        FitCombo.SelectedItem = defaults.Fit.ToString();
        var canWallpaper = capabilities.HasFlag(DesktopCapabilities.Wallpaper) && theme.Wallpapers.Count > 0;
        WallpaperCheck.IsEnabled = canWallpaper;
        WallpaperCheck.IsChecked = canWallpaper && defaults.Wallpaper;
        if (theme.Wallpapers.Count == 0)
            unavailable.Add("This theme has no wallpaper.");

        ModeCheck.Content = theme.Mode == AppearanceMode.Light ? "Switch Windows to light mode" : "Switch Windows to dark mode";
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
        Fit = Enum.TryParse<WallpaperFit>(FitCombo.SelectedItem as string, out var fit) ? fit : WallpaperFit.Fill,
    };

    /// <summary>Shows the dialog; returns the chosen options, or null if cancelled.</summary>
    public static async Task<ApplyOptions?> ShowAsync(XamlRoot root, InstalledTheme theme)
    {
        var settings = App.GetService<SettingsStore>();
        var apply = App.GetService<ApplyService>();
        var dialog = new ApplyDialog(theme, settings.Load().ApplyDefaults, apply.Capabilities, apply.HasOriginalSnapshot)
        {
            XamlRoot = root,
        };

        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
            return null;

        var options = dialog.Options;
        if (dialog.RememberCheck.IsChecked == true)
            settings.Update(s => s with { ApplyDefaults = options });
        return options;
    }
}
