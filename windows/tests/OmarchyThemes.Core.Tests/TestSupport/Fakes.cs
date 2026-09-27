using System.Net;
using System.Text;
using OmarchyThemes.Core.Colors;
using OmarchyThemes.Core.Palettes;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Theming;

namespace OmarchyThemes.Core.Tests.TestSupport;

/// <summary>Routes requests by exact URL (or a fallback) and records what was sent.</summary>
internal sealed class FakeHttpHandler : HttpMessageHandler
{
    private readonly Dictionary<string, Func<HttpRequestMessage, HttpResponseMessage>> _routes = new();

    public List<HttpRequestMessage> Requests { get; } = [];

    public Func<HttpRequestMessage, HttpResponseMessage>? Fallback { get; set; }

    public FakeHttpHandler On(string url, Func<HttpRequestMessage, HttpResponseMessage> respond)
    {
        _routes[url] = respond;
        return this;
    }

    public FakeHttpHandler On(string url, string body, string? etag = null) =>
        On(url, _ => Text(body, etag));

    public int CountFor(string url) => Requests.Count(r => r.RequestUri!.AbsoluteUri == url);

    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
    {
        Requests.Add(request);
        var url = request.RequestUri!.AbsoluteUri;
        if (_routes.TryGetValue(url, out var respond))
            return Task.FromResult(respond(request));
        if (Fallback is not null)
            return Task.FromResult(Fallback(request));
        return Task.FromResult(new HttpResponseMessage(HttpStatusCode.NotFound));
    }

    public static HttpResponseMessage Text(string body, string? etag = null, HttpStatusCode status = HttpStatusCode.OK)
    {
        var response = new HttpResponseMessage(status) { Content = new StringContent(body, Encoding.UTF8) };
        if (etag is not null)
            response.Headers.ETag = new System.Net.Http.Headers.EntityTagHeaderValue(etag);
        return response;
    }

    public static HttpResponseMessage Status(HttpStatusCode status, params (string Name, string Value)[] headers)
    {
        var response = new HttpResponseMessage(status) { Content = new StringContent("") };
        foreach (var (name, value) in headers)
            response.Headers.TryAddWithoutValidation(name, value);
        return response;
    }

    public static HttpResponseMessage Throw() => throw new HttpRequestException("No network");
}

internal sealed class ManualTimeProvider(DateTimeOffset start) : TimeProvider
{
    public DateTimeOffset Now { get; set; } = start;

    public override DateTimeOffset GetUtcNow() => Now;

    public void Advance(TimeSpan by) => Now += by;
}

internal sealed class TempDir : IDisposable
{
    public TempDir() => Directory.CreateDirectory(Path);

    public string Path { get; } = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "omatheme-tests", Guid.NewGuid().ToString("N"));

    public AppPaths AppPaths => new(Path);

    public void Dispose()
    {
        try { Directory.Delete(Path, recursive: true); }
        catch (IOException) { }
    }
}

/// <summary>Writes the URL into the file instead of downloading; can be told to fail for a URL.</summary>
internal sealed class FakeDownloader : IDownloader
{
    public HashSet<string> FailUrls { get; } = [];
    public List<Uri> Downloaded { get; } = [];

    public Task DownloadAsync(Uri url, string destinationPath, IProgress<long>? bytesReceived, CancellationToken ct)
    {
        ct.ThrowIfCancellationRequested();
        if (FailUrls.Contains(url.AbsoluteUri))
            throw new HttpRequestException($"Download failed for {url}");
        var bytes = Encoding.UTF8.GetBytes(url.AbsoluteUri);
        File.WriteAllBytes(destinationPath, bytes);
        bytesReceived?.Report(bytes.Length);
        Downloaded.Add(url);
        return Task.CompletedTask;
    }
}

/// <summary>Records every OS call instead of touching the real desktop.</summary>
internal sealed class FakeDesktopBackend : IDesktopBackend
{
    public DesktopCapabilities Capabilities { get; set; } =
        DesktopCapabilities.Wallpaper | DesktopCapabilities.AppearanceMode | DesktopCapabilities.AccentColor;

    public List<string> Calls { get; } = [];
    public HashSet<string> FailOn { get; } = [];

    public DesktopSnapshot SnapshotToReturn { get; set; } =
        new(DateTimeOffset.UnixEpoch, new Dictionary<string, string> { ["wallpaper"] = @"C:\original.jpg" });

    public DesktopSnapshot? Restored { get; private set; }

    private Task Record(string call)
    {
        Calls.Add(call);
        var name = call.Split(':')[0];
        if (FailOn.Contains(name))
            throw new InvalidOperationException($"{name} exploded");
        return Task.CompletedTask;
    }

    public async Task<DesktopSnapshot> CaptureAsync(CancellationToken ct = default)
    {
        await Record("capture");
        return SnapshotToReturn;
    }

    public Task RestoreAsync(DesktopSnapshot snapshot, CancellationToken ct = default)
    {
        Restored = snapshot;
        return Record("restore");
    }

    public Task SetWallpaperAsync(string imagePath, WallpaperFit fit, CancellationToken ct = default) =>
        Record($"wallpaper:{System.IO.Path.GetFileName(imagePath)}:{fit}");

    public Task SetAppearanceModeAsync(AppearanceMode mode, CancellationToken ct = default) =>
        Record($"mode:{mode}");

    public Task SetAccentColorAsync(RgbColor accent, CancellationToken ct = default) =>
        Record($"accent:{accent}");
}

internal sealed class InMemorySnapshotStore : ISnapshotStore
{
    public DesktopSnapshot? Snapshot { get; set; }
    public DesktopSnapshot? Load() => Snapshot;
    public void Save(DesktopSnapshot snapshot) => Snapshot = snapshot;
    public void Clear() => Snapshot = null;
}

internal static class Fixture
{
    public static string Read(string name) =>
        File.ReadAllText(Path.Combine(AppContext.BaseDirectory, "Fixtures", name));
}
