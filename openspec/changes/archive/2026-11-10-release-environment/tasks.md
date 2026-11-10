# Tasks

## 1. Workflow

- [x] 1.1 Prove an empty `environment` runs a called job in no environment, and an environment secret reaches a called job through `secrets[<name>]` with `secrets: inherit`. Proof: throwaway workflow runs in this repository on 2026-11-05, since removed with their branch, environment, and deployment
- [x] 1.2 Add the `environment` input and the stored-name read to `release.yml`; proof: the PR's CI runs actionlint and the workflow policy
  Evidence, 2026-11-10: the `environment` input shipped in [#241][verctl#241]. [#272][verctl#272] replaced the stored-name read with the mapped `registry-token` secret.
- [x] 1.3 Release a caller from an environment; proof: a caller's publish job log shows the token exported
  Evidence, 2026-11-10: superseded: registry-token-secret task 1.3 carries the caller proof, with the token mapped by name.

## 2. Docs

- [x] 2.1 Update the instructions and their template, `README.md`, and `examples/workflows/release.yml`; proof: `operator_docs` passes in CI
  Evidence, 2026-11-10: docs shipped in [#241][verctl#241] and were rewritten for the mapped secret in [#272][verctl#272].
- [x] 2.2 Add a `patch` fragment; proof: `verctl check` in CI accepts it
  Evidence, 2026-11-10: the [#241][verctl#241] fragment passed `verctl check` and is in `CHANGELOG.md`.

[verctl#241]: https://github.com/victor-software-house/verctl/pull/241
[verctl#272]: https://github.com/victor-software-house/verctl/pull/272
