using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Stores;

/// <summary>
/// The app's settings (settings.json), read once and kept in memory, so bindings and badges don't
/// read the file each time.
/// </summary>
public sealed class Preferences
{
    private readonly SettingsStore _store;
    private readonly ILogger _log;

    public Preferences(SettingsStore store, ILogger<Preferences>? log = null)
    {
        _store = store;
        _log = log ?? NullLogger<Preferences>.Instance;
        Settings = store.Load();
    }

    /// <summary>Raised after the settings change.</summary>
    public event EventHandler? Changed;

    public AppSettings Settings { get; private set; }

    /// <summary>Changes the settings and saves them, if anything changed.</summary>
    public void Update(Func<AppSettings, AppSettings> change)
    {
        var updated = change(Settings);
        if (updated == Settings)
            return;
        Settings = updated;
        try
        {
            _store.Save(updated);
        }
        catch (Exception e) when (ExpectedErrors.IsExpected(e))
        {
            // The change still applies for this session; it just won't be there next launch.
            _log.LogError(e, "Saving settings failed");
        }
        Changed?.Invoke(this, EventArgs.Empty);
    }

    public void DismissWelcome() => Update(s => s with { WelcomeSeen = true });
}
