using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.App.ViewModels;

/// <summary>"Add to Windows Terminal": the theme's colors as a Terminal color scheme.</summary>
public sealed partial class ThemeDetailViewModel
{
    [ObservableProperty]
    [NotifyPropertyChangedFor(nameof(TerminalButtonText), nameof(TerminalHint))]
    public partial bool IsInTerminal { get; set; }

    /// <summary>Needs a palette (downloaded or not) and the terminal.</summary>
    public bool CanUseTerminal => _terminal.IsAvailable && Palette is not null;

    public bool ShowTerminal => Palette is not null;

    public string TerminalButtonText => IsInTerminal ? $"Remove from {_terminal.DisplayName}" : $"Add to {_terminal.DisplayName}";

    public string TerminalSchemeName => _terminal.SchemeName(Entry);

    public string TerminalHint =>
        !_terminal.IsAvailable ? $"Install {_terminal.DisplayName} to use these colors in your terminal."
        : IsInTerminal ? $"Available in {_terminal.DisplayName} as “{TerminalSchemeName}”."
        : $"Adds these colors to {_terminal.DisplayName} as a color scheme. Your Terminal settings aren't changed.";

    partial void OnInstalledChanged(InstalledTheme? value) => OnPaletteChanged();

    partial void OnDetailsChanged(ThemeDetails? value) => OnPaletteChanged();

    private void OnPaletteChanged()
    {
        OnPropertyChanged(nameof(CanUseTerminal));
        OnPropertyChanged(nameof(ShowTerminal));
    }

    private void RefreshTerminalState()
    {
        IsInTerminal = _terminal.IsAdded(Entry);
        OnPropertyChanged(nameof(TerminalSchemeName));
        OnPaletteChanged();
    }

    [RelayCommand]
    private async Task ToggleTerminalSchemeAsync()
    {
        if (IsInTerminal)
            Result.Show(_terminal.Remove(Entry));
        else if (Palette is { } palette)
            Result.Show(await _terminal.AddAsync(Entry, palette));
        IsInTerminal = _terminal.IsAdded(Entry);
    }
}
