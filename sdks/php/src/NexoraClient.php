<?php

declare(strict_types=1);

/**
 * A dependency-free client for the NEXORA v1 REST API.
 */
final class NexoraClient
{
    private const DEFAULT_BASE_URL = 'http://127.0.0.1:8080';

    private string $baseUrl;
    private ?string $apiKey;

    public function __construct(string $baseUrl = self::DEFAULT_BASE_URL, ?string $apiKey = null)
    {
        $normalized = rtrim(trim($baseUrl), '/');
        $parts = parse_url($normalized);
        if ($parts === false || !isset($parts['scheme'], $parts['host']) ||
            !in_array(strtolower($parts['scheme']), ['http', 'https'], true) ||
            isset($parts['query']) || isset($parts['fragment'])) {
            throw new InvalidArgumentException(
                'baseUrl must be an absolute HTTP URL without a query or fragment'
            );
        }

        $this->baseUrl = $normalized;
        $this->apiKey = $apiKey;
    }

    /** Returns engine and API information. */
    public function info(): array
    {
        return $this->request('GET', '/api/v1/info');
    }

    /** Returns available source identifiers. */
    public function sources(): array
    {
        return $this->request('GET', '/api/v1/sources');
    }

    /** Searches for tracks across the requested sources. */
    public function search(string $query, ?array $sources = null, int $limit = 10): array
    {
        return $this->request('POST', '/api/v1/search', [
            'query' => $query,
            'sources' => $sources,
            'limit' => $limit,
        ]);
    }

    /** Creates a playback session. A null output uses Opus at 48 kHz with two channels. */
    public function createSession(?array $output = null): array
    {
        return $this->request('POST', '/api/v1/sessions', [
            'application' => 'sdk',
            'version' => '1.0.0',
            'output' => $output ?? [
                'sample_rate' => 48000,
                'channels' => 2,
                'codec' => 'opus',
            ],
        ]);
    }

    /** Returns the current state of a session. */
    public function getSession(string $id): array
    {
        return $this->request('GET', $this->sessionPath($id));
    }

    /** Starts playback of a track. */
    public function play(string $id, string $trackId, int $startMs = 0): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/play', [
            'track_id' => $trackId,
            'start_position_ms' => $startMs,
        ]);
    }

    /** Pauses a session. */
    public function pause(string $id): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/pause');
    }

    /** Resumes a paused session. */
    public function resume(string $id): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/resume');
    }

    /** Stops a session. */
    public function stop(string $id): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/stop');
    }

    /** Seeks to a position in the current track. */
    public function seek(string $id, int $positionMs): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/seek', [
            'position_ms' => $positionMs,
        ]);
    }

    /** Sets a session's output volume. */
    public function setVolume(string $id, int $volume): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/volume', [
            'volume' => $volume,
        ]);
    }

    /** Returns a session's queue. */
    public function queue(string $id): array
    {
        return $this->request('GET', $this->sessionPath($id) . '/queue');
    }

    /** Adds a track to a session's queue. */
    public function enqueue(string $id, string $trackId): array
    {
        return $this->request('POST', $this->sessionPath($id) . '/queue', [
            'track_id' => $trackId,
        ]);
    }

    /** Returns the session event WebSocket URL for an external WebSocket client. */
    public function websocketUrl(string $id): string
    {
        $parts = parse_url($this->baseUrl);
        if ($parts === false) {
            throw new RuntimeException('Invalid base URL');
        }

        $scheme = strtolower($parts['scheme']) === 'https' ? 'wss' : 'ws';
        $host = $parts['host'];
        if (strpos($host, ':') !== false && !str_starts_with($host, '[')) {
            $host = '[' . $host . ']';
        }
        $authority = $host;
        if (isset($parts['port'])) {
            $authority .= ':' . $parts['port'];
        }
        $path = rtrim($parts['path'] ?? '', '/');

        return $scheme . '://' . $authority . $path . '/ws/v1/sessions/' . rawurlencode($id);
    }

    /** @throws NexoraException */
    private function request(string $method, string $path, ?array $payload = null): array
    {
        $headers = ['Accept: application/json'];
        if ($this->apiKey !== null && $this->apiKey !== '') {
            $headers[] = 'Authorization: Bearer ' . $this->apiKey;
        }

        $options = [
            'http' => [
                'method' => $method,
                'ignore_errors' => true,
                'timeout' => 30,
                'header' => implode("\r\n", $headers),
            ],
        ];
        if ($payload !== null) {
            try {
                $encoded = json_encode($payload, JSON_THROW_ON_ERROR);
            } catch (JsonException $error) {
                throw new NexoraException(0, 'REQUEST_ERROR', $error->getMessage(), null);
            }
            $headers[] = 'Content-Type: application/json';
            $options['http']['header'] = implode("\r\n", $headers);
            $options['http']['content'] = $encoded;
        }

        $body = @file_get_contents(
            $this->baseUrl . $path,
            false,
            stream_context_create($options)
        );
        if ($body === false) {
            $lastError = error_get_last();
            $message = $lastError['message'] ?? 'NEXORA request failed';
            throw new NexoraException(0, 'TRANSPORT_ERROR', trim($message), null);
        }

        $status = $this->responseStatus($http_response_header ?? []);
        if ($status < 200 || $status >= 300) {
            throw $this->parseError($status, $body);
        }
        if (trim($body) === '') {
            return [];
        }

        try {
            $decoded = json_decode($body, true, 512, JSON_THROW_ON_ERROR);
        } catch (JsonException $error) {
            throw new NexoraException($status, 'INVALID_RESPONSE', $error->getMessage(), $body);
        }
        if (!is_array($decoded)) {
            throw new NexoraException($status, 'INVALID_RESPONSE', 'Expected a JSON object', $body);
        }
        return $decoded;
    }

    private function responseStatus(array $headers): int
    {
        $status = 0;
        foreach ($headers as $header) {
            if (preg_match('/^HTTP\/\S+\s+(\d{3})/', $header, $matches) === 1) {
                $status = (int) $matches[1];
            }
        }
        return $status;
    }

    private function parseError(int $status, string $body): NexoraException
    {
        $code = null;
        $message = null;
        $decoded = json_decode($body, true);
        if (is_array($decoded) && isset($decoded['error']) && is_array($decoded['error'])) {
            $code = isset($decoded['error']['code']) ? (string) $decoded['error']['code'] : null;
            $message = isset($decoded['error']['message']) ? (string) $decoded['error']['message'] : null;
        }

        return new NexoraException(
            $status,
            $code ?? 'HTTP_ERROR',
            $message ?? "NEXORA request failed with HTTP {$status}",
            $body
        );
    }

    private function sessionPath(string $id): string
    {
        return '/api/v1/sessions/' . rawurlencode($id);
    }
}

/** An HTTP or transport failure returned by NEXORA. */
final class NexoraException extends RuntimeException
{
    public int $status;
    public string $errorCode;
    public ?string $body;

    public function __construct(int $status, string $errorCode, string $message, ?string $body)
    {
        parent::__construct($message, $status);
        $this->status = $status;
        $this->errorCode = $errorCode;
        $this->body = $body;
    }
}
