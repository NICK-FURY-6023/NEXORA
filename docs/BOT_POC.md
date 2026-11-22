# Bot proof of concept

`examples/bot_poc.py` drives NEXORA through the same control flow a Discord
bot uses: engine info, session create, enqueue, play, volume, state and the
session event WebSocket URL. It uses the bundled Python SDK and the standard
library only.

## Run

```bash
cargo build -p nexora-server
./target/debug/nexora-server --port 18090 --grpc-port 18091
# in another shell:
python3 examples/bot_poc.py
```

Override the target with `NEXORA_URL=http://127.0.0.1:8080`.

## What this proves

The engine accepts a bot-shaped client and completes the session lifecycle
over `/api/v1` without any external service. The event URL is handed to the
caller; a real Discord bot would open that WebSocket and forward Discord
voice state (token, endpoint, session id, channel id) through the documented
voice path.

## Boundary

Actual Discord gateway/voice connection needs the bot's own token and voice
server payloads. No token is stored in this repository. The PoC deliberately
stops before that trust boundary so it can run on any machine.
