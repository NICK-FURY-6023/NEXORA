package dev.nexora;

import java.io.IOException;
import java.lang.reflect.Array;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * A dependency-free, synchronous client for the NEXORA v1 REST API.
 *
 * <p>Successful responses are returned as raw JSON strings so this client does not require a
 * third-party JSON library.</p>
 */
public final class NexoraClient {
    private static final String DEFAULT_BASE_URL = "http://127.0.0.1:8080";
    private static final Map<String, Object> DEFAULT_OUTPUT = Map.of(
            "sample_rate", 48000,
            "channels", 2,
            "codec", "opus");

    private final HttpClient httpClient;
    private final URI baseUri;
    private final String apiKey;

    /** Creates a client without authentication. */
    public NexoraClient(String baseUrl) {
        this(baseUrl, null);
    }

    /** Creates a client, optionally using a bearer API key. */
    public NexoraClient(String baseUrl, String apiKey) {
        this.baseUri = validateBaseUri(baseUrl);
        this.apiKey = apiKey;
        this.httpClient = HttpClient.newHttpClient();
    }

    /** Returns engine and API information as raw JSON. */
    public String info() {
        return request("GET", "/api/v1/info", null);
    }

    /** Returns available source identifiers as raw JSON. */
    public String sources() {
        return request("GET", "/api/v1/sources", null);
    }

