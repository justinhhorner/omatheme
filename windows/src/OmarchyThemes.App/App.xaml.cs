using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using Microsoft.UI.Xaml;
using OmarchyThemes.App.Services;
using OmarchyThemes.App.ViewModels;
using OmarchyThemes.Core;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Themes;
using OmarchyThemes.Core.Theming;
using OmarchyThemes.Platform.Windows;

namespace OmarchyThemes.App;

public partial class App : Application
{
    public App()
    {
        InitializeComponent();
        Services = ConfigureServices();
    }

    public static new App Current => (App)Application.Current;

    public IServiceProvider Services { get; }

    public MainWindow? MainWindow { get; private set; }

    public static T GetService<T>() where T : notnull => Current.Services.GetRequiredService<T>();

    private static ServiceProvider ConfigureServices()
    {
        var environment = AppEnvironment.FromProcess();
        var paths = environment.Paths;
        var services = new ServiceCollection();

        services.AddSingleton(environment);
        services.AddSingleton(paths);
        services.AddLogging(logging => logging
            .SetMinimumLevel(LogLevel.Information)
            .AddDebug()
            .AddProvider(new FileLoggerProvider(paths.LogsDir)));
        services.AddSingleton(_ => new HttpCache(HttpClients.CreateApi(), paths.CacheDir));
        services.AddSingleton(sp => new CatalogService(sp.GetRequiredService<HttpCache>(), github: sp.GetRequiredService<GitHubClient>()));
        services.AddSingleton(sp => new GitHubClient(sp.GetRequiredService<HttpCache>(), StorageInfo.GitHubToken()));
        services.AddSingleton<ThemeResolver>();
        services.AddSingleton<ThemeDetailsService>();
        services.AddSingleton<ThemeDownloads>();
        services.AddSingleton(_ => new ThemeStore(paths, new HttpDownloader(HttpClients.CreateDownloads())));
        services.AddSingleton(_ => new SettingsStore(paths));

        // Nothing touches the desktop until the user explicitly applies a theme.
        services.AddSingleton<IDesktopBackend>(_ => environment.IsDryRun
            ? new DryRunDesktopBackend()
            : WindowsDesktopBackend.CreateDefault(Path.Combine(paths.Root, "original-desktop")));
        services.AddSingleton<ISnapshotStore>(_ => new FileSnapshotStore(paths));
        // Test switches keep Windows Terminal's real fragments folder untouched too.
        services.AddSingleton(_ => new WindowsTerminalSchemes(environment.IsDryRun || environment.DataDir is not null
            ? Path.Combine(paths.Root, "windows-terminal-fragments")
            : WindowsTerminalSchemes.DefaultFragmentsDir));
        services.AddSingleton<ThemeApplier>();
        services.AddSingleton<ApplyService>();
        services.AddSingleton<NavigationService>();

        services.AddSingleton<GalleryViewModel>();
        services.AddSingleton<DownloadedViewModel>();
        services.AddTransient<ThemeDetailViewModel>();
        services.AddTransient<SettingsViewModel>();

        return services.BuildServiceProvider();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        GetService<AppPaths>().EnsureCreated();
        GetService<ThemeStore>().CleanUpStaging();

        var environment = GetService<AppEnvironment>();
        GetService<ILogger<App>>().LogInformation("Started {Version}; data folder {Root}{DryRun}",
            typeof(App).Assembly.GetName().Version, GetService<AppPaths>().Root, environment.IsDryRun ? "; DRY RUN" : "");

        var showWelcome = !GetService<SettingsStore>().Load().WelcomeSeen;
        MainWindow = new MainWindow(showWelcome);
        MainWindow.Activate();
    }
}
