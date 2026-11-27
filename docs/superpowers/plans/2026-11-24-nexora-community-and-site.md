# NEXORA — community health and documentation site — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give NEXORA the community surface a public project needs — a security policy, a code of conduct, issue and PR templates, and a documentation website built from the docs tree.

**Architecture:** The site is [mdbook](https://rust-lang.github.io/mdBook/), so the site's content is the repository's existing markdown: `docs/README.md` becomes the landing page and the `docs/` tree becomes the book chapters. No content is duplicated; the book reads the same files contributors edit. Policies live at the repository root where GitHub expects them.

**Tech Stack:** mdbook (Rust toolchain), GitHub Actions for the build and the Pages deploy, plain markdown for every policy and template.

**Spec:** `docs/superpowers/specs/2026-11-24-nexora-full-parity-design.md` (M11 row: "Compatibility matrix, docs, benchmarks, release") and the goal statement: *"docs se lekar website community wagra sab kuch"*.

## Global Constraints

- Rust stable, edition 2024, pinned dependencies; `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` stay green.
- No `unwrap`, `expect` or `panic!` in production paths that can fail at runtime.
- Dependencies respect their licences; attribution is recorded.
- Large downloads and installs go to the E drive.
- Documentation layout follows ADR-021: new docs linked from `docs/README.md`.
- Every policy statement must be true of this repository: no promise the project cannot keep (no SLA, no response-time guarantee, no "production-grade" claim).

## Review Focus

1. A security reporter must find a working, private reporting path — a public issue tracker must never be the way to report a vulnerability, and the policy must say so explicitly.
2. The code of conduct must name an enforcement contact and a path to report, not just link to an external document.
3. The issue templates must not ask for secrets, tokens or logs containing credentials.
4. The site build must fail the CI job when a link in the book is broken, and the deploy must publish the same book the repository documents.
5. Every chapter in the site must exist in the repository — the site must not carry content that only lives in the built output.

---

### Task 1: security policy

**Files:**
- Create: `SECURITY.md`
- Test: none (documentation; verified by a link check in Task 5)

**Interfaces:**
- Consumes: `docs/SECURITY_REVIEW.md` (the existing risk review) and the project's supported-version policy.
- Produces: the root `SECURITY.md` GitHub reads for the "Security" tab.

- [ ] **Step 1: Write `SECURITY.md`**

Required sections, in order: **Supported versions** (a table with the 0.1.x line as supported and older lines as unsupported); **Reporting a vulnerability** (a private route — the maintainer's email from the git author identity, not a public issue, plus "do not open a public issue for a vulnerability"); **What to include** (affected component, reproduction steps, impact, and the words "no secrets, tokens or real user data — redact before sending"); **Response** (an acknowledgement is best effort, no SLA is promised, fixes ship in the next patch release); **Disclosure** (the reporter is credited unless they ask otherwise).

- [ ] **Step 2: Verify the file is linked and truthful**

Run: `grep -n "Reporting a vulnerability" SECURITY.md` and confirm the reporting address is the maintainer's, and that `docs/SECURITY_REVIEW.md` does not contradict it.
Expected: both checks print the expected lines.

- [ ] **Step 3: Commit**

```bash
git add SECURITY.md
git commit -m "docs: add the security policy"
```

---

### Task 2: code of conduct and governance note

**Files:**
- Create: `CODE_OF_CONDUCT.md`
- Test: none (documentation)

**Interfaces:**
- Consumes: the same maintainer contact as `SECURITY.md`.
- Produces: the root `CODE_OF_CONDUCT.md` GitHub reads for the community tab.

- [ ] **Step 1: Write `CODE_OF_CONDUCT.md`**

Required sections: **Our pledge**; **Expected behaviour** (behaviour that keeps a Discord-bot project welcoming: no harassment, no sexualised language, no personal attacks, respect differing viewpoints, accept constructive critique gracefully); **Unacceptable behaviour**; **Reporting** (the same maintainer contact, plus "reports are handled confidentially and reviewed by the maintainers"); **Enforcement** (a ladder: correction, warning, temporary ban, permanent ban, with the maintainers as the enforcing body); **Scope** (project spaces: repository, issue tracker, discussions, and any voice channel the project runs).

- [ ] **Step 2: Commit**

```bash
git add CODE_OF_CONDUCT.md
git commit -m "docs: add the code of conduct"
```

---

### Task 3: issue and pull-request templates

**Files:**
- Create: `.github/ISSUE_TEMPLATE/bug_report.yml`, `.github/ISSUE_TEMPLATE/feature_request.yml`, `.github/ISSUE_TEMPLATE/config.yml`
- Create: `.github/PULL_REQUEST_TEMPLATE.md`
- Test: none (documentation; verified by the config file in Task 5)

**Interfaces:**
- Consumes: the repo's existing contributing rules (`docs/CONTRIBUTING.md`: Conventional Commits, baseline checks, ownership table).
- Produces: GitHub's issue and PR forms.

- [ ] **Step 1: Write the bug report form**

YAML form with: **What happened** (textarea, "the track, the endpoint or the command, and what you expected instead"), **Version** (input, "the `version` from `/nexora/info` or the commit sha"), **Environment** (dropdown: OS, plus "NEXORA version"), **Steps to reproduce** (textarea), **Logs** (textarea with the warning *"Redact tokens, passwords and personal data before pasting"* and a default of "none"), and a **Severity** dropdown (blocker, major, minor, question).

- [ ] **Step 2: Write the feature request form**

Fields: **Problem** (textarea, "what you cannot do today"), **Proposal** (textarea), **Alternatives considered** (textarea), **Scope check** (dropdown: "protocol/API surface", "audio pipeline", "sources", "filters", "voice", "docs and tooling", "other") and a line pointing at `docs/COMPATIBILITY.md` so a request can name a matrix row.

- [ ] **Step 3: Write `config.yml` and the PR template**

`config.yml`: blank issues enabled, contact links to `SECURITY.md` and `CODE_OF_CONDUCT.md`. PR template: a title convention line (`feat:`, `fix:`, `docs:`, `test:`, `chore:`), a checklist of the three baseline commands, a line asking for the compatibility-matrix row a change touches, and a line asking for the roadmap item id.

- [ ] **Step 4: Commit**

```bash
git add .github/ISSUE_TEMPLATE .github/PULL_REQUEST_TEMPLATE.md
git commit -m "docs: add issue and pull-request templates"
```

---

### Task 4: the documentation site

**Files:**
- Create: `book.toml`
- Create: `src/SUMMARY.md`
- Create: `src/index.md`
- Modify: `.github/workflows/ci.yml` (site build job)
- Test: `mdbook build` (the book must build)

**Interfaces:**
- Consumes: the `docs/` tree, `README.md`, `CHANGELOG.md`.
- Produces: a static site under `book/` (git-ignored), built in CI and published to GitHub Pages.

- [ ] **Step 1: Write `book.toml`**

`[book] title = "NEXORA"`, `description = "Universal audio infrastructure in Rust"`, `src = "docs"`, `output = "book"`, `language = "en"`. The book's source is the `docs/` directory itself, so the site and the repository share one copy of every chapter.

- [ ] **Step 2: Write `src/SUMMARY.md` and `src/index.md`**

`src/` must live where mdbook expects it: create `docs/src/SUMMARY.md` and `docs/src/index.md`, and point `book.toml`'s `src` at `docs/src`. The summary lists the chapters by section — Start, Architecture, API, Protocol, Compatibility, ADRs, Research, Contributing — and every entry must point at a file that exists in `docs/`.

`docs/src/index.md` is the landing page: what NEXORA is, the three commands to get running (quick start, the nexora compatibility matrix, the roadmap), and the "not yet" list (no voice sender, no WASM plugins, no commercial providers) so a first-time reader is not misled.

- [ ] **Step 3: Build the book locally**

Run: `mdbook build` (install with `cargo install mdbook` if missing — to the E drive per the install rule)
Expected: the command exits 0 and `book/` contains the rendered chapters.

- [ ] **Step 4: Add the CI job**

In `.github/workflows/ci.yml` add a `site` job on `ubuntu-latest`: checkout, install `mdbook` (pinned action `peaceiris/actions-mdbook@v2`), run `mdbook build`, and upload the `book/` directory as the Pages artifact. A second job publishes it to GitHub Pages on pushes to `main`. The build step must fail the job when a chapter listed in `SUMMARY.md` does not exist.

- [ ] **Step 5: Commit**

```bash
git add book.toml docs/src .github/workflows/ci.yml
git commit -m "docs: add the mdbook documentation site and its Pages workflow"
```

---

### Task 5: community links and release notes

**Files:**
- Modify: `README.md` (community section), `docs/README.md` (site link), `CHANGELOG.md` (release section)
- Create: `.github/ISSUE_TEMPLATE` link check job in `.github/workflows/ci.yml` (or a `scripts/check_links.py`)
- Test: the link check script

**Interfaces:**
- Consumes: `SECURITY.md`, `CODE_OF_CONDUCT.md`, the issue templates, the site.
- Produces: the entry points a visitor needs, and a CI check that keeps them alive.

- [ ] **Step 1: Write the link check**

Create `scripts/check_links.py`: walk the repository's markdown files, extract relative links, and fail when a target does not exist. Ignore `http://` and `https://` targets. The script exits non-zero and prints every broken link.

- [ ] **Step 2: Run the link check**

Run: `python scripts/check_links.py`
Expected: exit 0 after fixing every broken relative link it reports (the ADR-001 orphan duplicate is one such link — see the M0 ledger note).

- [ ] **Step 3: Add the community section to the README**

A "Community" section with: the documentation site link, the "Report a vulnerability" link to `SECURITY.md`, the "Code of conduct" link to `CODE_OF_CONDUCT.md`, the issue and PR links, and a line that the compatibility matrix is the project's honest compatibility statement.

- [ ] **Step 4: Wire the link check into CI**

Add a `links` job to `.github/workflows/ci.yml` that runs `python scripts/check_links.py` on every push and pull request.

- [ ] **Step 5: Commit**

```bash
git add scripts/check_links.py README.md docs/README.md CHANGELOG.md .github/workflows/ci.yml
git commit -m "docs: add community links and a repository link check"
```

---

## Self-review

**Spec coverage.** The goal names "docs se leker website community wagra sab kuch": Task 4 is the website (mdbook over the existing docs), Tasks 1-3 are the community files GitHub expects, Task 5 wires the entry points and keeps the links alive. The design doc's M11 row ("Compatibility matrix, docs, benchmarks, release") is covered by the matrix already in the repo plus this plan's site and release notes; benchmarks and the release workflow stay in M11's own scope.

**Step scan.** Every step names a file, its required sections or fields, and a command with an expected result. The policy prose is specified by its required sections and rules rather than written out, because the wording is the contributor's to word — the requirements are what must be present.

**Type consistency.** `SECURITY.md`, `CODE_OF_CONDUCT.md`, the issue forms, the PR template, `book.toml`, `docs/src/SUMMARY.md`, `docs/src/index.md` and `scripts/check_links.py` are each named once and referenced consistently by the later tasks.

**Review Focus.** All five lines have an owning task: private reporting path (Task 1), enforcement contact (Task 2), no secrets in templates (Task 3), CI fails on broken links and the Pages job publishes the built book (Tasks 4 and 5), no content that lives only in the build output (Task 4 uses `docs/src` + the existing `docs/` tree, with SUMMARY pointing only at existing files).

**Proportion.** Five tasks for a community-and-site milestone; the bulk of the work is files whose required shape is pinned rather than whose prose is dictated.
