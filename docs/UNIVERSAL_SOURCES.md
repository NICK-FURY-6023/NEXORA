# NEXORA Universal Sources

This matrix replaces the old "commercial providers are a non-goal" line in
`docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md`. Every adapter
is our own Rust code in `crates/nexora-source-universal`; no Java plugin,
third-party source library or hosted resolver is used.

| Adapter id | Example input | User configuration | Status |
|---|---|---|---|
| `internet-archive` | `https://archive.org/details/...` | none | metadata + resolution parsing implemented |
| `bandcamp-public` | `https://artist.bandcamp.com/track/...` | none | oEmbed parsing + host validation implemented |
| `soundcloud-public` | `https://soundcloud.com/artist/...` | none | oEmbed parsing + host validation implemented |
| `youtube` | `https://www.youtube.com/watch?v=...` | none for public metadata | search/details parsing implemented; stream resolution is subject to the platform's terms |
| `spotify` | `https://open.spotify.com/track/...` | one-time OAuth login | token store + PKCE implemented; search/preview parsing implemented |
| `deezer` | `https://www.deezer.com/track/...` | one-time OAuth login | token gate + parsing implemented |
| `apple-music` | `https://music.apple.com/...` | one-time OAuth login | token gate + parsing implemented |
| `tidal` | `https://tidal.com/browse/track/...` | one-time OAuth login | token gate + parsing implemented |

The honest boundary: "zero configuration" is only true for Tier A and for
public YouTube metadata. Spotify, Deezer, Apple Music and Tidal require the
user's own authorized account — no software can generate a provider-issued
token. NEXORA does not circumvent DRM, provider authentication or access
controls.

## What is implemented now

- `detect()` maps all eight URL families to their adapter id.
- Recorded-fixture tests cover metadata mapping, missing fields, host
  validation and SSRF rejection for private, loopback and link-local hosts.
- `TokenStore` keeps provider tokens in memory and builds a real PKCE
  authorization URL (random verifier, SHA-256 challenge, base64url).
- Provider status codes map to `SourceError` (`401 -> Unauthorized`,
  `429 -> RateLimited`, `5xx -> Unavailable`).

## Live playback status

All eight adapters are registered by `register_builtin_sources` in
`services/nexora-server/src/lib.rs`, and the crate has a bounded `reqwest`
transport (`src/transport.rs` with the shared `src/http.rs` reader). Recorded
fixtures cover metadata mapping, missing fields, host validation and SSRF
rejection for private, loopback and link-local hosts; `search`/`resolve` call
the parsers through that transport.

What still needs a real user account is provider credentials: Spotify, Deezer,
Apple Music and Tidal only play when the user supplies their own authorized
token. Public YouTube stream resolution remains subject to the platform's
terms, so the shipped code parses public metadata and refuses to bypass access
controls. No credentialed live-provider test suite is claimed here.

Opt-in live checks live in `crates/nexora-source-universal/tests/live_providers.rs`.
They are ignored by default; set `NEXORA_LIVE_SPOTIFY_TOKEN`,
`NEXORA_LIVE_DEEZER_TOKEN`, `NEXORA_LIVE_APPLE_TOKEN` or
`NEXORA_LIVE_TIDAL_TOKEN` to the user's own token and run
`cargo test -p nexora-source-universal --test live_providers -- --ignored`.
No token value is stored in the repository. A plain run reports four ignored
tests; with `--ignored`, a test with no token skips early and a test with a
token must return at least one parseable result.
