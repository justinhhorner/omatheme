using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;

namespace OmarchyThemes.Core.Tests;

public sealed class JsonFileTests : IDisposable
{
    private readonly TempDir _dir = new();

    public void Dispose() => _dir.Dispose();

    [Fact]
    public void Writes_with_other_options_as_utf8_without_a_bom_and_no_temp_file_left()
    {
        var path = Path.Combine(_dir.Path, "sub", "other.json");

        JsonFile.WriteAtomic(path, new { SomeKey = "Rosé" }, new JsonSerializerOptions());

        var bytes = File.ReadAllBytes(path);
        Assert.False(bytes.AsSpan().StartsWith(Encoding.UTF8.Preamble));
        Assert.Equal("Rosé", JsonNode.Parse(bytes)!["SomeKey"]!.GetValue<string>());
        Assert.Equal([path], Directory.GetFiles(Path.GetDirectoryName(path)!));
    }

    [Fact]
    public void Byte_writes_replace_the_previous_file()
    {
        var path = Path.Combine(_dir.Path, "sub", "body");

        JsonFile.WriteBytesAtomic(path, [1, 2, 3]);
        JsonFile.WriteBytesAtomic(path, [4]);

        Assert.Equal(new byte[] { 4 }, File.ReadAllBytes(path));
        Assert.Equal([path], Directory.GetFiles(Path.GetDirectoryName(path)!));
    }

    [Fact]
    public void A_failed_write_keeps_the_previous_file_and_leaves_no_temp_file()
    {
        var path = Path.Combine(_dir.Path, "sub", "value.json");
        JsonFile.WriteAtomic(path, new { Value = 1 });

        Assert.ThrowsAny<NotSupportedException>(() => JsonFile.WriteAtomic(path, new { Value = typeof(int) }));

        Assert.Equal(1, JsonNode.Parse(File.ReadAllText(path))!["value"]!.GetValue<int>());
        Assert.Equal([path], Directory.GetFiles(Path.GetDirectoryName(path)!));
    }
}
