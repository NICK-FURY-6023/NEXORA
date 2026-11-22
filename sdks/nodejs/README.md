# NEXORA Node.js SDK

Minimal, dependency-free SDK using Node 24 built-ins (`fetch`, `WebSocket`).

```js
import { NexoraClient } from "@nexora/sdk";

const client = new NexoraClient({ baseUrl: "http://127.0.0.1:8080", apiKey: process.env.NEXORA_SERVER_PASSWORD });
await client.search("song", ["local-media"], 10);
const session = await client.createSession();
await client.play(session.session_id, "track_1");

const socket = client.subscribe(session.session_id, (event) => console.log(event.event_type));
// socket.close() when done.
```

Run tests: `cd sdks/nodejs && node --test`.
