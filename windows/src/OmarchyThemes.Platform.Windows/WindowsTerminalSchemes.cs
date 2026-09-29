using System.Text.Json;
using System.Text.Json.Serialization;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Platform.Windows;

/// <summary>
/// Adds a theme's colors to Windows Terminal as a color scheme, through Terminal's JSON fragment
/// extensions: one file per theme in the per-user fragments folder
/// (<c>%LOCALAPPDATA%\Microsoft\Windows Terminal\Fragments\OmarchyThemes\&lt;slug&gt;.json</c>).
/// Terminal merges fragments into its settings, so the user's settings.json is never edited, and
/// removing the file removes the scheme. Terminal reads fragments when it starts.
/// See https://learn.microsoft.com/windows/terminal/json-fragment-extensions.
/// </summary>
public sealed class WindowsTerminalSchemes(string fragmentsDir)
{
    private static readonly JsonSerializerOptions Json = new() { WriteIndented = true };

    public static string DefaultFragmentsDir => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
        "Microsoft", "Windows Terminal", "Fragments", "OmarchyThemes");

    public string FragmentsDir { get; } = fragmentsDir;

    /// <summary>"Tokyo Night (Omarchy)": the suffix keeps it apart from Terminal's and the user's own schemes.</summary>
    public static string SchemeName(string themeName) => $"{themeName} (Omarchy)";

    /// <summary>True when Windows Terminal (stable or Preview) is installed for this user.</summary>
    public static bool IsTerminalInstalled()
    {
        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        return File.Exists(Path.Combine(local, "Microsoft", "WindowsApps", "wt.exe"))
            || Directory.Exists(Path.Combine(local, "Packages", "Microsoft.WindowsTerminal_8wekyb3d8bbwe"))
            || Directory.Exists(Path.Combine(local, "Packages", "Microsoft.WindowsTerminalPreview_8wekyb3d8bbwe"));
    }

    public bool IsAdded(string slug) => File.Exists(FilePath(slug));

    /// <summary>Writes (or replaces) the theme's scheme. UTF-8 without a BOM, as Terminal requires.</summary>
    public void Add(string slug, string themeName, TerminalColors colors) =>
        JsonFile.WriteAtomic(FilePath(slug), new Fragment([Scheme.From(SchemeName(themeName), colors)]), Json);

    public void Remove(string slug)
    {
        var path = FilePath(slug);
        if (!Directory.Exists(FragmentsDir))
            return;
        File.Delete(path);
        if (!Directory.EnumerateFileSystemEntries(FragmentsDir).Any())
            Directory.Delete(FragmentsDir);
    }

    internal string FilePath(string slug)
    {
        if (!AppPaths.IsValidSlug(slug))
            throw new ArgumentException($"Invalid theme slug '{slug}'.", nameof(slug));
        return Path.Combine(FragmentsDir, slug + ".json");
    }

    private sealed record Fragment([property: JsonPropertyName("schemes")] IReadOnlyList<Scheme> Schemes);

    /// <summary>A Windows Terminal color scheme: a name and every color in the table are required.</summary>
    private sealed record Scheme(
        [property: JsonPropertyName("name")] string Name,
        [property: JsonPropertyName("background")] string Background,
        [property: JsonPropertyName("foreground")] string Foreground,
        [property: JsonPropertyName("cursorColor")] string CursorColor,
        [property: JsonPropertyName("selectionBackground")] string SelectionBackground,
        [property: JsonPropertyName("black")] string Black,
        [property: JsonPropertyName("red")] string Red,
        [property: JsonPropertyName("green")] string Green,
        [property: JsonPropertyName("yellow")] string Yellow,
        [property: JsonPropertyName("blue")] string Blue,
        [property: JsonPropertyName("purple")] string Purple,
        [property: JsonPropertyName("cyan")] string Cyan,
        [property: JsonPropertyName("white")] string White,
        [property: JsonPropertyName("brightBlack")] string BrightBlack,
        [property: JsonPropertyName("brightRed")] string BrightRed,
        [property: JsonPropertyName("brightGreen")] string BrightGreen,
        [property: JsonPropertyName("brightYellow")] string BrightYellow,
        [property: JsonPropertyName("brightBlue")] string BrightBlue,
        [property: JsonPropertyName("brightPurple")] string BrightPurple,
        [property: JsonPropertyName("brightCyan")] string BrightCyan,
        [property: JsonPropertyName("brightWhite")] string BrightWhite)
    {
        public static Scheme From(string name, TerminalColors c)
        {
            var a = c.Ansi.Select(x => x.ToHex()).ToArray();
            return new Scheme(name, c.Background.ToHex(), c.Foreground.ToHex(), c.Cursor.ToHex(), c.SelectionBackground.ToHex(),
                a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7],
                a[8], a[9], a[10], a[11], a[12], a[13], a[14], a[15]);
        }
    }
}
