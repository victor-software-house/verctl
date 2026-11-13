# Tasks

## 1. First publish

- [ ] 1.1 `verctl release bootstrap [PACKAGE]...`, with the `Retry-After` wait. Proof: CI tests against recorded responses, and the shrub first publish of `shrub-process`.
- [ ] 1.2 `verctl release trust [PACKAGE]...` and `release trust --dry-run`: list first, add only a missing entry, never edit another. Proof: CI tests against recorded `github_configs` responses for a missing, a matching and a foreign entry.
- [ ] 1.3 `check` fails a never-published standalone package at a version other than `0.0.0`. Proof: a CI test.

## 2. Workflow

- [ ] 2.1 The `bootstrap` workflow input. Proof: the workflow's policy check and a dry run.

## 3. Documents

- [ ] 3.1 Update AGENTS.md, the instructions, the operator skill and the README for `release bootstrap`, `release trust` and the `bootstrap` input. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
- [ ] 3.2 A `patch!` fragment for verctl lists the stricter `check` under `### Breaking`. Proof: `verctl version --dry-run` shows the entry under that heading.
