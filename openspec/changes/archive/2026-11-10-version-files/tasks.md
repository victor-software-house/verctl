# Tasks

## 1. Version sources

- [x] 1.1 Add the `pyproject` stock driver (`toml`, `project.version`).
  Refuse a version that `project.dynamic` lists, and infer `uv lock` from `uv.lock`.
  → verify: unit tests for a bump, a byte-for-byte round trip, and the dynamic refusal.
  Evidence, 2026-11-10: shipped in [#257][verctl#257]: `pyproject_changes_only_the_version` and `a_dynamic_python_version_fails_the_plan`.
- [x] 1.2 Add the `go.mod` driver format.
  Read and write the `// vX.Y.Z` module-line comment, and read `0.0.0` when there is none.
  Refuse a major that differs from the `/vN` path suffix.
  → verify: unit tests for each `go.mod` scenario in the spec.
  Evidence, 2026-11-10: shipped in [#259][verctl#259], with the unit tests in `src/gomod.rs` and three bump tests.
- [x] 1.3 Add the `VERSION` driver format: one version and a newline.
  Read a missing file as `0.0.0`, and have `prepare` create it.
  → verify: unit tests for the bump, the missing file, and extra text.
  → verify: an integration test that `check` leaves a missing file missing.
  Evidence, 2026-11-10: shipped in [#260][verctl#260], with the bump tests and `check_leaves_a_missing_version_file_missing`.
- [x] 1.4 Add `marker: true` and the marker driver format.
  Require exactly one marker line, and rewrite it in place.
  → verify: unit tests for `#`, `//`, and `<!-- -->` markers, no line, and two lines.
  Evidence, 2026-11-10: shipped in [#261][verctl#261]: `each_opener_marks_the_version`, `a_file_with_no_marker_fails`, `two_markers_fail_and_name_both_lines` and `a_marker_line_versions_a_single_file`.

## 3. Bundle

- [x] 3.1 Document the version sources in `verctl instructions`, the operator skill, `README.md`, and `docs/`, through their templates → verify: `operator_docs` passes in CI.
  Evidence, 2026-11-10: each of [#257][verctl#257] to [#261][verctl#261] changed the operator docs through their templates.
- [x] 3.2 Add a patch fragment → verify: `verctl check` passes on the branch.
  Evidence, 2026-11-10: four patch fragments wait for the next release: `pyproject-driver`, `gomod-driver`, `version-file` and `marker-line`.
- [x] 3.3 Push and read CI on the pushed commit → verify: `verify` passes; record the run here.
  Evidence, 2026-11-10: the stack merged whole; its top, [#262][verctl#262], passed verify in [run 42687387208][verctl-262-verify].

[verctl#257]: https://github.com/victor-software-house/verctl/pull/257
[verctl#259]: https://github.com/victor-software-house/verctl/pull/259
[verctl#260]: https://github.com/victor-software-house/verctl/pull/260
[verctl#261]: https://github.com/victor-software-house/verctl/pull/261
[verctl#262]: https://github.com/victor-software-house/verctl/pull/262
[verctl-262-verify]: https://github.com/victor-software-house/verctl/actions/runs/42687387208
