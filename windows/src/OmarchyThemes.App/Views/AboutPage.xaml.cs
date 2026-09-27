using System.Reflection;
using Microsoft.UI.Xaml.Controls;

namespace OmarchyThemes.App.Views;

public sealed partial class AboutPage : Page
{
    public AboutPage()
    {
        InitializeComponent();
        var version = Assembly.GetExecutingAssembly()
            .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "dev";
        VersionText.Text = $"Version {version.Split('+')[0]}";
    }
}
