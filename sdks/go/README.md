# NEXORA Go SDK

Dependency-free Go client for the NEXORA v1 REST API.

```go
client := nexora.NewClient("http://127.0.0.1:8080", "optional-api-key")
info, err := client.Info()
session, err := client.CreateSession(nil)
_, err = client.Play(session["session_id"].(string), "track-123", 0)
```

The client also exposes `Sources`, `Search`, `GetSession`, `Pause`, `Resume`, `Stop`, `Seek`, `SetVolume`, `Queue`, `Enqueue`, and `WebsocketURL`.

HTTP failures return `*nexora.NexoraError` with `Status` and `Code` fields.
