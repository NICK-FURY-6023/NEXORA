# Contributing to NEXORA

## Before opening a pull request

1. Read `README.md` and `docs/MASTER_AI_INSTRUCTIONS.md`.
2. Keep changes scoped to the current milestone.
3. Use safe Rust; document and test any `unsafe` with a safety argument.
4. Add tests for every implemented component.
5. Run the baseline checks:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

6. Update `docs/PROJECT_STATUS.md`, `CHANGELOG.md`, and relevant docs.
7. Never commit `.env`, credentials, tokens, or local databases.

## Architecture decisions

Important architecture changes must be recorded in `docs/adr/`. The initial decisions are summarized in `docs/adr/README.md`.

## Commit style

Use Conventional Commits: `feat:`, `fix:`, `docs:`, `test:`, `refactor:`, `chore:`, `perf:`, `ci:`.
