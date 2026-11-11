# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The `prerelease` setting

- [ ] 1.1 `PackageSpec.prerelease: Option<Prerelease>`, with a `ctl_core::validate` rule for lowercase ASCII letters.
  Verify: a config test loads `prerelease: rc` and fails on `rc1`, `RC`, `rc.1`, and `1`, each naming the package and the field.

## 2. Candidate versions

- [ ] 2.1 `bump::apply` takes the package's `prerelease` and computes the base and the count as design decision 3 gives.
  Verify: a table test with one row per line of the design's table, plus the two 0.x and `0.0.0` refusals.
- [ ] 2.2 A current prerelease that is not `<letters>.<number>`, and a new tag that sorts below the current one, fail the plan.
  Verify: tests for `0.8.0-rc1` and for `0.8.0-rc.2` with `prerelease: beta`, each naming both versions.
- [ ] 2.3 `pending` finds a package pending with no fragment when its version is a prerelease and `prerelease` is omitted or names another tag.
  Verify: a `verctl version` integration test writes `0.8.0` from `0.8.0-rc.2` with an empty `.changeset/`, and `0.8.0-rc.1` from `0.8.0-beta.2`.

## 3. The stable section

- [ ] 3.1 A function returns the fragments each candidate since the package's last stable version commit consumed, read from first-parent history.
  Verify: a git test builds two candidate Version PR merge commits that each delete one fragment, and gets both fragments back.
- [ ] 3.2 `git::added_by` takes a start commit, so a deleted fragment's origin resolves.
  Verify: the existing `added_by` tests pass, and a new one resolves a fragment deleted at `HEAD`.
- [ ] 3.3 The stable Version PR renders its section from the recovered and pending fragments, and leaves the candidate sections in place.
  Verify: an integration test of the "Two candidates, then stable" scenario compares the whole CHANGELOG with an `indoc!` fixture.
- [ ] 3.4 A commit missing from a shallow checkout fails the plan before any file changes.
  Verify: a test on a shallow clone exits non-zero, names the commit, and leaves `Cargo.toml` and `CHANGELOG.md` unchanged.

## 4. Publish and release

- [ ] 4.1 The bun publisher adds `--tag <prerelease tag>` for a version with a prerelease. cargo and a replaced command are unchanged.
  Verify: `publisher` tests for a bun candidate, a cargo candidate, and a command candidate, comparing whole argv.
- [ ] 4.2 `ensure_release` creates a candidate's Release with `prerelease: true` and `make_latest: false`, and a stable one with neither field changed.
  Verify: two wiremock tests in `src/github.rs` match the request body.

## 5. Pins

- [ ] 5.1 The pin version pattern matches a whole semver version, prerelease and build included. A tool pin's ref uses the same pattern.
  Verify: `a_prerelease_pin_fails_instead_of_half_rewriting` and `an_include_with_a_prerelease_ref_fails_instead_of_half_rewriting` become tests that the version moves.
  A new test keeps the failure for `?ref=v1.2.3_local`.

## 6. Bundle

- [ ] 6.1 `verctl instructions`, the operator skill, `README.md`, and `examples/ver.yaml` describe `prerelease`, through their templates.
  Verify: `UPDATE_OPERATOR_DOCS=1 cargo test operator_docs` on the build host leaves no diff on a second run, and the prose lint passes.
- [ ] 6.2 A `.changeset/` fragment for the patch release.
  Verify: `verctl check` in CI passes on the pull request.
- [ ] 6.3 At archive, the `pin-patterns` purpose drops "how a prerelease stops a release".
  Verify: `openspec validate --specs --strict` passes after the archive.

## 7. First use

- [ ] 7.1 After 0.8.1 releases with this change, verctl's own package sets `prerelease: rc`, and the next patch fragment releases `0.8.2-rc.1`.
  Verify: the release run's log shows a crates.io upload of `0.8.2-rc.1`, and `gh release view v0.8.2-rc.1 --json isPrerelease` reads `true`.
- [ ] 7.2 One caller pins `0.8.2-rc.1` and runs its release workflow green.
  Verify: the caller's run link, recorded here.
- [ ] 7.3 verctl removes the key and releases `0.8.2`, whose CHANGELOG section lists every candidate's entries.
  Verify: `gh release view --json tagName` with no tag reads `v0.8.2`.
