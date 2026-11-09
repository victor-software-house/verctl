# Tasks

## 1. Version sources

- [ ] 1.1 Add the `pyproject` stock driver (`toml`, `project.version`), refuse a version that `project.dynamic` lists, and infer `uv lock` from `uv.lock` → verify: unit tests for a bump, a byte-for-byte round trip, and the dynamic refusal.
- [ ] 1.2 Add the `go.mod` driver format: read and write the `// vX.Y.Z` module-line comment, `0.0.0` with none, and refuse a major that differs from the `/vN` path suffix → verify: unit tests for each `go.mod` scenario in the spec.
- [ ] 1.3 Add the `VERSION` driver format: one version and a newline, a missing file as `0.0.0`, and `prepare` creating it → verify: unit tests for the bump, the missing file, and extra text; an integration test that `check` leaves a missing file missing.
- [ ] 1.4 Add `marker: true` and the marker driver format: exactly one marker line, rewritten in place → verify: unit tests for `#`, `//`, and `<!-- -->` markers, no line, and two lines.

## 2. Configuration boundary

- [ ] 2.1 Resolve a directory `path` in `Config::parse`, failing on two manifests → verify: config tests for `path: .` with `VERSION`, a single manifest, and two manifests.
- [ ] 2.2 Infer `publisher: none` for `VERSION`, marker, and `go.mod` packages (`go.mod` lands with 1.2, `VERSION` with 1.3, the marker with 1.4), and fail a `pyproject.toml` package with no publisher → verify: config and `publish --plan` tests.
- [ ] 2.3 Fail the load when two packages share a changelog → verify: a config test naming both packages and `changelog`.

## 3. Bundle

- [ ] 3.1 Document the version sources in `verctl instructions`, the operator skill, `README.md`, and `docs/`, through their templates → verify: `operator_docs` passes in CI.
- [ ] 3.2 Add a patch fragment → verify: `verctl check` passes on the branch.
- [ ] 3.3 Push and read CI on the pushed commit → verify: `verify` passes; record the run here.
