# NEXORA C# SDK

Dependency-free .NET client for the NEXORA v1 REST API.

```csharp
using var client = new NexoraClient("http://127.0.0.1:8080", "optional-api-key");
var info = await client.Info();
var session = await client.CreateSession();
await client.Play(session.GetProperty("session_id").GetString()!, "track-123", 0);
```

The client also exposes `Sources`, `Search`, `GetSession`, `Pause`, `Resume`, `Stop`, `Seek`, `SetVolume`, `Queue`, `Enqueue`, and `WebsocketUrl`.

Failed requests throw `NexoraException`; its `Status` and `Code` properties expose the HTTP status and NEXORA error code.
