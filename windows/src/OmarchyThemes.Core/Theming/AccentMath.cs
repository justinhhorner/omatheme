using OmarchyThemes.Core.Colors;

namespace OmarchyThemes.Core.Theming;

/// <summary>
/// Color math for Windows accent settings. It lives in Core (not the Windows backend) so it can
/// be unit-tested. Windows stores colors as DWORDs in ABGR (DWM AccentColor) or ARGB
/// (ColorizationColor) order, and the Explorer AccentPalette as 8 RGBA entries.
/// </summary>
public static class AccentMath
{
    /// <summary>Lightness offsets for Light3..Light1 (towards white) and Dark1..Dark3 (towards black).</summary>
    private static readonly double[] Steps = [0.7, 0.45, 0.2];

    public static uint ToAbgr(RgbColor c, byte alpha = 0xFF) =>
        (uint)(alpha << 24 | c.B << 16 | c.G << 8 | c.R);

    public static uint ToArgb(RgbColor c, byte alpha = 0xFF) =>
        (uint)(alpha << 24 | c.R << 16 | c.G << 8 | c.B);

    public static RgbColor FromAbgr(uint value) =>
        new((byte)value, (byte)(value >> 8), (byte)(value >> 16));

    /// <summary>
    /// Keeps a theme accent usable as a Windows accent. Near-black or near-white colors
    /// (common for monochrome themes) would make Start/taskbar highlights unreadable.
    /// </summary>
    public static RgbColor NormalizeAccent(RgbColor accent)
    {
        var (h, s, l) = accent.ToHsl();
        var clamped = Math.Clamp(l, 0.25, 0.75);
        return clamped == l ? accent : RgbColor.FromHsl(h, s, clamped);
    }

    /// <summary>Light3, Light2, Light1, Accent, Dark1, Dark2, Dark3: the seven shades Windows derives from an accent.</summary>
    public static IReadOnlyList<RgbColor> Shades(RgbColor accent)
    {
        var (h, s, l) = accent.ToHsl();
        var shades = new List<RgbColor>(7);
        foreach (var k in Steps)
            shades.Add(RgbColor.FromHsl(h, s, l + (1 - l) * k));
        shades.Add(accent);
        foreach (var k in Enumerable.Reverse(Steps))
            shades.Add(RgbColor.FromHsl(h, s, l * (1 - k)));
        return shades;
    }

    /// <summary>The 32-byte REG_BINARY for HKCU\…\Explorer\Accent\AccentPalette (8 × RGBA; the 8th is a neutral).</summary>
    public static byte[] ToAccentPaletteBytes(RgbColor accent)
    {
        var colors = Shades(accent).Append(new RgbColor(0x76, 0x76, 0x76)).ToArray();
        var bytes = new byte[32];
        for (var i = 0; i < colors.Length; i++)
        {
            bytes[i * 4] = colors[i].R;
            bytes[i * 4 + 1] = colors[i].G;
            bytes[i * 4 + 2] = colors[i].B;
            bytes[i * 4 + 3] = 0x00;
        }
        return bytes;
    }
}
