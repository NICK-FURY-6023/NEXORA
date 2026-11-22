// Package nexora provides a dependency-free client for the NEXORA v1 REST API.
package nexora

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"strings"
)

const defaultBaseURL = "http://127.0.0.1:8080"

func defaultOutput() map[string]any {
	return map[string]any{
		"sample_rate": 48000,
		"channels":    2,
		"codec":       "opus",
	}
}

// Client is a synchronous NEXORA REST client.
type Client struct {
	baseURL    string
	apiKey     string
	httpClient *http.Client
}

// NewClient creates a client. Pass an empty API key to disable bearer authentication.
func NewClient(baseURL string, apiKey string) *Client {
	if baseURL == "" {
		baseURL = defaultBaseURL
	}
	return &Client{
		baseURL:    strings.TrimRight(baseURL, "/"),
		apiKey:     apiKey,
		httpClient: http.DefaultClient,
	}
}

// NexoraError describes an HTTP or transport failure.
type NexoraError struct {
	Status  int
	Code    string
	Message string
	Body    string
}

// Error implements the error interface.
func (e *NexoraError) Error() string {
	if e == nil {
		return "<nil>"
	}
	return fmt.Sprintf("NEXORA request failed: %s (status %d, code %s)", e.Message, e.Status, e.Code)
}

// Info returns engine and API information.
func (c *Client) Info() (map[string]any, error) {
	return c.request(http.MethodGet, "/api/v1/info", nil)
}

// Sources returns available source identifiers.
func (c *Client) Sources() (map[string]any, error) {
	return c.request(http.MethodGet, "/api/v1/sources", nil)
}

// Search searches for tracks across the requested sources. A nil sources slice is sent as JSON null.
func (c *Client) Search(query string, sources []string, limit int) (map[string]any, error) {
	return c.request(http.MethodPost, "/api/v1/search", map[string]any{
		"query":   query,
		"sources": sources,
		"limit":   limit,
	})
}

// CreateSession creates a playback session. A nil output uses Opus at 48 kHz with two channels.
func (c *Client) CreateSession(output map[string]any) (map[string]any, error) {
	if output == nil {
		output = defaultOutput()
	}
	return c.request(http.MethodPost, "/api/v1/sessions", map[string]any{
		"application": "sdk",
		"version":     "1.0.0",
		"output":      output,
	})
}

// GetSession returns the current state of a session.
func (c *Client) GetSession(id string) (map[string]any, error) {
	return c.request(http.MethodGet, sessionPath(id), nil)
}

// Play starts playback of a track.
func (c *Client) Play(id string, trackID string, startMS int64) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/play", map[string]any{
		"track_id":         trackID,
		"start_position_ms": startMS,
	})
}

// Pause pauses a session.
func (c *Client) Pause(id string) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/pause", nil)
}

// Resume resumes a paused session.
func (c *Client) Resume(id string) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/resume", nil)
}

// Stop stops a session.
func (c *Client) Stop(id string) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/stop", nil)
}

// Seek seeks to a position in the current track.
func (c *Client) Seek(id string, positionMS int64) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/seek", map[string]any{
		"position_ms": positionMS,
	})
}

// SetVolume sets a session's output volume.
func (c *Client) SetVolume(id string, volume int) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/volume", map[string]any{
		"volume": volume,
	})
}

// Queue returns a session's queue.
func (c *Client) Queue(id string) (map[string]any, error) {
	return c.request(http.MethodGet, sessionPath(id)+"/queue", nil)
}

// Enqueue adds a track to a session's queue.
func (c *Client) Enqueue(id string, trackID string) (map[string]any, error) {
	return c.request(http.MethodPost, sessionPath(id)+"/queue", map[string]any{
		"track_id": trackID,
	})
}

// WebsocketURL returns the session event WebSocket URL for an external WebSocket client.
func (c *Client) WebsocketURL(id string) string {
	base, err := url.Parse(c.baseURL)
	if err != nil {
		return strings.Replace(c.baseURL, "http://", "ws://", 1)
	}
	if base.Scheme == "https" {
		base.Scheme = "wss"
	} else {
		base.Scheme = "ws"
	}

	escapedPath := strings.TrimRight(base.EscapedPath(), "/") + "/ws/v1/sessions/" + url.PathEscape(id)
	decodedPath, err := url.PathUnescape(escapedPath)
	if err != nil {
		decodedPath = escapedPath
	}
	base.Path = decodedPath
	base.RawPath = escapedPath
	base.RawQuery = ""
	base.Fragment = ""
	return base.String()
}

func (c *Client) request(method string, path string, payload any) (map[string]any, error) {
	var body io.Reader
	if payload != nil {
		encoded, err := json.Marshal(payload)
		if err != nil {
			return nil, &NexoraError{Code: "REQUEST_ERROR", Message: err.Error()}
		}
		body = bytes.NewReader(encoded)
	}

	request, err := http.NewRequest(method, c.baseURL+path, body)
	if err != nil {
		return nil, &NexoraError{Code: "REQUEST_ERROR", Message: err.Error()}
	}
	request.Header.Set("Accept", "application/json")
	if payload != nil {
		request.Header.Set("Content-Type", "application/json")
	}
	if c.apiKey != "" {
		request.Header.Set("Authorization", "Bearer "+c.apiKey)
	}

	response, err := c.httpClient.Do(request)
	if err != nil {
		return nil, &NexoraError{Code: "TRANSPORT_ERROR", Message: err.Error()}
	}
	defer response.Body.Close()

	raw, err := io.ReadAll(response.Body)
	if err != nil {
		return nil, &NexoraError{
			Status:  response.StatusCode,
			Code:    "TRANSPORT_ERROR",
			Message: err.Error(),
		}
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return nil, apiError(response.StatusCode, raw)
	}
	if len(bytes.TrimSpace(raw)) == 0 {
		return map[string]any{}, nil
	}

	var result map[string]any
	if err := json.Unmarshal(raw, &result); err != nil {
		return nil, &NexoraError{
			Status:  response.StatusCode,
			Code:    "INVALID_RESPONSE",
			Message: err.Error(),
			Body:    string(raw),
		}
	}
	return result, nil
}

func apiError(status int, body []byte) *NexoraError {
	var payload struct {
		Error struct {
			Code    string `json:"code"`
			Message string `json:"message"`
		} `json:"error"`
	}
	_ = json.Unmarshal(body, &payload)

	code := payload.Error.Code
	if code == "" {
		code = "HTTP_ERROR"
	}
	message := payload.Error.Message
	if message == "" {
		message = "NEXORA request failed"
	}
	return &NexoraError{
		Status:  status,
		Code:    code,
		Message: message,
		Body:    string(body),
	}
}

func sessionPath(id string) string {
	return "/api/v1/sessions/" + url.PathEscape(id)
}
