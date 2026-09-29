using System.Globalization;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace OmarchyThemes.Core.Storage;

/// <summary>
/// Shared JSON settings and crash-safe read/write helpers for the app's local files. The format is
/// the contract in docs/data-format.md, shared with the macOS app: camelCase keys, absent values
/// omitted, UTC dates with milliseconds and "Z".
/// </summary>
public static class JsonFile
{
    public static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.CamelCase), new UtcDateJsonConverter() },
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
    public static void WriteAtomic<T>(string path, T value) => WriteAtomic(path, value, Options);

    /// <summary>
    /// <see cref="WriteAtomic{T}(string, T)"/> with other serializer options, for files in another
    /// program's format. UTF-8 without a byte-order mark.
    /// </summary>
    public static void WriteAtomic<T>(string path, T value, JsonSerializerOptions options) =>
        ReplaceAtomic(path, stream => JsonSerializer.Serialize(stream, value, options));

    /// <summary>Writes bytes via a temp file + rename.</summary>
    public static void WriteBytesAtomic(string path, byte[] bytes) =>
        ReplaceAtomic(path, stream => stream.Write(bytes));

    private static void ReplaceAtomic(string path, Action<Stream> write)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temp = path + ".tmp";
        using (var stream = File.Create(temp))
            write(stream);
        File.Move(temp, path, overwrite: true);
    }
}

/// <summary>
/// Dates as ISO 8601 UTC with milliseconds and "Z" ("2026-09-27T09:00:00.123Z"), per
/// docs/data-format.md. Reads any ISO 8601 form; one without a zone is UTC (older macOS files).
/// </summary>
public sealed class UtcDateJsonConverter : JsonConverter<DateTimeOffset>
{
    public override DateTimeOffset Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options) =>
        DateTimeOffset.TryParse(reader.GetString(), CultureInfo.InvariantCulture,
            DateTimeStyles.AssumeUniversal | DateTimeStyles.AdjustToUniversal, out var date)
            ? date
            : throw new JsonException($"Invalid date '{reader.GetString()}'.");

    public override void Write(Utf8JsonWriter writer, DateTimeOffset value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.UtcDateTime.ToString("yyyy-MM-dd'T'HH:mm:ss.fff'Z'", CultureInfo.InvariantCulture));
}
