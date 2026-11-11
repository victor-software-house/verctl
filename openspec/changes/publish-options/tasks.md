# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host, not the authoring workstation.

## 1. The `publish` field

- [ ] 1.1 `PackageSpec.publish` reads a mapping as options and a list as the whole upload command, through a visitor → verify: config tests load `publish: {verify: false}`, `publish: [uv, publish]`, and `publish: []`, and `publish: fast` fails naming both shapes.
- [ ] 1.2 Every reader of the old `publish_argv` field goes through `PackageSpec::publish_command` → verify: `rg publish_argv src` finds nothing.

## 2. One resolved publisher

- [ ] 2.1 `Publisher::of(config, package, root)` resolves `ReleaseOnly`, `Cargo`, `Bun`, or `Command` in the order the design gives → verify: a table test with one case per resolution step.
- [ ] 2.2 `resolve`, `registry_check`, `uploads`, and `workspace_members` match on `Publisher` → verify: the existing tests in `src/publisher.rs` pass unchanged, and `rg 'name == "cargo"' src/publisher.rs` finds nothing.
- [ ] 2.3 `check`'s "does this package publish" calls the same resolution, and the manifest readers move beside it → verify: the existing `pull_request` tests for a changed `publish` key pass unchanged.

## 3. Cargo options

- [ ] 3.1 `CargoPublish::read` names cargo's options, their types, and their defaults in one place → verify: unit tests for `verify` omitted, `true`, `false`, a wrong type, and an unknown key.
- [ ] 3.2 The load fails for an option the publisher does not have, with the four messages of design decision 2 → verify: one config test per scenario of "Options are checked against the publisher at load".
- [ ] 3.3 `verify: false` appends `--no-verify` to the single-crate upload and to each member upload, and leaves the registry check in place → verify: tests for the scenarios "A crate CI already packed", "Omitted", "Options keep the registry check", and "Options reach every member".
- [ ] 3.4 The plan lists such a package via `cargo, no verify` → verify: a `publish::plan` test, and a presentation snapshot of the dry run.

## 4. Manifest opt-out

- [ ] 4.1 A `Cargo.toml` with `publish = false` or `[]`, inherited or not, and a `package.json` with `"private": true`, resolve to `ReleaseOnly` when the publisher was inferred → verify: tests for the first three scenarios of "A manifest that opts out releases on GitHub only".
- [ ] 4.2 A named `publisher` or a `publish` list is used as written → verify: a test for "A named publisher wins".
- [ ] 4.3 A `publish` mapping or a `registry` beside a manifest that opts out fails the plan → verify: a test for "Options beside a manifest that opts out".

## 5. Bundle

- [ ] 5.1 `verctl instructions` and the operator skill, through their templates: the Publishers section says what `publish` takes, lists cargo's options, and says when to set `verify: false`; the release-only section says a manifest that opts out needs no `publisher: none` → verify: `operator_docs` passes in CI.
- [ ] 5.2 `README.md`, `examples/ver.yaml` (a live `publish: {verify: false}`), and `AGENTS.md` (an option of one publisher is a typed option, never an argv fragment) → verify: `tests/examples.rs` passes in CI.
- [ ] 5.3 This repository's `.ctl/ver.yaml` sets `publish: {verify: false}` on `verctl`, with the reason in a comment → verify: `verctl check` passes in CI.
- [ ] 5.4 A patch fragment → verify: `verctl check` passes on the branch.

## 6. Proof

- [ ] 6.1 Push, and read CI on the pushed commit → verify: `verify` passes; record the run here.
- [ ] 6.2 The next verctl release shows no verify build in its publish job and still reads the crates.io index → verify: the publish job log of that run; record it here and on [VER-118](../../../tasks.yaml).
