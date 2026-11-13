# Tasks

## 1. First publish

- [ ] 1.1 `verctl release --new`, with the `Retry-After` wait. Proof: CI tests against recorded responses, and the shrub first publish of `shrub-process`.
- [ ] 1.2 `verctl trust` and `trust --dry-run`: list first, add only a missing entry, never edit another. Proof: CI tests against recorded `github_configs` responses for a missing, a matching and a foreign entry.
- [ ] 1.3 `check` fails a never-published standalone package at a version other than `0.0.0`. Proof: a CI test.

## 2. Workflow

- [ ] 2.1 The `publish-new` workflow input. Proof: the workflow's policy check and a dry run.

## 3. Documents

- [ ] 3.1 Update AGENTS.md, the instructions, the operator skill and the README for `release --new`, `trust` and `publish-new`. Proof: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` leaves no diff in CI.
