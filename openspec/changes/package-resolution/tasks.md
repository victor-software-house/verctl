# Tasks

## 2. Configuration boundary

- [ ] 2.1 Resolve a directory `path` in `Config::parse`, failing on two manifests → verify: config tests for `path: .` with `VERSION`, a single manifest, and two manifests.
- [ ] 2.2 Infer `publisher: none` for `VERSION`, marker, and `go.mod` packages (each lands with its format, tasks 1.2 to 1.4 of [`version-files`](../archive/2026-11-10-version-files/tasks.md)).
  Fail a `pyproject.toml` package with no publisher → verify: config and `publish --plan` tests.
- [ ] 2.3 Fail the load when two packages share a changelog → verify: a config test naming both packages and `changelog`.

## 3. Bundle

- [ ] 3.1 Document directory paths, publisher inference, and changelog collisions in `verctl instructions`, the operator skill, `README.md`, and `docs/`, through their templates → verify: `operator_docs` passes in CI.
- [ ] 3.2 Add a patch fragment → verify: `verctl check` passes on the branch.
- [ ] 3.3 Push and read CI on the pushed commit → verify: `verify` passes; record the run here.
