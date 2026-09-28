using System.Globalization;
using System.Net;
using System.Net.Http.Headers;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json.Serialization;
using OmarchyThemes.Core.Storage;

namespace OmarchyThemes.Core.Net;

/// <summary>
/// Disk-backed HTTP GET cache. Responses are stored with their ETag / Last-Modified and
/// revalidated with conditional requests (a GitHub 304 doesn't count against the rate limit).
/// When the network or server fails and a cached copy exists, the stale copy is returned with
/// the error attached, which keeps the app usable offline.
/// </summary>
public sealed class HttpCache
{
    private readonly HttpClient _http;
    private readonly string _directory;
    private readonly TimeProvider _time;

    public HttpCache(HttpClient http, string directory, TimeProvider? time = null)
    {
        _http = http;
        _directory = directory;
        _time = time ?? TimeProvider.System;
    }

    public async Task<CachedResponse> GetAsync(Uri uri, CacheOptions? options = null, CancellationToken ct = default)
    {
        options ??= CacheOptions.Default;
        var cached = TryGetCached(uri);

        if (cached is not null && !options.ForceRevalidate && _time.GetUtcNow() - cached.FetchedAt < options.MaxAge)
            return cached;

        using var request = new HttpRequestMessage(HttpMethod.Get, uri);
        options.ConfigureRequest?.Invoke(request);
        if (cached?.ETag is { } etag && EntityTagHeaderValue.TryParse(etag, out var tag))
            request.Headers.IfNoneMatch.Add(tag);
        else if (cached?.LastModified is { } lastModified)
            request.Headers.IfModifiedSince = lastModified;

        HttpResponseMessage response;
        try
        {
            response = await _http.SendAsync(request, HttpCompletionOption.ResponseContentRead, ct).ConfigureAwait(false);
        }
        catch (Exception e) when (e is HttpRequestException || (e is TaskCanceledException && !ct.IsCancellationRequested))
        {
            // Network failure or timeout (not user cancellation).
            if (cached is not null)
                return cached with { IsStale = true, Error = e };
            throw;
        }

        using (response)
        {
            if (response.StatusCode == HttpStatusCode.NotModified && cached is not null)
            {
                var refreshed = cached with { FetchedAt = _time.GetUtcNow(), FromCache = true };
                WriteMeta(uri, refreshed);
                return refreshed;
            }

            if (!response.IsSuccessStatusCode)
            {
                var error = options.MapError?.Invoke(response)
                    ?? new HttpRequestException($"{uri.Host} returned {(int)response.StatusCode} {response.ReasonPhrase}.", null, response.StatusCode);
                if (cached is not null && options.AllowStaleOnError)
                    return cached with { IsStale = true, Error = error };
                throw error;
            }

            var body = await response.Content.ReadAsByteArrayAsync(ct).ConfigureAwait(false);
            var fresh = new CachedResponse(
                body,
                _time.GetUtcNow(),
                FromCache: false,
                IsStale: false,
                ETag: response.Headers.ETag?.ToString(),
                LastModified: response.Content.Headers.LastModified);
            Store(uri, fresh);
            return fresh;
        }
    }

    /// <summary>Returns the cached copy without touching the network, or null.</summary>
    public CachedResponse? TryGetCached(Uri uri)
    {
        var (bodyPath, metaPath) = PathsFor(uri);
        var meta = JsonFile.TryRead<CacheMeta>(metaPath);
        if (meta is null || meta.Url != uri.AbsoluteUri || !File.Exists(bodyPath))
            return null;
        try
        {
            return new CachedResponse(File.ReadAllBytes(bodyPath), meta.FetchedAt, FromCache: true, IsStale: false, meta.ETag, ParseHttpDate(meta.LastModified));
        }
        catch (IOException)
        {
            return null;
        }
    }

    private void Store(Uri uri, CachedResponse response)
    {
        var (bodyPath, _) = PathsFor(uri);
        JsonFile.WriteBytesAtomic(bodyPath, response.Body);
        WriteMeta(uri, response);
    }

    private void WriteMeta(Uri uri, CachedResponse response)
    {
        var (_, metaPath) = PathsFor(uri);
        JsonFile.WriteAtomic(metaPath, new CacheMeta(
            uri.AbsoluteUri, response.FetchedAt, response.ETag, response.LastModified?.ToString("R", CultureInfo.InvariantCulture)));
    }

    private (string Body, string Meta) PathsFor(Uri uri)
    {
        var key = Convert.ToHexStringLower(SHA256.HashData(Encoding.UTF8.GetBytes(uri.AbsoluteUri)));
        var baseName = Path.Combine(_directory, key[..2], key);
        return (baseName + ".body", baseName + ".json");
    }

    /// <summary>
    /// Cache metadata as in docs/data-format.md: the ETag and Last-Modified headers verbatim. Older files
    /// (key "eTag", an ISO date) still read: key matching is case-insensitive and both date forms parse.
    /// </summary>
    private sealed record CacheMeta(
        string Url,
        DateTimeOffset FetchedAt,
        [property: JsonPropertyName("etag")] string? ETag,
        string? LastModified);

    private static DateTimeOffset? ParseHttpDate(string? text) =>
        DateTimeOffset.TryParse(text, CultureInfo.InvariantCulture, DateTimeStyles.AssumeUniversal, out var date) ? date : null;
}

public sealed record CacheOptions
{
    public static readonly CacheOptions Default = new();

    /// <summary>How long a cached copy is used without any network request. Zero always revalidates.</summary>
    public TimeSpan MaxAge { get; init; } = TimeSpan.Zero;

    /// <summary>Revalidate even if the cached copy is younger than <see cref="MaxAge"/>.</summary>
    public bool ForceRevalidate { get; init; }

    /// <summary>Return the cached copy (marked stale) when the server responds with an error.</summary>
    public bool AllowStaleOnError { get; init; } = true;

    public Action<HttpRequestMessage>? ConfigureRequest { get; init; }

    /// <summary>Turns a non-success response into a domain exception (e.g. GitHub rate limiting).</summary>
    public Func<HttpResponseMessage, Exception?>? MapError { get; init; }
}

public sealed record CachedResponse(
    byte[] Body,
    DateTimeOffset FetchedAt,
    bool FromCache,
    bool IsStale,
    string? ETag = null,
    DateTimeOffset? LastModified = null)
{
    /// <summary>Why a stale copy was served instead of a fresh one.</summary>
    public Exception? Error { get; init; }

    public string Text => Encoding.UTF8.GetString(Body).TrimStart('﻿');
}
