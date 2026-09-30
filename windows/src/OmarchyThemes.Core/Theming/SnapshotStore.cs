using System.Text.Json;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Core.Theming;

/// <summary>Where <see cref="ThemeApplier"/> keeps the desktop it saved before the first Apply.</summary>
public interface ISnapshotStore
{
    /// <summary>The saved desktop, or null if none was saved.</summary>
    /// <exception cref="SnapshotUnreadableException">One was saved but can't be read.</exception>
    DesktopSnapshot? Load();

    void Save(DesktopSnapshot snapshot);
    void Clear();
}

/// <summary>
/// The saved original desktop exists but can't be read. It's the only way back to the user's own
/// desktop, so it must not be treated as missing (and replaced by the themed desktop).
/// </summary>
public sealed class SnapshotUnreadableException(string message, Exception? inner = null) : IOException(message, inner);

/// <summary>original-desktop.json in the data folder (docs/data-format.md).</summary>
public sealed class FileSnapshotStore(AppPaths paths) : ISnapshotStore
{
    public DesktopSnapshot? Load()
    {
        var path = paths.SnapshotFile;
        if (!File.Exists(path))
            return null;
        try
        {
            using var stream = File.OpenRead(path);
            return JsonSerializer.Deserialize<DesktopSnapshot>(stream, JsonFile.Options)
                ?? throw new JsonException("The file is empty.");
        }
        catch (Exception e) when (e is JsonException or IOException or UnauthorizedAccessException)
        {
            throw new SnapshotUnreadableException(
                $"The saved copy of your original desktop can't be read ({Path.GetFileName(path)} in the data folder).", e);
        }
    }

    public void Save(DesktopSnapshot snapshot) => JsonFile.WriteAtomic(paths.SnapshotFile, snapshot);

    public void Clear() => File.Delete(paths.SnapshotFile);
}
