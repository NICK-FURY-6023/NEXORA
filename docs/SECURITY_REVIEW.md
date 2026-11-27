# NEXORA Security Review — 2026-11-22, updated 2026-11-27

Scope: Phase 1-4 core, deployment artifacts, cluster registry, plugin contract,
and the M10 WASM runtime. The `sdks/` tree that this review originally covered
was removed in commit `32208b4`; no SDK ships today.

## Checks passed

- No Discord tokens, API keys, passwords, or connection strings are present in tracked source or docs. `deploy/.env.example` uses only an empty password placeholder.
- HTTP server accepts an optional bearer password via `NEXORA_SERVER_PASSWORD`; when unset it is development-only and documented as such.
- Source framework validates HTTP URLs and blocks loopback, private, link-local, and localhost targets before DNS (SSRF guard).
- Local source adapter rejects path traversal.
- Plugin manifests are validated; sandbox policy is restrictive by default (no network, no filesystem, 5 s / 64 MB limits).
- No credentials are embedded in tracked client code; the only client example
  (`examples/bot_poc.py`) uses documented `/api/v1` endpoints only.
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

- Lavalink client conformance is partial; more `/nexora/*` endpoints should be tested against real clients.
- Session snapshots are opt-in via `--session-snapshot` and store no credentials; the snapshot file is not encrypted, so host filesystem access control is required.
- Authenticated Prometheus exposition exists at `GET /metrics`; alerting rules and long-term storage are not configured.
- No SDK is shipped, so no SDK surface is covered by this review.
- Credentialed live-provider flows for Spotify, Deezer, Apple Music and Tidal
  require the user's own authorized token and are not covered by this review.

## Decision

No new credential exposure or scope violation was found in this increment. Proceed with the documented limitations explicitly tracked.
