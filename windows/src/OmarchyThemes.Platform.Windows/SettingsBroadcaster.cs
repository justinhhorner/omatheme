using Windows.Win32;
using Windows.Win32.Foundation;
using Windows.Win32.UI.WindowsAndMessaging;

namespace OmarchyThemes.Platform.Windows;

public interface ISettingsBroadcaster
{
    /// <summary>Tells running apps and the shell that a settings area changed (WM_SETTINGCHANGE).</summary>
    void Broadcast(string area);
}

public sealed class Win32SettingsBroadcaster : ISettingsBroadcaster
{
    /// <summary>The area Explorer and apps watch for light/dark and accent changes.</summary>
    public const string ImmersiveColorSet = "ImmersiveColorSet";

    public unsafe void Broadcast(string area)
    {
        fixed (char* p = area)
        {
            // SMTO_ABORTIFHUNG: a hung window must not freeze the app.
            PInvoke.SendMessageTimeout(
                HWND.HWND_BROADCAST, PInvoke.WM_SETTINGCHANGE, default, (LPARAM)(nint)p,
                SEND_MESSAGE_TIMEOUT_FLAGS.SMTO_ABORTIFHUNG, 1000, out _);
        }
    }
}
