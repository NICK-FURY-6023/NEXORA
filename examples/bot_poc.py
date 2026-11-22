"""Bot-shaped end-to-end PoC for NEXORA.

This proves the control flow a Discord bot would use against the engine:
health/info, session create, queue, play, volume, state read and event URL.
No Discord token is needed. Actual Discord gateway/voice wiring is a separate
step that requires the bot's own credentials.
"""

from __future__ import annotations

import os
import sys
from pathlib import Path

SDK_ROOT = Path(__file__).resolve().parent.parent / "sdks" / "python"
sys.path.insert(0, str(SDK_ROOT))

from nexora_sdk import NexoraClient  # noqa: E402


def main() -> None:
    base_url = os.environ.get("NEXORA_URL", "http://127.0.0.1:18090")
    client = NexoraClient(base_url)

    print("info:", client.info())
    session = client.create_session()
    session_id = session["session_id"]
    print("session:", session_id)

    client.enqueue(session_id, "track_1")
    print("play:", client.play(session_id, "track_1"))
    client.set_volume(session_id, 80)

    state = client.get_session(session_id)
    print("state:", state["status"], state["volume"])
    print("queue:", client.queue(session_id))
    print("events:", client.websocket_url(session_id))


if __name__ == "__main__":
    main()
