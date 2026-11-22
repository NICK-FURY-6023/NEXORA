# NEXORA Java SDK

Dependency-free Java 11+ client for the NEXORA v1 REST API. Responses are raw JSON strings.

```java
import dev.nexora.NexoraClient;
import java.util.Map;

NexoraClient client = new NexoraClient("http://127.0.0.1:8080", "optional-api-key");
String info = client.info();
String session = client.createSession(Map.of(
        "sample_rate", 48000,
        "channels", 2,
        "codec", "opus"));
```

The client also exposes `sources`, `search`, `getSession`, `play`, `pause`, `resume`, `stop`, `seek`, `setVolume`, `queue`, `enqueue`, and `websocketUrl`.

Failed requests throw `NexoraClient.NexoraException`; `getStatus()` and `getCode()` expose the HTTP status and NEXORA error code.
