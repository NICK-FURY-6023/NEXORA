"""Bot-shaped end-to-end PoC for NEXORA.

This proves the control flow a Discord bot would use against the engine:
health/info, session create, queue, play, volume, state read and event URL.
No Discord token is needed and no third-party package is required: the
requests below go straight to the documented ``/api/v1`` contract with the
Python standard library. Actual Discord gateway/voice wiring is a separate
step that requires the bot's own credentials.
"""

from __future__ import annotations

import json
import os
import urllib.parse
import urllib.request
from typing import Any

BASE_URL = os.environ.get("NEXORA_URL", "http://127.0.0.1:18090").rstrip("/")
DEFAULT_OUTPUT = {"sample_rate": 48000, "channels": 2, "codec": "opus"}


def request(method: str, path: str, payload: dict[str, Any] | None = None) -> dict[str, Any]:
    data = None
    headers = {"Accept": "application/json"}
    if payload is not None:
        data = json.dumps(payload).encode("utf-8")
        headers["Content-Type"] = "application/json"

    req = urllib.request.Request(
        f"{BASE_URL}{path}",
        data=data,
        headers=headers,
        method=method,
    )
    with urllib.request.urlopen(req) as response:
        body = response.read()
    return json.loads(body.decode("utf-8")) if body else {}


def session_path(session_id: str) -> str:
    return f"/api/v1/sessions/{urllib.parse.quote(session_id, safe='')}"


def websocket_url(session_id: str) -> str:
    parsed = urllib.parse.urlsplit(BASE_URL)
    scheme = {"http": "ws", "https": "wss"}.get(parsed.scheme, parsed.scheme)
    path = f"/ws/v1/sessions/{urllib.parse.quote(session_id, safe='')}"
    return urllib.parse.urlunsplit((scheme, parsed.netloc, path, "", ""))


def main() -> None:
    print("info:", request("GET", "/api/v1/info"))

    session = request(
        "POST",
        "/api/v1/sessions",
        {"application": "bot-poc", "version": "1.0.0", "output": DEFAULT_OUTPUT},
    )
    session_id = session["session_id"]
    print("session:", session_id)

    # The local-media adapter addresses a file by its file name inside the
    # configured media root, so the track id keeps the extension.
    track_id = os.environ.get("NEXORA_TRACK", "track_1.wav")
    print("enqueue:", request("POST", f"{session_path(session_id)}/queue", {"track_id": track_id}))
    print(
        "play:",
        request(
            "POST",
            f"{session_path(session_id)}/play",
            {"track_id": track_id, "start_position_ms": 0},
        ),
    )
    print("volume:", request("POST", f"{session_path(session_id)}/volume", {"volume": 80}))

    state = request("GET", session_path(session_id))
    print("state:", state["status"], state["volume"])
    print("queue:", request("GET", f"{session_path(session_id)}/queue"))
    print("events:", websocket_url(session_id))


if __name__ == "__main__":
    main()
