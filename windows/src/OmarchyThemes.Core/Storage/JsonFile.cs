using System.Text.Json;
using System.Text.Json.Serialization;

namespace OmarchyThemes.Core.Storage;

/// <summary>Shared JSON settings and crash-safe read/write helpers for the app's local files.</summary>
public static class JsonFile
{
    public static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase) },
    };

    /// <summary>Reads <paramref name="path"/>, returning null if it is missing or unreadable.</summary>
    public static T? TryRead<T>(string path) where T : class
    {
        try
        {
            if (!File.Exists(path))
                return null;
            using var stream = File.OpenRead(path);
            return JsonSerializer.Deserialize<T>(stream, Options);
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException)
        {
            return null;
        }
    }

    /// <summary>Writes via a temp file + rename so a crash never leaves a half-written file.</summary>
    public static void WriteAtomic<T>(string path, T value)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temp = path + ".tmp";
        using (var stream = File.Create(temp))
            JsonSerializer.Serialize(stream, value, Options);
        File.Move(temp, path, overwrite: true);
    }

    /// <summary>Writes bytes via a temp file + rename.</summary>
    public static void WriteBytesAtomic(string path, byte[] bytes)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temp = path + ".tmp";
        File.WriteAllBytes(temp, bytes);
        File.Move(temp, path, overwrite: true);
    }
}
