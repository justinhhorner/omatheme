using Windows.Graphics.Imaging;

namespace OmarchyThemes.Platform.Windows.Wallpaper;

public interface IImageConverter
{
    /// <summary>Returns a path Windows can use as a wallpaper, converting the image to PNG if needed.</summary>
    Task<string> EnsureWallpaperFormatAsync(string path, CancellationToken ct = default);
}

/// <summary>
/// Converts formats the wallpaper APIs don't reliably accept (notably WebP, which many Omarchy
/// themes ship) to PNG with the Windows Imaging Component, via WinRT BitmapDecoder/Encoder.
/// The converted file sits next to the original, so it's removed along with the theme.
/// </summary>
public sealed class WicImageConverter : IImageConverter
{
    private static readonly HashSet<string> NativeFormats = new(StringComparer.OrdinalIgnoreCase)
        { ".jpg", ".jpeg", ".png", ".bmp" };

    public static string ConvertedPathFor(string path) =>
        Path.Combine(Path.GetDirectoryName(path)!, "." + Path.GetFileNameWithoutExtension(path) + ".wallpaper.png");

    public async Task<string> EnsureWallpaperFormatAsync(string path, CancellationToken ct = default)
    {
        if (NativeFormats.Contains(Path.GetExtension(path)))
            return path;

        var target = ConvertedPathFor(path);
        if (File.Exists(target) && File.GetLastWriteTimeUtc(target) >= File.GetLastWriteTimeUtc(path))
            return target;

        var temp = target + ".tmp";
        try
        {
            using (var input = File.OpenRead(path).AsRandomAccessStream())
            using (var output = File.Create(temp).AsRandomAccessStream())
            {
                var decoder = await BitmapDecoder.CreateAsync(input).AsTask(ct).ConfigureAwait(false);
                using var bitmap = await decoder.GetSoftwareBitmapAsync(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied)
                    .AsTask(ct).ConfigureAwait(false);
                var encoder = await BitmapEncoder.CreateAsync(BitmapEncoder.PngEncoderId, output).AsTask(ct).ConfigureAwait(false);
                encoder.SetSoftwareBitmap(bitmap);
                await encoder.FlushAsync().AsTask(ct).ConfigureAwait(false);
            }
            File.Move(temp, target, overwrite: true);
            return target;
        }
        catch (Exception e) when (e is not OperationCanceledException and not IOException)
        {
            throw new InvalidOperationException(
                $"Windows couldn't read {Path.GetFileName(path)}. If it's a WebP image, install the \"WebP Image Extensions\" from the Microsoft Store.", e);
        }
        finally
        {
            File.Delete(temp);
        }
    }
}