    /** Searches for tracks across the requested sources. */
    public String search(String query, List<String> sources, int limit) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("query", query);
        payload.put("sources", sources);
        payload.put("limit", limit);
        return request("POST", "/api/v1/search", payload);
    }

    /** Creates a playback session using the supplied output format. */
    public String createSession(Map<String, Object> output) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("application", "sdk");
        payload.put("version", "1.0.0");
        payload.put("output", output == null ? DEFAULT_OUTPUT : output);
        return request("POST", "/api/v1/sessions", payload);
    }

    /** Returns the current state of a session. */
    public String getSession(String id) {
        return request("GET", sessionPath(id), null);
    }

    /** Starts playback of a track. */
    public String play(String id, String trackId, long startMs) {
        Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("track_id", trackId);
        payload.put("start_position_ms", startMs);
        return request("POST", sessionPath(id) + "/play", payload);
    }

    /** Pauses a session. */
    public String pause(String id) {
        return request("POST", sessionPath(id) + "/pause", null);
    }

    /** Resumes a paused session. */
    public String resume(String id) {
        return request("POST", sessionPath(id) + "/resume", null);
    }

    /** Stops a session. */
    public String stop(String id) {
        return request("POST", sessionPath(id) + "/stop", null);
    }

    /** Seeks to a position in the current track. */
    public String seek(String id, long positionMs) {
        Map<String, Object> payload = Map.of("position_ms", positionMs);
        return request("POST", sessionPath(id) + "/seek", payload);
    }

    /** Sets a session's output volume. */
    public String setVolume(String id, int volume) {
        Map<String, Object> payload = Map.of("volume", volume);
        return request("POST", sessionPath(id) + "/volume", payload);
    }

    /** Returns a session's queue. */
    public String queue(String id) {
        return request("GET", sessionPath(id) + "/queue", null);
    }

    /** Adds a track to a session's queue. */
    public String enqueue(String id, String trackId) {
        Map<String, Object> payload = Map.of("track_id", trackId);
        return request("POST", sessionPath(id) + "/queue", payload);
    }

    /** Returns the session event WebSocket URL for an external WebSocket client. */
    public String websocketUrl(String id) {
        String scheme = "https".equalsIgnoreCase(baseUri.getScheme()) ? "wss" : "ws";
        return scheme + "://" + baseUri.getRawAuthority() + basePath() + "/ws/v1/sessions/"
                + encodePathSegment(id);
    }

    private String request(String method, String path, Object payload) {
        try {
            HttpRequest.Builder builder = HttpRequest.newBuilder(requestUri(path))
                    .header("Accept", "application/json");
            if (apiKey != null && !apiKey.isBlank()) {
                builder.header("Authorization", "Bearer " + apiKey);
            }
            if (payload == null) {
                builder.method(method, HttpRequest.BodyPublishers.noBody());
            } else {
                builder.header("Content-Type", "application/json");
                builder.method(method, HttpRequest.BodyPublishers.ofString(Json.encode(payload)));
            }

            HttpResponse<String> response = httpClient.send(
                    builder.build(), HttpResponse.BodyHandlers.ofString());
            String body = response.body() == null ? "" : response.body();
            if (response.statusCode() < 200 || response.statusCode() >= 300) {
                throw httpError(response.statusCode(), body);
            }
            return body.isBlank() ? "{}" : body;
        } catch (NexoraException error) {
            throw error;
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new NexoraException(0, "TRANSPORT_ERROR", "NEXORA request was interrupted", null);
        } catch (IOException | IllegalArgumentException error) {
            throw new NexoraException(0, "REQUEST_ERROR", error.getMessage(), null);
        }
    }

    private URI requestUri(String path) {
        return URI.create(baseUri.getScheme() + "://" + baseUri.getRawAuthority() + basePath() + path);
    }

    private String basePath() {
        String path = baseUri.getRawPath();
        if (path == null || path.isEmpty() || "/".equals(path)) {
            return "";
        }
        return path.endsWith("/") ? path.substring(0, path.length() - 1) : path;
    }

    private static String sessionPath(String id) {
        return "/api/v1/sessions/" + encodePathSegment(id);
    }

    private static String encodePathSegment(String value) {
        if (value == null) {
            throw new IllegalArgumentException("id must not be null");
        }
        return URLEncoder.encode(value, StandardCharsets.UTF_8)
                .replace("+", "%20")
                .replace("%7E", "~");
    }

    private static URI validateBaseUri(String value) {
        if (value == null || value.isBlank()) {
            throw new IllegalArgumentException("baseUrl must not be blank");
        }
        String normalized = value.trim();
        while (normalized.endsWith("/")) {
            normalized = normalized.substring(0, normalized.length() - 1);
        }
        URI uri;
        try {
            uri = URI.create(normalized);
        } catch (IllegalArgumentException error) {
            throw new IllegalArgumentException("baseUrl must be an absolute HTTP URL", error);
        }
        String scheme = uri.getScheme();
        if (scheme == null || (!"http".equalsIgnoreCase(scheme) && !"https".equalsIgnoreCase(scheme))
                || uri.getHost() == null || uri.getRawQuery() != null || uri.getRawFragment() != null) {
            throw new IllegalArgumentException("baseUrl must be an absolute HTTP URL without a query or fragment");
        }
        return uri;
    }

    private static NexoraException httpError(int status, String body) {
        String code = extractJsonString(body, "code");
        String message = extractJsonString(body, "message");
        if (code == null) {
            code = "HTTP_ERROR";
        }
        if (message == null) {
            message = "NEXORA request failed";
        }
        return new NexoraException(status, code, message, body);
    }

    private static String extractJsonString(String json, String field) {
        if (json == null || json.isBlank()) {
            return null;
        }
        Pattern pattern = Pattern.compile("\\\"" + Pattern.quote(field) + "\\\"\\s*:\\s*\\\"([^\\\"]*)\\\"");
        Matcher matcher = pattern.matcher(json);
        return matcher.find() ? matcher.group(1) : null;
    }

    /** An HTTP or transport failure returned by NEXORA. */
    public static final class NexoraException extends RuntimeException {
        private final int status;
        private final String code;
        private final String body;

        NexoraException(int status, String code, String message, String body) {
            super(message);
            this.status = status;
            this.code = code;
            this.body = body;
        }

        /** Returns the HTTP status code, or zero for a transport failure. */
        public int getStatus() {
            return status;
        }

        /** Returns the NEXORA error code. */
        public String getCode() {
            return code;
        }

        /** Returns the raw error response body when one was received. */
        public String getBody() {
            return body;
        }
    }

    /** Minimal JSON encoding for the primitive values accepted by this client. */
    private static final class Json {
        private Json() {
        }

        static String encode(Object value) {
            StringBuilder output = new StringBuilder();
            append(output, value);
            return output.toString();
        }

        private static void append(StringBuilder output, Object value) {
            if (value == null) {
                output.append("null");
            } else if (value instanceof String string) {
                appendString(output, string);
            } else if (value instanceof Number || value instanceof Boolean) {
                output.append(value);
            } else if (value instanceof Map<?, ?> map) {
                output.append('{');
                boolean first = true;
                for (Map.Entry<?, ?> entry : map.entrySet()) {
                    if (!first) {
                        output.append(',');
                    }
                    first = false;
                    appendString(output, String.valueOf(entry.getKey()));
                    output.append(':');
                    append(output, entry.getValue());
                }
                output.append('}');
            } else if (value instanceof Iterable<?> iterable) {
                output.append('[');
                boolean first = true;
                for (Object item : iterable) {
                    if (!first) {
                        output.append(',');
                    }
                    first = false;
                    append(output, item);
                }
                output.append(']');
            } else if (value.getClass().isArray()) {
                output.append('[');
                int length = Array.getLength(value);
                for (int index = 0; index < length; index++) {
                    if (index > 0) {
                        output.append(',');
                    }
                    append(output, Array.get(value, index));
                }
                output.append(']');
            } else {
                throw new IllegalArgumentException("Unsupported JSON value: " + value.getClass().getName());
            }
        }

        private static void appendString(StringBuilder output, String value) {
            output.append('"');
            for (int index = 0; index < value.length(); index++) {
                char character = value.charAt(index);
                switch (character) {
                    case '"' -> output.append("\\\"");
                    case '\\' -> output.append("\\\\");
                    case '\b' -> output.append("\\b");
                    case '\f' -> output.append("\\f");
                    case '\n' -> output.append("\\n");
                    case '\r' -> output.append("\\r");
                    case '\t' -> output.append("\\t");
                    default -> {
                        if (character < 0x20) {
                            output.append(String.format("\\u%04x", (int) character));
                        } else {
                            output.append(character);
                        }
                    }
                }
            }
            output.append('"');
        }
    }
}
