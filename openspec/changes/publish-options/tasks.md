# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host, not the authoring workstation.

## 1. The `publish` field

- [ ] 1.1 `PackageSpec.publish` reads a mapping as options and a list as the whole upload command, through a visitor.
  Verify: a config test loads `publish: {verify: false}`, `publish: [uv, publish]`, and `publish: []`.
  Another fails on `publish: fast` and names both shapes.
- [ ] 1.2 Every reader of the old `publish_argv` field goes through `PackageSpec::publish_command` → verify: `rg publish_argv src` finds nothing.

## 2. One resolved publisher

- [ ] 2.1 `Publisher::of(config, package, root)` resolves `ReleaseOnly`, `Cargo`, `Bun`, or `Command` in the order the design gives → verify: a table test with one case per resolution step.
- [ ] 2.2 `resolve`, `registry_check`, `uploads`, and `workspace_members` match on `Publisher`.
  Verify: the existing tests in `src/publisher.rs` pass with only the new root argument of `resolve`, and `rg 'name == "cargo"' src/publisher.rs` finds nothing.
- [ ] 2.3 The `check` question "does this package publish" calls the same resolution, and the manifest readers move to `src/opt_out.rs`.
  Verify: the moved reader tests pass unchanged.

## 3. Cargo options

- [ ] 3.1 `CargoPublish::read` names cargo's options, their types, and their defaults in one place.
  Verify: a unit test covers `verify` omitted, `true`, `false`, a wrong type, and an unknown key.
- [ ] 3.2 The load fails for an option the publisher does not have, with the four messages of design decision 2.
  Verify: a config test covers each scenario of "Options are checked against the publisher at load" and "A publisher with no options refuses every option".
- [ ] 3.3 `verify: false` appends `--no-verify` to the single-crate upload and to each member upload, and keeps the registry check.
  Verify: tests cover "A crate CI already packed", "Omitted", "Options keep the registry check", and "Options reach every member".
- [ ] 3.4 The plan lists such a package via `cargo, no verify` → verify: a dry-run test in `tests/publish.rs`.

## 4. Manifest opt-out

- [ ] 4.1 A package whose publisher was inferred resolves to `ReleaseOnly` when its manifest opts out.
  That covers a `Cargo.toml` with `publish = false` or `[]`, inherited or not, and a `package.json` with `"private": true`.
  Verify: tests cover the first three scenarios of "A manifest that opts out releases on GitHub only".
  A dry-run test lists a private crate.
- [ ] 4.2 A named `publisher` or a `publish` list is used as written → verify: a test for "A named publisher wins".
- [ ] 4.3 A `publish` mapping or a `registry` beside a manifest that opts out fails the plan.
  Verify: a test for "Options beside a manifest that opts out".

## 5. Bundle

- [ ] 5.1 `verctl instructions` and the operator skill, through their templates.
  The Publishers section covers what `publish` takes, cargo's options, and when to set `verify: false`.
  The release-only section covers the manifest opt-out.
  Verify: `operator_docs` passes in CI.
- [ ] 5.2 `README.md`, `examples/ver.yaml` with a live `publish: {verify: false}`, and the `AGENTS.md` rule on typed options → verify: `tests/examples.rs` passes in CI.
- [ ] 5.3 This repository's `.ctl/ver.yaml` sets `publish: {verify: false}` on `verctl`, with the reason in a comment → verify: `verctl check` passes in CI.
- [ ] 5.4 A patch fragment → verify: `verctl check` passes on the branch.

## 6. Proof

- [ ] 6.1 Push, and read CI on the pushed commit → verify: `verify` passes; record the run here.
- [ ] 6.2 The next verctl release runs no verify build in its publish job and still reads the crates.io index.
  Verify: the publish job log of that run, recorded here and on [VER-118](../../../tasks.yaml).
