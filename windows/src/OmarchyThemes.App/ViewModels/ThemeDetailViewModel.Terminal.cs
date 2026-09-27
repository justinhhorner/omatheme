using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Microsoft.UI.Xaml.Controls;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Platform.Windows;

namespace OmarchyThemes.App.ViewModels;

/// <summary>"Add to Windows Terminal": the theme's colors as a Terminal color scheme.</summary>
public sealed partial class ThemeDetailViewModel
{
    private static readonly bool TerminalInstalled = WindowsTerminalSchemes.IsTerminalInstalled();

    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(TerminalButtonText), nameof(TerminalHint))]
    public partial bool IsInTerminal { get; set; }

    /// <summary>Needs a palette (downloaded or not) and Windows Terminal.</summary>
    public bool CanUseTerminal => TerminalInstalled && Palette is not null;

    public bool ShowTerminal => Palette is not null;

    public string TerminalButtonText => IsInTerminal ? "Remove from Windows Terminal" : "Add to Windows Terminal";

    public string TerminalSchemeName => WindowsTerminalSchemes.SchemeName(Name);

    public string TerminalHint =>
        !TerminalInstalled ? "Install Windows Terminal to use these colors in your terminal."
        : IsInTerminal ? $"Available in Windows Terminal as “{TerminalSchemeName}”."
        : "Adds these colors to Windows Terminal as a color scheme. Your Terminal settings aren't changed.";

    partial void OnInstalledChanged(InstalledTheme? value) => OnPaletteChanged();

    partial void OnDetailsChanged(ThemeDetails? value) => OnPaletteChanged();

    private void OnPaletteChanged()
    {
        OnPropertyChanged(nameof(CanUseTerminal));
        OnPropertyChanged(nameof(ShowTerminal));
    }

    private void RefreshTerminalState()
    {
        IsInTerminal = _terminal.IsAdded(Entry.Slug);
        OnPropertyChanged(nameof(TerminalSchemeName));
        OnPaletteChanged();
    }

    /// <summary>
    /// The palette to export. Themes downloaded before the app read `muted` and `bright_foreground`
    /// (named colors.toml) saved a palette without them, so look the theme up again (cached, usually
    /// free) to get Omarchy's exact bright black, bright white and cursor. Offline, use what's saved.
    /// </summary>
    private async Task<Palette?> PaletteForTerminalAsync()
    {
        var palette = Palette;
        var savedWithoutNamedExtras = palette is { Source: PaletteSource.ColorsToml, Muted: null, BrightForeground: null }
            && !palette.Swatches.Any(s => s.Name == "Bright black");
        if (Installed is null || !savedWithoutNamedExtras)
            return palette;
        try
        {
            return (await _details.ResolveAsync(Entry)).Palette ?? palette;
        }
        catch (Exception e) when (e is GitHubException or ThemeResolveException or HttpRequestException or TaskCanceledException)
        {
            return palette;
        }
    }

    [RelayCommand]
    private async Task ToggleTerminalSchemeAsync()
    {
        try
        {
            if (IsInTerminal)
            {
                _terminal.Remove(Entry.Slug);
                IsInTerminal = false;
                ShowResult(InfoBarSeverity.Informational, "Removed from Windows Terminal",
                    $"“{TerminalSchemeName}” is gone after Terminal restarts. A profile that used it goes back to Terminal's default colors.");
                return;
            }

            if (await PaletteForTerminalAsync() is not { } palette)
                return;
            _terminal.Add(Entry.Slug, Name, TerminalColors.From(palette));
            IsInTerminal = true;
            ShowResult(InfoBarSeverity.Success, "Added to Windows Terminal",
                $"In Windows Terminal, open Settings and choose “{TerminalSchemeName}” as a profile's color scheme "
                + "(Profiles › Defaults › Appearance applies it to all of them). If Terminal is open, restart it first.");
        }
        catch (Exception e) when (e is IOException or UnauthorizedAccessException)
        {
            ShowResult(InfoBarSeverity.Error, "Couldn't update Windows Terminal", e.Message);
        }
    }
}
