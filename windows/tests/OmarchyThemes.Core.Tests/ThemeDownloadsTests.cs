using System.Net;
using System.Text;
using System.Text.Json;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.GitHub;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Storage;
using OmarchyThemes.Core.Tests.TestSupport;
using OmarchyThemes.Core.Themes;

namespace OmarchyThemes.Core.Tests;

/// <summary>Downloads that outlive the page, and lookups shared between callers.</summary>
public sealed class ThemeDownloadsTests : IDisposable
{
    private const string TreeUrl = "https://api.github.com/repos/o/r/git/trees/HEAD?recursive=1";
    private static readonly CatalogEntry Entry = new("r", "My Theme", "https://github.com/o/r", ScreenshotUrl: null);

    private readonly TempDir _dir = new();
    private readonly GatedHttpHandler _http = new();
    private readonly GatedDownloader _downloader = new();
    private readonly ThemeDetailsService _details;
    private readonly ThemeDownloads _downloads;

    public ThemeDownloadsTests()
    {
        _http.Responses[TreeUrl] = JsonSerializer.Serialize(new
        {
            truncated = false,
            tree = new[]
            {
                new { path = "backgrounds/1.png", type = "blob", size = (long?)100 },
                new { path = "backgrounds/2.png", type = "blob", size = (long?)300 },
            },
        });
        var cache = new HttpCache(new HttpClient(_http), Path.Combine(_dir.Path, "cache"));
        _details = new ThemeDetailsService(new ThemeResolver(new GitHubClient(cache)));
        _downloads = new ThemeDownloads(_details, new ThemeStore(_dir.AppPaths, _downloader));
    }

    public void Dispose()
    {
        _http.Release();
        _downloader.Release();
        _dir.Dispose();
    }

    [Fact]
    public async Task Starting_again_joins_the_running_download()
    {
        _http.Release();
        var first = _downloads.Start(Entry);
        var second = _downloads.Start(Entry);

        Assert.Same(first, second);
        Assert.Same(first, _downloads.Get(Entry.Slug));

        _downloader.Release();
        var outcome = await first.Completion;

        Assert.True(outcome.Succeeded);
        Assert.Equal(["1.png", "2.png"], outcome.Theme!.Wallpapers);
        Assert.Equal(2, _downloader.Calls); // each wallpaper downloaded once, not twice
        Assert.Null(_downloads.Get(Entry.Slug));
    }

    [Fact]
    public async Task Progress_is_reported_in_bytes_across_files()
    {
        _http.Release();
        _downloader.OneFileAtATime = true;
        var download = _downloads.Start(Entry);
        var changed = 0;
        download.PropertyChanged += (_, _) => Interlocked.Increment(ref changed);

        // The first file (100 bytes) is half done.
        await _downloader.WaitUntilStarted(0);
        Assert.Equal(new DownloadStatus(DownloadPhase.Downloading, 0, 2, 50, 400), download.Status);
        Assert.Equal(12.5, download.Status.Percent);

        // The second file's progress counts the first file's full size.
        _downloader.ReleaseFile(0);
        await _downloader.WaitUntilStarted(1);
        Assert.Equal(new DownloadStatus(DownloadPhase.Downloading, 1, 2, 100 + 150, 400), download.Status);

        _downloader.ReleaseFile(1);
        await download.Completion;
        Assert.Equal(new DownloadStatus(DownloadPhase.Finishing, 2, 2, 400, 400), download.Status);
        Assert.Equal(100, download.Status.Percent);
        Assert.True(changed > 0, "PropertyChanged was never raised");
    }

    [Fact]
    public async Task Cancelling_leaves_nothing_behind()
    {
        _http.Release();
        var download = _downloads.Start(Entry);
        await _downloader.WaitUntilStarted(0);

        download.Cancel();
        var outcome = await download.Completion;

        Assert.True(outcome.Cancelled);
        Assert.False(outcome.Succeeded);
        Assert.Null(_downloads.Get(Entry.Slug));
        Assert.Empty(Directory.EnumerateFileSystemEntries(_dir.AppPaths.ThemesDir));
    }

