using System.Text.RegularExpressions;
using Tomlyn;
using Tomlyn.Model;

namespace OmarchyThemes.Core.Palettes;

/// <summary>
/// Reads a TOML file into "section.key" → string pairs (non-string values are dropped;
/// palettes only need strings). Community theme files are often hand-edited, so if strict
/// TOML parsing fails we fall back to a lenient line scanner rather than rejecting the theme.
/// </summary>
internal static partial class FlatToml
{
    public static Dictionary<string, string> Parse(string text)
    {
        try
        {
            var table = TomlSerializer.Deserialize<TomlTable>(text);
            var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
            if (table is not null)
                Flatten(table, prefix: "", result);
            return result;
        }
        catch (Exception)
        {
            return ParseLenient(text);
        }
    }

    private static void Flatten(TomlTable table, string prefix, Dictionary<string, string> result)
    {
        foreach (var (key, value) in table)
        {
            var fullKey = prefix + key;
            switch (value)
            {
                case string s:
                    result[fullKey] = s;
                    break;
                case TomlTable child:
                    Flatten(child, fullKey + ".", result);
                    break;
            }
        }
    }

    internal static Dictionary<string, string> ParseLenient(string text)
    {
        var result = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);
        var section = "";
        foreach (var rawLine in text.Split('\n'))
        {
            var line = rawLine.Trim();
            if (line.Length == 0 || line[0] == '#')
                continue;

            var header = SectionRegex().Match(line);
            if (header.Success)
            {
                section = header.Groups[1].Value.Replace(" ", "").Replace("\"", "") + ".";
                continue;
            }

            var kv = KeyValueRegex().Match(line);
            if (kv.Success)
                result[section + kv.Groups[1].Value] = kv.Groups[2].Value;
        }
        return result;
    }

    [GeneratedRegex(@"^\[\s*([^\[\]]+?)\s*\]")]
    private static partial Regex SectionRegex();

    [GeneratedRegex("""^([A-Za-z0-9_\-]+)\s*=\s*["']([^"']*)["']""")]
    private static partial Regex KeyValueRegex();
}
