using System;
using System.Collections.Generic;
using System.Net.Http;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;

namespace Nexora.SDK;

/// <summary>A dependency-free client for the NEXORA v1 REST API.</summary>
public sealed class NexoraClient : IDisposable
{
    private const string DefaultBaseUrl = "http://127.0.0.1:8080";

    private readonly HttpClient httpClient;
    private readonly bool ownsHttpClient;
    private readonly Uri baseUri;
    private readonly string? apiKey;

    public NexoraClient(string? baseUrl = null, string? apiKey = null, HttpClient? httpClient = null)
    {
        this.apiKey = apiKey;
        ownsHttpClient = httpClient == null;
        this.httpClient = httpClient ?? new HttpClient();
        baseUri = ValidateBaseUri(baseUrl ?? DefaultBaseUrl);
    }

    /// <summary>Returns engine and API information.</summary>
    public Task<JsonElement> Info() => RequestAsync(HttpMethod.Get, "/api/v1/info", null);

    /// <summary>Returns available source identifiers.</summary>
    public Task<JsonElement> Sources() => RequestAsync(HttpMethod.Get, "/api/v1/sources", null);

    /// <summary>Searches for tracks across the requested sources.</summary>
    public Task<JsonElement> Search(
        string query,
        IEnumerable<string>? sources = null,
        int limit = 10) =>
        RequestAsync(HttpMethod.Post, "/api/v1/search", new Dictionary<string, object?>
        {
            ["query"] = query,
            ["sources"] = sources,
            ["limit"] = limit,
        });

    /// <summary>Creates a playback session. A null output uses Opus at 48 kHz with two channels.</summary>
    public Task<JsonElement> CreateSession(IDictionary<string, object?>? output = null) =>
        RequestAsync(HttpMethod.Post, "/api/v1/sessions", new Dictionary<string, object?>
        {
            ["application"] = "sdk",
            ["version"] = "1.0.0",
            ["output"] = output ?? new Dictionary<string, object?>
            {
                ["sample_rate"] = 48000,
                ["channels"] = 2,
                ["codec"] = "opus",
            },
        });

    /// <summary>Returns the current state of a session.</summary>
    public Task<JsonElement> GetSession(string id) =>
        RequestAsync(HttpMethod.Get, SessionPath(id), null);