    [Fact]
    public async Task Failures_are_reported_not_thrown()
    {
        _http.Responses.Remove(TreeUrl); // 404
        _http.Release();

        var outcome = await _downloads.Start(Entry).Completion;

        Assert.False(outcome.Succeeded);
        Assert.IsType<GitHubNotFoundException>(outcome.Error);
        Assert.Null(_downloads.Get(Entry.Slug));
    }

    [Fact]
    public async Task Concurrent_lookups_of_a_theme_share_one_request()
    {
        var first = _details.ResolveAsync(Entry);
        var second = _details.ResolveAsync(Entry);
        _http.Release();

        Assert.Same(await first, await second);
        Assert.Equal(1, _http.CountFor(TreeUrl));

        // Later lookups come from the session cache.
        await _details.ResolveAsync(Entry);
        Assert.Equal(1, _http.CountFor(TreeUrl));
    }

    [Fact]
    public async Task One_caller_giving_up_does_not_cancel_a_shared_lookup()
    {
        using var cts = new CancellationTokenSource();
        var impatient = _details.ResolveAsync(Entry, ct: cts.Token);
        var patient = _details.ResolveAsync(Entry);

        cts.Cancel();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => impatient);

        _http.Release();
        Assert.Equal(2, (await patient).Wallpapers.Count);
    }

    /// <summary>Answers from <see cref="Responses"/> (else 404), but only after <see cref="Release"/>.</summary>
    private sealed class GatedHttpHandler : HttpMessageHandler
    {
        private readonly TaskCompletionSource _gate = new(TaskCreationOptions.RunContinuationsAsynchronously);
        private readonly List<string> _requests = [];

        public Dictionary<string, string> Responses { get; } = new();

        public void Release() => _gate.TrySetResult();

        public int CountFor(string url)
        {
            lock (_requests)
                return _requests.Count(r => r == url);
        }

        protected override async Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken ct)
        {
            var url = request.RequestUri!.AbsoluteUri;
            lock (_requests)
                _requests.Add(url);
            await _gate.Task.WaitAsync(ct);
            return Responses.TryGetValue(url, out var body)
                ? new HttpResponseMessage(HttpStatusCode.OK) { Content = new StringContent(body, Encoding.UTF8) }
                : new HttpResponseMessage(HttpStatusCode.NotFound) { Content = new StringContent("") };
        }
    }

    /// <summary>
    /// Reports half of each file's size, then waits before finishing it: for <see cref="Release"/>, or
    /// with <see cref="OneFileAtATime"/> for <see cref="ReleaseFile"/> of that file.
    /// </summary>
    private sealed class GatedDownloader : IDownloader
    {
        private static TaskCompletionSource NewGate() => new(TaskCreationOptions.RunContinuationsAsynchronously);

        private readonly TaskCompletionSource _all = NewGate();
        private readonly TaskCompletionSource[] _started = [NewGate(), NewGate()];
        private readonly TaskCompletionSource[] _files = [NewGate(), NewGate()];
        private readonly Dictionary<string, long> _sizes = new() { ["1.png"] = 100, ["2.png"] = 300 };
        private int _calls;

        public bool OneFileAtATime { get; set; }

        public int Calls => _calls;

        public void Release() => _all.TrySetResult();

        public void ReleaseFile(int index) => _files[index].TrySetResult();

        public Task WaitUntilStarted(int index) => _started[index].Task.WaitAsync(TimeSpan.FromSeconds(10));

        public async Task DownloadAsync(Uri url, string destinationPath, IProgress<long>? bytesReceived, CancellationToken ct)
        {
            var index = Math.Min(Interlocked.Increment(ref _calls) - 1, _files.Length - 1);
            var size = _sizes.GetValueOrDefault(Path.GetFileName(url.AbsolutePath), 10);
            bytesReceived?.Report(size / 2);
            _started[index].TrySetResult();
            await (OneFileAtATime ? _files[index].Task : _all.Task).WaitAsync(ct);
            bytesReceived?.Report(size);
            await File.WriteAllBytesAsync(destinationPath, new byte[size], ct);
        }
    }
}
