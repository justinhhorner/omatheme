using System.Net;
using OmarchyThemes.Core;

namespace OmarchyThemes.App.Services;

public static class HttpClients
{
    /// <summary>For pages and API calls: short timeout.</summary>
    public static HttpClient CreateApi() => Create(TimeSpan.FromSeconds(30));

    /// <summary>For wallpaper downloads (can be tens of MB): no overall timeout, cancelled by the user instead.</summary>
    public static HttpClient CreateDownloads() => Create(Timeout.InfiniteTimeSpan);

    private static HttpClient Create(TimeSpan timeout)
    {
        var client = new HttpClient(new SocketsHttpHandler
        {
            AutomaticDecompression = DecompressionMethods.All,
            PooledConnectionLifetime = TimeSpan.FromMinutes(5),
            ConnectTimeout = TimeSpan.FromSeconds(15),
        })
        {
            Timeout = timeout,
        };
        client.DefaultRequestHeaders.UserAgent.ParseAdd(AppInfo.UserAgent);
        return client;
    }
}