    /// <summary>Starts playback of a track.</summary>
    public Task<JsonElement> Play(string id, string trackId, long startMs = 0) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/play", new Dictionary<string, object?>
        {
            ["track_id"] = trackId,
            ["start_position_ms"] = startMs,
        });

    /// <summary>Pauses a session.</summary>
    public Task<JsonElement> Pause(string id) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/pause", null);

    /// <summary>Resumes a paused session.</summary>
    public Task<JsonElement> Resume(string id) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/resume", null);

    /// <summary>Stops a session.</summary>
    public Task<JsonElement> Stop(string id) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/stop", null);

    /// <summary>Seeks to a position in the current track.</summary>
    public Task<JsonElement> Seek(string id, long positionMs) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/seek", new Dictionary<string, object?>
        {
            ["position_ms"] = positionMs,
        });

    /// <summary>Sets a session's output volume.</summary>
    public Task<JsonElement> SetVolume(string id, int volume) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/volume", new Dictionary<string, object?>
        {
            ["volume"] = volume,
        });

    /// <summary>Returns a session's queue.</summary>
    public Task<JsonElement> Queue(string id) =>
        RequestAsync(HttpMethod.Get, SessionPath(id) + "/queue", null);

    /// <summary>Adds a track to a session's queue.</summary>
    public Task<JsonElement> Enqueue(string id, string trackId) =>
        RequestAsync(HttpMethod.Post, SessionPath(id) + "/queue", new Dictionary<string, object?>
        {
            ["track_id"] = trackId,
        });

    /// <summary>Returns the session event WebSocket URL for an external WebSocket client.</summary>
    public string WebsocketUrl(string id)
    {
        var builder = new UriBuilder(baseUri)
        {
            Scheme = string.Equals(baseUri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase)
                ? "wss"
                : "ws",
            Query = string.Empty,
            Fragment = string.Empty,
        };
        var prefix = builder.Path.TrimEnd('/');
        builder.Path = $"{prefix}/ws/v1/sessions/{Uri.EscapeDataString(id)}";
        return builder.Uri.AbsoluteUri.TrimEnd('/');
    }

    private async Task<JsonElement> RequestAsync(
        HttpMethod method,
        string path,
        object? payload)
    {
        using var request = new HttpRequestMessage(method, new Uri(baseUri.AbsoluteUri + path.TrimStart('/')));
        request.Headers.Accept.Add(new System.Net.Http.Headers.MediaTypeWithQualityHeaderValue("application/json"));
        if (!string.IsNullOrEmpty(apiKey))
        {
            request.Headers.Authorization = new System.Net.Http.Headers.AuthenticationHeaderValue("Bearer", apiKey);
        }
        if (payload != null)
        {
            var json = JsonSerializer.Serialize(payload, payload.GetType());
            request.Content = new StringContent(json, Encoding.UTF8, "application/json");
        }

        try
        {
            using var response = await httpClient.SendAsync(request).ConfigureAwait(false);
            var body = await response.Content.ReadAsStringAsync().ConfigureAwait(false);
            if (!response.IsSuccessStatusCode)
            {
                throw ParseError((int)response.StatusCode, body);
            }
            if (string.IsNullOrWhiteSpace(body))
            {
                using var empty = JsonDocument.Parse("{}");
                return empty.RootElement.Clone();
            }

            try
            {
                using var document = JsonDocument.Parse(body);
                return document.RootElement.Clone();
            }
            catch (JsonException error)
            {
                throw new NexoraException(
                    (int)response.StatusCode,
                    "INVALID_RESPONSE",
                    error.Message,
                    body);
            }
        }
        catch (HttpRequestException error)
        {
            throw new NexoraException(0, "TRANSPORT_ERROR", error.Message, null);
        }
        catch (TaskCanceledException error)
        {
            throw new NexoraException(0, "TRANSPORT_ERROR", error.Message, null);
        }
    }

    private static NexoraException ParseError(int status, string body)
    {
        string? code = null;
        string? message = null;
        if (!string.IsNullOrWhiteSpace(body))
        {
            try
            {
                using var document = JsonDocument.Parse(body);
                if (document.RootElement.TryGetProperty("error", out var error) &&
                    error.ValueKind == JsonValueKind.Object)
                {
                    if (error.TryGetProperty("code", out var codeElement) &&
                        codeElement.ValueKind == JsonValueKind.String)
                    {
                        code = codeElement.GetString();
                    }
                    if (error.TryGetProperty("message", out var messageElement) &&
                        messageElement.ValueKind == JsonValueKind.String)
                    {
                        message = messageElement.GetString();
                    }
                }
            }
            catch (JsonException)
            {
                // Fall back to a generic HTTP error below.
            }
        }

        return new NexoraException(
            status,
            code ?? "HTTP_ERROR",
            message ?? $"NEXORA request failed with HTTP {status}",
            body);
    }

    private static string SessionPath(string id) =>
        "/api/v1/sessions/" + Uri.EscapeDataString(id);

    private static Uri ValidateBaseUri(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            throw new ArgumentException("baseUrl must not be blank", nameof(value));
        }

        var normalized = value.Trim().TrimEnd('/');
        if (!Uri.TryCreate(normalized, UriKind.Absolute, out var uri) ||
            string.IsNullOrEmpty(uri.Host) ||
            (!string.Equals(uri.Scheme, Uri.UriSchemeHttp, StringComparison.OrdinalIgnoreCase) &&
             !string.Equals(uri.Scheme, Uri.UriSchemeHttps, StringComparison.OrdinalIgnoreCase)) ||
            !string.IsNullOrEmpty(uri.Query) ||
            !string.IsNullOrEmpty(uri.Fragment))
        {
            throw new ArgumentException(
                "baseUrl must be an absolute HTTP URL without a query or fragment",
                nameof(value));
        }
        return uri;
    }

    public void Dispose()
    {
        if (ownsHttpClient)
        {
            httpClient.Dispose();
        }
    }
}

/// <summary>An HTTP or transport failure returned by NEXORA.</summary>
public sealed class NexoraException : Exception
{
    public NexoraException(int status, string code, string message, string? body)
        : base(message)
    {
        Status = status;
        Code = code;
        Body = body;
    }

    /// <summary>The HTTP status code, or zero for a transport failure.</summary>
    public int Status { get; }

    /// <summary>The NEXORA error code.</summary>
    public string Code { get; }

    /// <summary>The raw error response body when one was received.</summary>
    public string? Body { get; }
}
