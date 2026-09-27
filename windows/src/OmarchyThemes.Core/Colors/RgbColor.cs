using System.Diagnostics.CodeAnalysis;
using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace OmarchyThemes.Core.Colors;

/// <summary>An opaque sRGB color. Serializes to JSON as "#rrggbb".</summary>
[JsonConverter(typeof(RgbColorJsonConverter))]
public readonly record struct RgbColor(byte R, byte G, byte B)
{
    /// <summary>Accepts "#rrggbb", "rrggbb", "0xrrggbb", "#rgb" and "#rrggbbaa" (alpha ignored).</summary>
    public static bool TryParse([NotNullWhen(true)] string? text, out RgbColor color)
    {
        color = default;
        if (string.IsNullOrWhiteSpace(text))
            return false;

        var s = text.AsSpan().Trim();
        if (s.StartsWith("#"))
            s = s[1..];
        else if (s.StartsWith("0x", StringComparison.OrdinalIgnoreCase))
            s = s[2..];

        if (s.Length == 3)
        {
            Span<char> expanded = [s[0], s[0], s[1], s[1], s[2], s[2]];
            return TryParseHex6(expanded, out color);
        }
        if (s.Length is 6 or 8)
            return TryParseHex6(s[..6], out color);
        return false;
    }

    public static RgbColor Parse(string text) =>
        TryParse(text, out var c) ? c : throw new FormatException($"'{text}' is not a hex color.");

    private static bool TryParseHex6(ReadOnlySpan<char> hex, out RgbColor color)
    {
        color = default;
        if (!uint.TryParse(hex, NumberStyles.AllowHexSpecifier, CultureInfo.InvariantCulture, out var v))
            return false;
        color = new RgbColor((byte)(v >> 16), (byte)(v >> 8), (byte)v);
        return true;
    }

    public string ToHex() => $"#{R:x2}{G:x2}{B:x2}";

    public override string ToString() => ToHex();

    /// <summary>WCAG relative luminance, 0 (black) to 1 (white).</summary>
    public double RelativeLuminance => 0.2126 * Linear(R) + 0.7152 * Linear(G) + 0.0722 * Linear(B);

    /// <summary>True for colors that read as a light background.</summary>
    public bool IsLight => RelativeLuminance > 0.4;

    private static double Linear(byte channel)
    {
        var c = channel / 255.0;
        return c <= 0.04045 ? c / 12.92 : Math.Pow((c + 0.055) / 1.055, 2.4);
    }

    /// <summary>Hue in degrees [0,360), saturation and lightness in [0,1].</summary>
    public (double H, double S, double L) ToHsl()
    {
        double r = R / 255.0, g = G / 255.0, b = B / 255.0;
        var max = Math.Max(r, Math.Max(g, b));
        var min = Math.Min(r, Math.Min(g, b));
        var l = (max + min) / 2;
        if (max == min)
            return (0, 0, l);

        var d = max - min;
        var s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
        double h;
        if (max == r) h = (g - b) / d + (g < b ? 6 : 0);
        else if (max == g) h = (b - r) / d + 2;
        else h = (r - g) / d + 4;
        return (h * 60, s, l);
    }

    public static RgbColor FromHsl(double h, double s, double l)
    {
        s = Math.Clamp(s, 0, 1);
        l = Math.Clamp(l, 0, 1);
        if (s == 0)
        {
            var v = ToByte(l);
            return new RgbColor(v, v, v);
        }

        h = ((h % 360) + 360) % 360 / 360;
        var q = l < 0.5 ? l * (1 + s) : l + s - l * s;
        var p = 2 * l - q;
        return new RgbColor(
            ToByte(HueToRgb(p, q, h + 1.0 / 3)),
            ToByte(HueToRgb(p, q, h)),
            ToByte(HueToRgb(p, q, h - 1.0 / 3)));
    }

    private static double HueToRgb(double p, double q, double t)
    {
        if (t < 0) t += 1;
        if (t > 1) t -= 1;
        if (t < 1.0 / 6) return p + (q - p) * 6 * t;
        if (t < 1.0 / 2) return q;
        if (t < 2.0 / 3) return p + (q - p) * (2.0 / 3 - t) * 6;
        return p;
    }

    private static byte ToByte(double unit) => (byte)Math.Round(Math.Clamp(unit, 0, 1) * 255);
}

public sealed class RgbColorJsonConverter : JsonConverter<RgbColor>
{
    public override RgbColor Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        RgbColor.TryParse(reader.GetString(), out var c)
            ? c
            : throw new JsonException($"Invalid color '{reader.GetString()}'.");

    public override void Write(Utf8JsonWriter writer, RgbColor value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.ToHex());
}
