using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Core.Theming;

/// <summary>Where <see cref="ThemeApplier"/> keeps the desktop it saved before the first Apply.</summary>
public interface ISnapshotStore
{
    DesktopSnapshot? Load();
    void Save(DesktopSnapshot snapshot);
    void Clear();
}

/// <summary>original-desktop.json in the data folder (docs/data-format.md).</summary>
public sealed class FileSnapshotStore(AppPaths paths) : ISnapshotStore
{
    public DesktopSnapshot? Load() => JsonFile.TryRead<DesktopSnapshot>(paths.SnapshotFile);

    public void Save(DesktopSnapshot snapshot) => JsonFile.WriteAtomic(paths.SnapshotFile, snapshot);

    public void Clear() => File.Delete(paths.SnapshotFile);
}
