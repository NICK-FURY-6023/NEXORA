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

## What is still required for live playback

An HTTP transport is not wired yet: the workspace has no generic HTTP client
dependency, and root `Cargo.toml` is owned by the other agent in this shared
tree. Adding one exact-pinned client crate and wiring `search`/`resolve` to
the parsers above is the next milestone. Until then these adapters are
parsers and validators, not live network sources.
