# NEXORA Python SDK

A synchronous Python client for the NEXORA v1 HTTP API. It uses only the Python standard library.

## Install

From this directory, install the package locally:

```bash
python -m pip install .
```

You can also add this directory to `PYTHONPATH` and import `nexora_sdk` without installing it.

## Usage

```python
from nexora_sdk import NexoraClient, NexoraSDKError

client = NexoraClient(
    base_url="http://127.0.0.1:8080",
    api_key="your-api-key",  # optional
)

print(client.info())
print(client.sources())

results = client.search("ambient", sources=["local", "radio"], limit=10)
session = client.create_session()
session_id = session["session_id"]

client.play(session_id, "track-123")
client.set_volume(session_id, 75)
print(client.queue(session_id))

try:
    client.pause("missing-session")
except NexoraSDKError as error:
    print(error.status_code, error.error_body)
```

`create_session()` defaults to Opus output at 48 kHz with two channels. Pass an `output` dictionary to override it.

## WebSocket events

The SDK does not implement a WebSocket client. Use `websocket_url()` to obtain the session event URL and pass it to an external WebSocket library:

```python
url = client.websocket_url(session_id)
```

For an `http://` base URL the result uses `ws://`; for `https://` it uses `wss://`.

## Tests

```bash
python -m unittest discover -s tests -v
```
