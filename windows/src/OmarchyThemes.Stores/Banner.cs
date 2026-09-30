using System.Globalization;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Stores;

/// <summary>The outcome of something the user did, for the page's notification bar.</summary>
public sealed record Banner(SummaryKind Kind, string Title, string Message)
{
    public static Banner From(ApplySummary summary) => new(summary.Kind, summary.Title, summary.Message);
}

public static class TimeText
{
    /// <summary>"just now", "5 min ago", "3 h ago", "yesterday", else the date.</summary>
    public static string Ago(DateTimeOffset when, DateTimeOffset now)
    {
        var age = now - when;
        return age.TotalMinutes < 1 ? "just now"
            : age.TotalHours < 1 ? $"{(int)age.TotalMinutes} min ago"
            : age.TotalDays < 1 ? $"{(int)age.TotalHours} h ago"
            : age.TotalDays < 2 ? "yesterday"
            : when.ToLocalTime().ToString("d MMM", CultureInfo.CurrentCulture);
    }
}
