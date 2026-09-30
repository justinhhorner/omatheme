using System.Text;
using Microsoft.Extensions.Logging;

namespace OmarchyThemes.App.Services;

/// <summary>
/// Writes Information-and-above log lines to a daily file in the data folder
/// (<c>logs\omarchy-themes-yyyyMMdd.log</c>), keeping the last 7 days. Kept deliberately small: the
/// point is to be able to see why "Couldn't load themes" happened after the fact.
/// </summary>
public sealed class FileLoggerProvider : ILoggerProvider
{
    private const int KeepDays = 7;
    private readonly string _directory;
    private readonly object _gate = new();

    public FileLoggerProvider(string directory)
    {
        _directory = directory;
        TryDeleteOldFiles();
    }

    public ILogger CreateLogger(string categoryName) => new FileLogger(this, ShortCategory(categoryName));

    public void Dispose()
    {
    }

    private void Write(string category, LogLevel level, string message, Exception? exception)
    {
        var line = new StringBuilder()
            .Append(DateTimeOffset.Now.ToString("yyyy-MM-dd HH:mm:ss.fff zzz"))
            .Append(" [").Append(level).Append("] ")
            .Append(category).Append(": ").Append(message);
        // Errors get the full stack trace, so a crash can be diagnosed from the log alone.
        if (exception is not null && level >= LogLevel.Error)
            line.AppendLine().Append(exception);
        else if (exception is not null)
            line.AppendLine().Append("    ").Append(exception.GetType().FullName).Append(": ").Append(exception.Message);
        line.AppendLine();

        lock (_gate)
        {
            try
            {
                Directory.CreateDirectory(_directory);
                File.AppendAllText(Path.Combine(_directory, $"omarchy-themes-{DateTime.Now:yyyyMMdd}.log"), line.ToString());
            }
            catch (IOException)
            {
                // Logging must never take the app down.
            }
            catch (UnauthorizedAccessException)
            {
            }
        }
    }

    private void TryDeleteOldFiles()
    {
        try
        {
            if (!Directory.Exists(_directory))
                return;
            foreach (var file in new DirectoryInfo(_directory).EnumerateFiles("omarchy-themes-*.log")
                         .Where(f => f.LastWriteTime < DateTime.Now.AddDays(-KeepDays)))
                file.Delete();
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    private static string ShortCategory(string category) =>
        category.StartsWith("OmarchyThemes.", StringComparison.Ordinal) ? category[(category.LastIndexOf('.') + 1)..] : category;

    private sealed class FileLogger(FileLoggerProvider provider, string category) : ILogger
    {
        public IDisposable? BeginScope<TState>(TState state) where TState : notnull => null;

        public bool IsEnabled(LogLevel logLevel) => logLevel >= LogLevel.Information && logLevel != LogLevel.None;

        public void Log<TState>(LogLevel logLevel, EventId eventId, TState state, Exception? exception, Func<TState, Exception?, string> formatter)
        {
            if (IsEnabled(logLevel))
                provider.Write(category, logLevel, formatter(state, exception), exception);
        }
    }
}
