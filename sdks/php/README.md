# NEXORA PHP SDK

Dependency-free PHP 8 client for the NEXORA v1 REST API.

```php
require 'src/NexoraClient.php';

$client = new NexoraClient('http://127.0.0.1:8080', 'optional-api-key');
$info = $client->info();
$session = $client->createSession();
$client->play($session['session_id'], 'track-123', 0);
```

The client also exposes `sources`, `search`, `getSession`, `pause`, `resume`, `stop`, `seek`, `setVolume`, `queue`, `enqueue`, and `websocketUrl`.

Failed requests throw `NexoraException`; its `status` and `errorCode` properties expose the HTTP status and NEXORA error code.
