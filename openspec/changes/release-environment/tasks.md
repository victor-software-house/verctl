# Tasks

## 1. Workflow

- [x] 1.1 Prove an empty `environment` runs a called job in no environment, and an environment secret reaches a called job through `secrets[<name>]` with `secrets: inherit`. Proof: throwaway workflow runs in this repository on 2026-11-05, since removed with their branch, environment, and deployment
- [ ] 1.2 Add the `environment` input and the stored-name read to `release.yml`; proof: the PR's CI runs actionlint and the workflow policy
- [ ] 1.3 Release a caller from an environment; proof: a caller's publish job log shows the token exported

## 2. Docs

- [ ] 2.1 Update the instructions and their template, `README.md`, and `examples/workflows/release.yml`; proof: `operator_docs` passes in CI
- [ ] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
