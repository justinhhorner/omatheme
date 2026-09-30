namespace OmarchyThemes.App.Services;

/// <summary>Snapshot of local storage use, for Settings.</summary>
public static class StorageInfo
{
    /// <summary>Total size of the files under <paramref name="path"/>; files that vanish or can't be read are skipped.</summary>
    public static long DirectorySize(string path)
    {
        if (!Directory.Exists(path))
            return 0;
        try
        {
            var options = new EnumerationOptions { RecurseSubdirectories = true, IgnoreInaccessible = true };
            return new DirectoryInfo(path).EnumerateFiles("*", options).Sum(f => f.Length);
        }
        catch (IOException)
        {
            return 0; // e.g. a folder deleted while counting; the next refresh shows the right size
        }
    }
}
