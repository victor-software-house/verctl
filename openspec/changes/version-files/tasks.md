# Tasks

## 1. Version sources

- [ ] 1.1 Add the `pyproject` stock driver (`toml`, `project.version`), refuse a version that `project.dynamic` lists, and infer `uv lock` from `uv.lock` → verify: unit tests for a bump, a byte-for-byte round trip, and the dynamic refusal.
- [ ] 1.2 Add the `go.mod` driver format: read and write the `// vX.Y.Z` module-line comment, `0.0.0` with none, and refuse a major that differs from the `/vN` path suffix → verify: unit tests for each `go.mod` scenario in the spec.
- [ ] 1.3 Add the `VERSION` driver format: one version and a newline, a missing file as `0.0.0`, and `prepare` creating it → verify: unit tests for the bump, the missing file, and extra text; an integration test that `check` leaves a missing file missing.
- [ ] 1.4 Add `marker: true` and the marker driver format: exactly one marker line, rewritten in place → verify: unit tests for `#`, `//`, and `<!-- -->` markers, no line, and two lines.

## 3. Bundle

- [ ] 3.1 Document the version sources in `verctl instructions`, the operator skill, `README.md`, and `docs/`, through their templates → verify: `operator_docs` passes in CI.
- [ ] 3.2 Add a patch fragment → verify: `verctl check` passes on the branch.
- [ ] 3.3 Push and read CI on the pushed commit → verify: `verify` passes; record the run here.
