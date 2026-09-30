using System.Net;
using OmarchyThemes.Core.Catalog;
using OmarchyThemes.Core.Net;
using OmarchyThemes.Core.Tests.TestSupport;

namespace OmarchyThemes.Core.Tests;

public sealed class HttpCacheTests : IDisposable
{
    private const string Url = "https://example.com/data.json";
    private readonly TempDir _dir = new();
    private readonly FakeHttpHandler _http = new();
    private readonly ManualTimeProvider _time = new(new DateTimeOffset(2026, 9, 26, 12, 0, 0, TimeSpan.Zero));
    private readonly HttpCache _cache;

    public HttpCacheTests() => _cache = new HttpCache(new HttpClient(_http), _dir.Path, _time);

    public void Dispose() => _dir.Dispose();

    [Fact]
    public async Task Serves_from_disk_within_max_age_without_network()
    {
        _http.On(Url, "v1", etag: "\"a\"");
        var options = new CacheOptions { MaxAge = TimeSpan.FromHours(1) };

        var first = await _cache.GetAsync(new Uri(Url), options);
        _time.Advance(TimeSpan.FromMinutes(30));
        var second = await _cache.GetAsync(new Uri(Url), options);

        Assert.False(first.FromCache);
        Assert.True(second.FromCache);
        Assert.Equal("v1", second.Text);
        Assert.Equal(1, _http.CountFor(Url));
    }

    [Fact]
    public async Task Revalidates_with_etag_and_uses_cached_body_on_304()
    {
        _http.On(Url, "v1", etag: "\"a\"");
        await _cache.GetAsync(new Uri(Url));

        _http.On(Url, req =>
        {
            Assert.Equal("\"a\"", req.Headers.IfNoneMatch.Single().Tag);
            return FakeHttpHandler.Status(HttpStatusCode.NotModified);
        });
        _time.Advance(TimeSpan.FromMinutes(5));
        var revalidated = await _cache.GetAsync(new Uri(Url));

        Assert.True(revalidated.FromCache);
        Assert.False(revalidated.IsStale);
        Assert.Equal("v1", revalidated.Text);
        Assert.Equal(_time.Now, revalidated.FetchedAt);
    }

    [Fact]
    public async Task Replaces_cache_when_content_changes()
    {
        _http.On(Url, "v1", etag: "\"a\"");
        await _cache.GetAsync(new Uri(Url));
        _http.On(Url, "v2", etag: "\"b\"");

        var updated = await _cache.GetAsync(new Uri(Url));

        Assert.Equal("v2", updated.Text);
        Assert.Equal("v2", _cache.TryGetCached(new Uri(Url))!.Text);
    }

    [Fact]
    public async Task Returns_stale_copy_when_offline()
    {
        _http.On(Url, "v1");
        await _cache.GetAsync(new Uri(Url));
        _http.On(Url, _ => FakeHttpHandler.Throw());

        var stale = await _cache.GetAsync(new Uri(Url));

        Assert.True(stale.IsStale);
        Assert.Equal("v1", stale.Text);
        Assert.IsType<HttpRequestException>(stale.Error);
    }

    [Fact]
    public async Task Throws_when_offline_with_nothing_cached()
    {
        _http.On(Url, _ => FakeHttpHandler.Throw());
        await Assert.ThrowsAsync<HttpRequestException>(() => _cache.GetAsync(new Uri(Url)));
    }

    [Fact]
    public async Task Maps_server_errors_through_options()
    {
        _http.On(Url, _ => FakeHttpHandler.Status(HttpStatusCode.InternalServerError));
        var options = new CacheOptions { MapError = _ => new TimeoutException("mapped") };

        var e = await Assert.ThrowsAsync<TimeoutException>(() => _cache.GetAsync(new Uri(Url), options));
        Assert.Equal("mapped", e.Message);
    }

    [Fact]
    public async Task Returns_the_response_when_caching_it_fails()
    {
        // A file where the cache folder should be makes every cache write fail.
        var blocked = Path.Combine(_dir.Path, "blocked");
        File.WriteAllText(blocked, "");
        var cache = new HttpCache(new HttpClient(_http), blocked, _time);
        _http.On(Url, "v1", etag: "\"a\"");

        var response = await cache.GetAsync(new Uri(Url));

        Assert.Equal("v1", response.Text);
        Assert.Null(cache.TryGetCached(new Uri(Url)));
    }

    [Fact]
    public async Task Catalog_service_loads_cache_offline_and_rejects_pages_without_themes()
    {
        var page = CatalogParser.DefaultPageUri.AbsoluteUri;
        var catalog = new CatalogService(_cache);
        Assert.Null(catalog.LoadCached());

        _http.On(page, Fixture.Read("catalog-live-structure.html"));
        var fresh = await catalog.RefreshAsync();
        Assert.Equal(4, fresh.Entries.Count);
        Assert.False(fresh.IsStale);
        Assert.Equal(4, catalog.LoadCached()!.Entries.Count);

        _http.On(page, _ => FakeHttpHandler.Throw());
        var offline = await catalog.RefreshAsync();
        Assert.True(offline.IsStale);
        Assert.Equal(4, offline.Entries.Count);

        _http.On(page, "<html><body>Under maintenance</body></html>");
        await Assert.ThrowsAsync<CatalogFormatException>(() => catalog.RefreshAsync());
    }
}
