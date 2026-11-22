# ADR-021: Repository documentation layout

## Status

Accepted, 2026-11-22.

## Context

NEXORA grew from a README and a flat `docs/` folder. Contributors and community
members could not tell which document was authoritative:

- the 1300-line root README mixes product vision, proposed architecture, and
  status;
- `docs/SYSTEM_DESIGN.md` and `docs/NEXORA_SYSTEM_DESIGN.md` both describe a
  system design, but only the first matches implemented code;
- deployment, API, research, and development docs had no single navigation
  entry;
- Lavalink solved this with a short repository README plus `docs/` organized by
  reader intent (getting started, configuration, API, changelog) and a nav file.

## Decision

Keep existing file paths to avoid link churn. Add one authoritative navigation
page, `docs/README.md`, that maps reader questions to the correct document.
Keep the root README as product vision, link it to the docs index, and mark
`docs/NEXORA_SYSTEM_DESIGN.md` as a superseded historical draft.

Document the repository map and "where does a change belong" ownership table in
`docs/README.md`. Add this ADR to `docs/adr/` using the existing numbering and
filename convention.

This is an information-architecture decision, not a new runtime dependency. No
mkdocs site is added in this slice because the project has no docs-site toolchain
yet and the immediate need is clarity, not theming.

## Alternatives considered

### Build a full mkdocs site now

- Pros: matches Lavalink's rendered docs experience.
- Cons: adds a Python toolchain, theme config, redirect management, and a new
  maintenance surface before the current docs are consolidated.
- Rejected: do the consolidation first; a docs site can be added later without
  moving content.

### Move every doc into new folders

- Pros: visually tidy.
- Cons: breaks existing links and hides the actual problem, which is ambiguity
  about authority.
- Rejected: higher churn without solving reader confusion.

### Leave the flat docs folder as-is

- Pros: no work.
- Cons: community members keep reading stale or speculative documents.
- Rejected: the project now has working runtime behavior that must be easy to
  find.

## Consequences

- Every future documentation addition must be linked from `docs/README.md`.
- `docs/SYSTEM_DESIGN.md` is the single authoritative source for implemented
  design and invariants.
- Historical research stays in `docs/research/` and is labeled as evidence, not
  current truth.
- Contributors can identify the owning crate and test file for a change without
  searching the whole workspace.
