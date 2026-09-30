namespace OmarchyThemes.Core;

public static class AppInfo
{
    /// <summary>"0.1.0", from the version in Directory.Build.props.</summary>
    public static string Version { get; } = typeof(AppInfo).Assembly.GetName().Version?.ToString(3) ?? "0.0.0";

    /// <summary>Sent with every request (GitHub rejects API requests without one).</summary>
    public static string UserAgent { get; } = $"OmarchyThemes/{Version} (+https://github.com/basecamp/omarchy)";
}
