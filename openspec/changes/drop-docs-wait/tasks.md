# Tasks

## 1. Remove

- [x] 1.1 Remove the `docs` job and the `docs` input from `.github/workflows/release.yml`, and every reference to them in `self-release.yml` and `examples/`. Proof: `mise run lint:workflows` and `mise run lint:workflow-policy` pass, and `rg -n 'docs\.rs|needs\.docs|inputs\.docs' .github examples` finds no job or input.
- [x] 1.2 Remove the `docs` verb, `DocsArgs`, `src/docsrs.rs` and their report, presentation and tests. Proof: CI passes, and a test asserts `verctl docs` exits 2 as unknown.

## 2. Documents

- [x] 2.1 Update AGENTS.md, the instructions, the operator skill and the README. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [x] 2.2 A `patch!` fragment names the removed job, input and verb.
