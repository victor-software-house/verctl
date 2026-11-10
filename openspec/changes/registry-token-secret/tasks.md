# Tasks

Compile, format, lint, and test tasks run in pull request CI with
`mise run verify`.

## 1. Workflow

- [x] 1.1 A secret held only in an environment, mapped by name under another key, reaches a called job in that environment. Proof: a throwaway caller and called workflow on a scratch branch
  - 2026-11-10 (UTC): [run 42780696593][run] read `named-token=match` in the environment job and `0 chars` in the job without one. The branch and environment were then deleted.
- [x] 1.2 `release.yml` reads only `secrets.registry-token`; proof: jactionlint 2.5.0 with the default profile reports no overprovisioned-secrets or obfuscation finding in `release.yml`
  - 2026-11-10 (UTC): both findings were on the publish step's `REGISTRY_TOKEN` line before the change and are gone after it. The remaining findings, missing-timeout and concurrency-limits, are on other lines.
- [ ] 1.3 A caller publishes from an environment with the token mapped by name; proof: that caller's publish job log shows the token exported

## 2. Bundle

- [x] 2.1 The instructions and their template, `README.md`, and `examples/workflows/release.yml` show the mapping. None tells a caller to pass `secrets: inherit`. Proof: `rg -n 'secrets: inherit' README.md src/instructions.md .ctl/operator examples` prints only lines that say never to pass it
- [x] 2.2 A `minor` fragment names the old call and the new one; proof: the fragment in the pull request
- [ ] 2.3 CI passes on the pull request: `operator_docs`, `lint:workflows`, and the workflow policy; proof: the verify run on the pushed commit

[run]: https://github.com/victor-software-house/verctl/actions/runs/42780696593
