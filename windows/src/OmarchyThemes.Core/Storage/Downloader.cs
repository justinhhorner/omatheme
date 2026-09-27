namespace OmarchyThemes.Core.Storage;

public interface IDownloader
{
    /// <summary>Downloads <paramref name="url"/> to <paramref name="destinationPath"/>, reporting bytes received.</summary>
    Task DownloadAsync(Uri url, string destinationPath, IProgress<long>? bytesReceived, CancellationToken ct);
}

public sealed class HttpDownloader(HttpClient http) : IDownloader
{
    public async Task DownloadAsync(Uri url, string destinationPath, IProgress<long>? bytesReceived, CancellationToken ct)
    {
        using var response = await http.GetAsync(url, HttpCompletionOption.ResponseHeadersRead, ct).ConfigureAwait(false);
        if (!response.IsSuccessStatusCode)
            throw new HttpRequestException(
                $"Downloading {Path.GetFileName(url.AbsolutePath)} failed: {(int)response.StatusCode} {response.ReasonPhrase}.",
                null, response.StatusCode);

        var partial = destinationPath + ".part";
        try
        {
            await using (var source = await response.Content.ReadAsStreamAsync(ct).ConfigureAwait(false))
            await using (var target = File.Create(partial))
            {
                var buffer = new byte[81920];
                long total = 0;
                int read;
                while ((read = await source.ReadAsync(buffer, ct).ConfigureAwait(false)) > 0)
                {
                    await target.WriteAsync(buffer.AsMemory(0, read), ct).ConfigureAwait(false);
                    total += read;
                    bytesReceived?.Report(total);
                }
            }
            File.Move(partial, destinationPath, overwrite: true);
        }
        finally
        {
            File.Delete(partial);
        }
    }
}
