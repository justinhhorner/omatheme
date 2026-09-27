using Microsoft.Extensions.DependencyInjection;
using Microsoft.UI.Xaml;
using OmarchyThemes.Core;

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

    private static ServiceProvider ConfigureServices()
    {
        var services = new ServiceCollection();
        services.AddSingleton(AppPaths.Default());
        return services.BuildServiceProvider();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Services.GetRequiredService<AppPaths>().EnsureCreated();

        MainWindow = new MainWindow();
        MainWindow.Activate();
    }
}
