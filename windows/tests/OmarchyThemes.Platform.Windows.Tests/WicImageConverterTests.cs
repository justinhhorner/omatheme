using OmarchyThemes.Platform.Windows.Wallpaper;
using Windows.Graphics.Imaging;

namespace OmarchyThemes.Platform.Windows.Tests;

/// <summary>Exercises the real Windows Imaging Component, but only on files in a temp folder.</summary>
public sealed class WicImageConverterTests : IDisposable
{
    private readonly string _dir = Path.Combine(Path.GetTempPath(), "omatheme-wic-tests", Guid.NewGuid().ToString("N"));
    private readonly WicImageConverter _converter = new();

    public WicImageConverterTests() => Directory.CreateDirectory(_dir);

    public void Dispose()
    {
        try { Directory.Delete(_dir, recursive: true); }
        catch (IOException) { }
    }

    [Theory]
    [InlineData("a.jpg")]
    [InlineData("a.JPEG")]
    [InlineData("a.png")]
    [InlineData("a.bmp")]
    public async Task Leaves_native_formats_alone(string name)
    {
        var path = Path.Combine(_dir, name);
        Assert.Equal(path, await _converter.EnsureWallpaperFormatAsync(path));
    }

    [Fact]
    public async Task Converts_other_formats_to_png_next_to_the_original_and_reuses_it()
    {
        // GIF stands in for WebP: both go through the same WIC decode path, and a GIF encoder
        // is always available for creating the input.
        var gif = Path.Combine(_dir, "1.gif");
        await WriteImageAsync(gif, BitmapEncoder.GifEncoderId);

        var converted = await _converter.EnsureWallpaperFormatAsync(gif);

        Assert.Equal(WicImageConverter.ConvertedPathFor(gif), converted);
        Assert.Equal(new byte[] { 0x89, (byte)'P', (byte)'N', (byte)'G' }, File.ReadAllBytes(converted)[..4]);

        var stamp = File.GetLastWriteTimeUtc(converted);
        Assert.Equal(converted, await _converter.EnsureWallpaperFormatAsync(gif));
        Assert.Equal(stamp, File.GetLastWriteTimeUtc(converted));
    }

    [Fact]
    public async Task Unreadable_images_give_an_actionable_error()
    {
        var bogus = Path.Combine(_dir, "broken.webp");
        File.WriteAllText(bogus, "not an image");

        var e = await Assert.ThrowsAsync<InvalidOperationException>(() => _converter.EnsureWallpaperFormatAsync(bogus));
        Assert.Contains("broken.webp", e.Message);
        Assert.False(File.Exists(WicImageConverter.ConvertedPathFor(bogus)));
    }

    private static async Task WriteImageAsync(string path, Guid encoderId)
    {
        using var stream = File.Create(path).AsRandomAccessStream();
        var encoder = await BitmapEncoder.CreateAsync(encoderId, stream);
        var pixels = Enumerable.Repeat(new byte[] { 0x26, 0x1b, 0x1a, 0xff }, 16 * 16).SelectMany(p => p).ToArray();
        encoder.SetPixelData(BitmapPixelFormat.Bgra8, BitmapAlphaMode.Premultiplied, 16, 16, 96, 96, pixels);
        await encoder.FlushAsync();
    }
}
