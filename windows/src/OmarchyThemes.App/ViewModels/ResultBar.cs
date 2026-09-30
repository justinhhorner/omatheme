using CommunityToolkit.Mvvm.ComponentModel;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.App.Helpers;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Stores;

namespace OmarchyThemes.App.ViewModels;

/// <summary>A page's result InfoBar: the outcome of the last thing the user did there.</summary>
public sealed partial class ResultBar : ObservableObject
{
    [ObservableProperty]
    public partial bool IsOpen { get; set; }

    [ObservableProperty]
    public partial string Title { get; set; } = "";

    [ObservableProperty]
    public partial string Message { get; set; } = "";

    [ObservableProperty]
    public partial InfoBarSeverity Severity { get; set; }

    public void Show(InfoBarSeverity severity, string title, string message)
    {
        Severity = severity;
        Title = title;
        Message = message;
        IsOpen = true;
    }

    public void Show(Banner banner) => Show(Ui.Severity(banner.Kind), banner.Title, banner.Message);

    public void Show(ApplySummary summary) => Show(Ui.Severity(summary.Kind), summary.Title, summary.Message);

    public void Close() => IsOpen = false;
}
