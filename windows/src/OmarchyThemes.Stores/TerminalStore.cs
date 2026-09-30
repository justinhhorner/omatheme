using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores;

/// <summary>Sending a theme's colors to a terminal app as a color scheme (see <see cref="ITerminalSchemes"/>).</summary>
public sealed class TerminalStore
{
    private readonly ITerminalSchemes _terminal;
    private readonly ThemeLibrary _library;
    private readonly ILogger _log;

    public TerminalStore(ITerminalSchemes terminal, ThemeLibrary library, ILogger<TerminalStore>? log = null)
    {
        _terminal = terminal;
        _library = library;
        _log = log ?? NullLogger<TerminalStore>.Instance;
    }

    /// <summary>"Windows Terminal".</summary>
    public string DisplayName => _terminal.DisplayName;

    /// <summary>The terminal is installed.</summary>
    public bool IsAvailable => _terminal.IsAvailable;

    /// <summary>The scheme's name in the terminal ("Tokyo Night (Omarchy)").</summary>
    public string SchemeName(CatalogEntry entry) => _terminal.SchemeName(ThemeName(entry));

    public bool IsAdded(CatalogEntry entry) => _terminal.IsAdded(entry.Slug);

    public async Task<Banner> AddAsync(CatalogEntry entry, Palette palette)
    {
        try
        {
            var colors = TerminalColors.From(await PaletteForTerminalAsync(entry, palette));
            _terminal.Add(entry.Slug, ThemeName(entry), colors);
            return new Banner(SummaryKind.Success, $"Added to {DisplayName}", _terminal.AddedMessage(SchemeName(entry)));
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            _log.LogError(e, "Adding {Slug} to {Terminal} failed", entry.Slug, DisplayName);
            return new Banner(SummaryKind.Error, $"Couldn't update {DisplayName}", e.Message);
        }
    }

    public Banner Remove(CatalogEntry entry)
    {
        try
        {
            _terminal.Remove(entry.Slug);
            return new Banner(SummaryKind.Info, $"Removed from {DisplayName}", _terminal.RemovedMessage(SchemeName(entry)));
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            _log.LogError(e, "Removing {Slug} from {Terminal} failed", entry.Slug, DisplayName);
            return new Banner(SummaryKind.Error, $"Couldn't update {DisplayName}", e.Message);
        }
    }

    /// <summary>The downloaded theme's name if there is one (it's what was saved), else the catalog's.</summary>
    private string ThemeName(CatalogEntry entry) => _library.Get(entry.Slug)?.Name ?? entry.Name;

    /// <summary>
    /// Themes downloaded before `muted` and `bright_foreground` were read (named colors.toml) saved a
    /// palette without them, so look the theme up again (cached, usually free) to get Omarchy's exact
    /// bright black, bright white and cursor. Offline, use what's saved.
    /// </summary>
    private async Task<Palette> PaletteForTerminalAsync(CatalogEntry entry, Palette saved)
    {
        var savedWithoutNamedExtras = saved is { Source: PaletteSource.ColorsToml, Muted: null, BrightForeground: null }
            && !saved.Swatches.Any(s => s.Name == AnsiColors.SwatchName(8));
        if (_library.Get(entry.Slug) is null || !savedWithoutNamedExtras)
            return saved;
        try
        {
            return (await _library.ResolveAsync(entry)).Palette ?? saved;
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            return saved;
        }
    }
}
