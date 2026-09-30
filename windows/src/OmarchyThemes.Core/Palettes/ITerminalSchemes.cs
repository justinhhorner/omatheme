namespace OmarchyThemes.Core.Palettes;

/// <summary>
/// A terminal app that can take a theme's colors as a color scheme (Windows: Windows Terminal).
/// Implementations only add and remove their own files and never edit the terminal's settings, so
/// supporting another terminal is one new class.
/// </summary>
public interface ITerminalSchemes
{
    /// <summary>"Windows Terminal".</summary>
    string DisplayName { get; }

    /// <summary>The terminal is installed.</summary>
    bool IsAvailable { get; }

    /// <summary>The scheme's name as the terminal lists it.</summary>
    string SchemeName(string themeName);

    bool IsAdded(string slug);

    /// <summary>Adds (or replaces) the theme's scheme.</summary>
    void Add(string slug, string themeName, TerminalColors colors);

    void Remove(string slug);

    /// <summary>How to use the scheme once it's added.</summary>
    string AddedMessage(string schemeName);

    /// <summary>What happens now it's removed.</summary>
    string RemovedMessage(string schemeName);
}
