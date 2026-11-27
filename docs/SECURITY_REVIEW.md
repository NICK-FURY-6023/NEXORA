# NEXORA Security Review — 2026-11-22, updated 2026-11-27

Scope: Phase 1-4 core, SDKs, deployment artifacts, cluster registry, plugin
contract, and the M10 WASM runtime.

## Checks passed

- No Discord tokens, API keys, passwords, or connection strings are present in tracked source or docs. `deploy/.env.example` uses only an empty password placeholder.
- HTTP server accepts an optional bearer password via `NEXORA_SERVER_PASSWORD`; when unset it is development-only and documented as such.
- Source framework validates HTTP URLs and blocks loopback, private, link-local, and localhost targets before DNS (SSRF guard).
- Local source adapter rejects path traversal.
- Plugin manifests are validated; sandbox policy is restrictive by default (no network, no filesystem, 5 s / 64 MB limits).
- SDKs use only documented endpoints and do not embed credentials.
- Docker image does not copy `.env`, `config.json`, or local databases.

## Checks added with the WASM runtime (M10)

- `PluginRuntime` executes guest modules under `wasmi` with fuel as the time
  bound and the engine limiter as the memory bound; an infinite loop is stopped
  rather than hanging the server.
- Only manifest-granted host effects are reachable. `host.fetch` rechecks the
  sandbox host policy, and `host.push` copies a bounded UTF-8 string from guest
  memory (up to 1 MiB).
- A denied host call is returned to the host as a policy error; it is not
  silently downgraded or granted.

## Remaining risks / deferred work

- Lavalink client conformance is partial; more `/v4` endpoints should be tested against real clients.
- Session state is in-memory and lost on restart; durable state needs authentication and encryption review.
- Monitoring configs are examples only; no Prometheus exposition is wired.
- Java/Go/C#/PHP SDK sources were not compiled because toolchains are unavailable on this host.
- Credentialed live-provider flows for Spotify, Deezer, Apple Music and Tidal
  require the user's own authorized token and are not covered by this review.

## Decision

No new credential exposure or scope violation was found in this increment. Proceed with the documented limitations explicitly tracked.
