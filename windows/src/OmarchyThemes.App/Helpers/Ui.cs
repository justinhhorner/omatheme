using Microsoft.UI;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.App.Helpers;

/// <summary>Small conversions used from x:Bind function bindings.</summary>
public static class Ui
{
    public static Visibility Visible(bool value) => value ? Visibility.Visible : Visibility.Collapsed;

    public static Visibility Collapsed(bool value) => value ? Visibility.Collapsed : Visibility.Visible;

    public static Visibility VisibleIf(object? value) => value is null or "" ? Visibility.Collapsed : Visibility.Visible;

    public static bool Not(bool value) => !value;

    public static bool IsSet(string? value) => !string.IsNullOrEmpty(value);

    /// <summary>
    /// The data item behind an element in an item template. ItemsRepeater doesn't set DataContext for
    /// compiled (x:Bind) templates, so template elements carry their item in <c>Tag="{x:Bind}"</c>.
    /// </summary>
    public static T? ItemOf<T>(object? sender) where T : class =>
        sender is FrameworkElement element ? element.Tag as T ?? element.DataContext as T : null;

    public static SolidColorBrush Brush(RgbColor color) => new(ColorHelper.FromArgb(255, color.R, color.G, color.B));

    /// <summary>Text color that stays readable on top of <paramref name="background"/>.</summary>
    public static SolidColorBrush ContrastBrush(RgbColor background) =>
        new(background.IsLight ? Colors.Black : Colors.White);

    /// <summary>Image from an http(s) URL or a local file path, decoded at roughly the size shown.</summary>
    public static ImageSource? Image(string? source, int decodeWidth)
    {
        if (string.IsNullOrEmpty(source) || !Uri.TryCreate(source, UriKind.Absolute, out var uri))
            return null;
        return new BitmapImage
        {
            DecodePixelWidth = decodeWidth,
            DecodePixelType = DecodePixelType.Logical,
            UriSource = uri,
        };
    }

    public static InfoBarSeverity Severity(SummaryKind kind) => kind switch
    {
        SummaryKind.Success => InfoBarSeverity.Success,
        SummaryKind.Warning => InfoBarSeverity.Warning,
        SummaryKind.Error => InfoBarSeverity.Error,
        _ => InfoBarSeverity.Informational,
    };

    public static string Ago(DateTimeOffset when, DateTimeOffset now)
    {
        var age = now - when;
        return age.TotalMinutes < 1 ? "just now"
            : age.TotalHours < 1 ? $"{(int)age.TotalMinutes} min ago"
            : age.TotalDays < 1 ? $"{(int)age.TotalHours} h ago"
            : age.TotalDays < 2 ? "yesterday"
            : when.ToLocalTime().ToString("d MMM");
    }

    public static string FormatBytes(long bytes) => bytes switch
    {
        < 1024 => $"{bytes} B",
        < 1024 * 1024 => $"{bytes / 1024.0:0} KB",
        < 1024L * 1024 * 1024 => $"{bytes / 1024.0 / 1024:0.0} MB",
        _ => $"{bytes / 1024.0 / 1024 / 1024:0.00} GB",
    };
}
