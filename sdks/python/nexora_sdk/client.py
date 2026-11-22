"""Synchronous HTTP client for the NEXORA API."""

from __future__ import annotations

import json
import urllib.error
import urllib.parse
import urllib.request
from typing import Any

_DEFAULT_BASE_URL = "http://127.0.0.1:8080"
_DEFAULT_OUTPUT = {"sample_rate": 48000, "channels": 2, "codec": "opus"}


class NexoraSDKError(Exception):
    """An HTTP or transport error returned by NEXORA."""

    def __init__(self, message: str, status_code: int | None = None, error_body: Any = None):
        super().__init__(message)
        self.status_code = status_code
        self.error_body = error_body


class NexoraClient:
    """Client for NEXORA's versioned REST API."""

    def __init__(
        self,
        base_url: str = _DEFAULT_BASE_URL,
        api_key: str | None = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key

    def info(self) -> dict[str, Any]:
        """Return engine and API information."""
        return self._request("GET", "/api/v1/info")

    def sources(self) -> dict[str, Any]:
        """Return the available source identifiers."""
        return self._request("GET", "/api/v1/sources")

    def search(
        self,
        query: str,
        sources: list[str] | None = None,
        limit: int = 10,
    ) -> dict[str, Any]:
        """Search for tracks across the requested sources."""
        return self._request(
            "POST",
            "/api/v1/search",
            {"query": query, "sources": sources, "limit": limit},
        )

    def create_session(self, output: dict[str, Any] | None = None) -> dict[str, Any]:
        """Create a playback session."""
        return self._request(
            "POST",
            "/api/v1/sessions",
            {
                "application": "python-sdk",
                "version": "1.0.0",
                "output": output if output is not None else _DEFAULT_OUTPUT.copy(),
            },
        )

    def get_session(self, session_id: str) -> dict[str, Any]:
        """Return the current state of a session."""
        return self._request("GET", self._session_path(session_id))

    def play(
        self,
        session_id: str,
        track_id: str,
        start_position_ms: int = 0,
    ) -> dict[str, Any]:
        """Start playback of a track in a session."""
        return self._request(
            "POST",
            f"{self._session_path(session_id)}/play",
            {"track_id": track_id, "start_position_ms": start_position_ms},
        )

    def pause(self, session_id: str) -> dict[str, Any]:
        """Pause a session."""
        return self._request("POST", f"{self._session_path(session_id)}/pause", {})

    def resume(self, session_id: str) -> dict[str, Any]:
        """Resume a paused session."""
        return self._request("POST", f"{self._session_path(session_id)}/resume", {})

    def stop(self, session_id: str) -> dict[str, Any]:
        """Stop a session."""
        return self._request("POST", f"{self._session_path(session_id)}/stop", {})

    def seek(self, session_id: str, position_ms: int) -> dict[str, Any]:
        """Seek to a position in the current track."""
        return self._request(
            "POST",
            f"{self._session_path(session_id)}/seek",
            {"position_ms": position_ms},
        )

    def set_volume(self, session_id: str, volume: int) -> dict[str, Any]:
        """Set a session's output volume."""
        return self._request(
            "POST",
            f"{self._session_path(session_id)}/volume",
            {"volume": volume},
        )

    def queue(self, session_id: str) -> dict[str, Any]:
        """Return the queue for a session."""
        return self._request("GET", f"{self._session_path(session_id)}/queue")

    def enqueue(self, session_id: str, track_id: str) -> dict[str, Any]:
        """Add a track to a session's queue."""
        return self._request(
            "POST",
            f"{self._session_path(session_id)}/queue",
            {"track_id": track_id},
        )

    def websocket_url(self, session_id: str) -> str:
        """Return the session event WebSocket URL for an external WS client."""
        parsed = urllib.parse.urlsplit(self.base_url)
        scheme = {"http": "ws", "https": "wss"}.get(parsed.scheme, parsed.scheme)
        path = f"/ws/v1/sessions/{urllib.parse.quote(session_id, safe='')}"
        return urllib.parse.urlunsplit((scheme, parsed.netloc, path, "", ""))

    @staticmethod
    def _session_path(session_id: str) -> str:
        return f"/api/v1/sessions/{urllib.parse.quote(session_id, safe='')}"

    def _request(
        self,
        method: str,
        path: str,
        payload: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        data = None
        headers = {"Accept": "application/json"}
        if payload is not None:
            data = json.dumps(payload).encode("utf-8")
            headers["Content-Type"] = "application/json"
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"

        request = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            headers=headers,
            method=method,
        )

        try:
            with urllib.request.urlopen(request) as response:
                body = response.read()
        except urllib.error.HTTPError as error:
            raw_body = error.read()
            error_body = self._decode_error_body(raw_body)
            message = self._error_message(error_body)
            raise NexoraSDKError(message, error.code, error_body) from error
        except urllib.error.URLError as error:
            raise NexoraSDKError(str(error.reason), None, str(error.reason)) from error

        if not body:
            return {}
        return json.loads(body.decode("utf-8"))

    @staticmethod
    def _decode_error_body(raw_body: bytes) -> Any:
        if not raw_body:
            return None
        text = raw_body.decode("utf-8", errors="replace")
        try:
            return json.loads(text)
        except json.JSONDecodeError:
            return text

    @staticmethod
    def _error_message(error_body: Any) -> str:
        if isinstance(error_body, dict):
            error = error_body.get("error")
            if isinstance(error, dict):
                message = error.get("message")
                if message:
                    return str(message)
            message = error_body.get("message")
            if message:
                return str(message)
        if error_body:
            return str(error_body)
        return "NEXORA request failed"
