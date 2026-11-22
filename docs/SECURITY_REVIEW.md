# NEXORA Security Review — 2026-11-22

Scope: Phase 1-4 core, SDKs, deployment artifacts, cluster registry, plugin contract.

## Checks passed

- No Discord tokens, API keys, passwords, or connection strings are present in tracked source or docs. `deploy/.env.example` uses only an empty password placeholder.
- HTTP server accepts an optional bearer password via `NEXORA_SERVER_PASSWORD`; when unset it is development-only and documented as such.
- Source framework validates HTTP URLs and blocks loopback, private, link-local, and localhost targets before DNS (SSRF guard).
- Local source adapter rejects path traversal.
- Plugin manifests are validated; sandbox policy is restrictive by default (no network, no filesystem, 5 s / 64 MB limits).
- SDKs use only documented endpoints and do not embed credentials.
- Docker image does not copy `.env`, `config.json`, or local databases.

## Remaining risks / deferred work

- Lavalink client conformance is partial; more `/v4` endpoints should be tested against real clients.
- Plugin WASM runtime is not implemented; the manifest contract is not yet enforced by a sandbox executor.
- Session state is in-memory and lost on restart; durable state needs authentication and encryption review.
- Monitoring configs are examples only; no Prometheus exposition is wired.
- Java/Go/C#/PHP SDK sources were not compiled because toolchains are unavailable on this host.

## Decision

No new credential exposure or scope violation was found in this increment. Proceed with the documented limitations explicitly tracked.
