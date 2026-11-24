# Security Policy

NEXORA is an audio server that runs on the operator's own machine. It accepts
network requests, reads local media, opens a WebSocket and (once the voice
sender ships) speaks to Discord. That makes it worth reporting problems in
private.

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x | yes |
| < 0.1 | no |

NEXORA is pre-1.0. Fixes land in the next patch release of the supported
line; there is no long-term-support branch.

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.** Use the private route below
instead. A public issue, a pull request or a chat message about an unfixed
problem can be read by someone who will use it.

Report privately to the maintainer: **partham302@gmail.com**

If you prefer a platform with a private reporting flow, ask the maintainer for
a GitHub Security Advisory link for this repository.

Include:

- the affected component (crate, service or endpoint),
- how to reproduce it — the request, the track, the configuration,
- what an attacker gains, and what they need in order to try it,
- the version or commit sha you tested.

Redact before sending: no tokens, no passwords, no cookies, no real user data.
Anonymised logs are enough.

## What happens next

The maintainer reads every report and tries to reproduce it. An
acknowledgement is best effort — no response-time guarantee is promised, and
none should be assumed. A confirmed problem is fixed in the next patch
release, and the report is credited in the release notes unless you ask not to
be named.

If a report turns out to be a configuration mistake rather than a flaw, it is
answered as such and not treated as a vulnerability.

## Disclosure

Keep a confirmed problem private until a release contains the fix. There is no
embargo policy beyond that: the maintainer publishes the fix and the notes
together, and anyone who reported it is credited unless they decline.

## Scope

In scope: the NEXORA server, the gateway, the audio engine, the source
adapters, the plugin API, and the deployment artifacts in `deploy/`.

Out of scope: vulnerabilities in upstream dependencies (report them to the
upstream project), in the JVM or the operating system, and in a source
provider's own service. A crash in a third-party crate reached only through
NEXORA's normal use is still worth reporting — say so and include the
dependency version.

## Security posture

The current review of the project's security properties lives in
[`docs/SECURITY_REVIEW.md`](docs/SECURITY_REVIEW.md): bearer-token
authorization on every route except the health and info endpoints, SSRF-safe
URL validation, path-traversal rejection for local media, bounded broadcast
channels, and volume clamping. That document records what is implemented; this
one records how to reach the maintainer.
