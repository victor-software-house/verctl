# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
Compile, format, lint, and test tasks that must run before a push run on the build host instead of the authoring workstation.

## 1. The `publish` field

- [x] 1.1 `PackageSpec.publish` reads a mapping as options and a list as the whole upload command, through a visitor.
  Verify: a config test loads `publish: {verify: false}`, `publish: [uv, publish]`, and `publish: []`.
  Another fails on `publish: fast` and names both shapes.
  Evidence: `config::tests::publish_reads_options_or_a_whole_command` and `publish_of_another_shape_names_both`.
- [x] 1.2 Every reader of the old `publish_argv` field goes through `PackageSpec::publish_command` → verify: `rg publish_argv src` finds nothing.
  Evidence: `rg publish_argv src` finds nothing.

## 2. One resolved publisher

- [x] 2.1 `Publisher::of(config, package, root)` resolves `ReleaseOnly`, `Cargo`, `Bun`, or `Command` in the order the design gives → verify: a table test with one case per resolution step.
  Evidence: `publisher::tests::each_resolution_step_picks_one_publisher` and `a_publishers_entry_with_its_own_argv_is_a_command`.
- [x] 2.2 `resolve`, `registry_check`, `uploads`, and `workspace_members` match on `Publisher`.
  Verify: the existing tests in `src/publisher.rs` pass with only the new root argument of `resolve`, and `rg 'name == "cargo"' src/publisher.rs` finds nothing.
  Evidence: `rg 'name == "cargo"' src/publisher.rs` finds nothing; the other publisher tests changed only the `resolve` root argument.
- [x] 2.3 The `check` question "does this package publish" calls the same resolution, and the manifest readers move to `src/opt_out.rs`.
  Verify: the moved reader tests pass unchanged.
  Evidence: The six reader tests moved unchanged to `src/opt_out.rs`.

## 3. Cargo options

- [x] 3.1 `CargoPublish::read` names cargo's options, their types, and their defaults in one place.
  Verify: a unit test covers `verify` omitted, `true`, `false`, a wrong type, and an unknown key.
  Evidence: `publisher::tests::cargo_verifies_unless_told_not_to`.
- [x] 3.2 The load fails for an option the publisher does not have, with the four messages of design decision 2.
  Verify: a config test covers each scenario of "Options are checked against the publisher at load" and "A publisher with no options refuses every option".
  Evidence: `config::tests::options_are_checked_against_the_publisher_at_load`.
- [x] 3.3 `verify: false` appends `--no-verify` to the single-crate upload and to each member upload, and keeps the registry check.
  Verify: tests cover "A crate CI already packed", "Omitted", "Options keep the registry check", and "Options reach every member".
  Evidence: `a_crate_ci_already_packed_skips_the_verify_build`, `options_keep_the_registry_check`, and `options_reach_every_member`.
- [x] 3.4 The plan lists such a package via `cargo, no verify` → verify: a dry-run test in `tests/publish.rs`.
  Evidence: `tests/publish.rs` `dry_run_lists_a_crate_that_skips_the_verify_build`.

## 4. Manifest opt-out

- [x] 4.1 A package whose publisher was inferred resolves to `ReleaseOnly` when its manifest opts out.
  That covers a `Cargo.toml` with `publish = false` or `[]`, inherited or not, and a `package.json` with `"private": true`.
  Verify: tests cover the first three scenarios of "A manifest that opts out releases on GitHub only".
  A dry-run test lists a private crate.
  Evidence: `each_resolution_step_picks_one_publisher` covers `publish = false`, `[]`, the inherited setting and a private `package.json`; `dry_run_releases_a_private_crate_on_github_only`.
- [x] 4.2 A named `publisher` or a `publish` list is used as written → verify: a test for "A named publisher wins".
  Evidence: The `named` row of `each_resolution_step_picks_one_publisher`.
- [x] 4.3 A `publish` mapping or a `registry` beside a manifest that opts out fails the plan.
  Verify: a test for "Options beside a manifest that opts out".
  Evidence: `options_beside_a_manifest_that_opts_out_fail_the_plan_only`, which also proves `check` reads the conflict without failing.

## 5. Bundle

- [x] 5.1 `verctl instructions` and the operator skill, through their templates.
  The Publishers section covers what `publish` takes, cargo's options, and when to set `verify: false`.
  The release-only section covers the manifest opt-out.
  Verify: `operator_docs` passes in CI.
  Evidence: `operator_docs` passed with the rest of the suite.
- [x] 5.2 `README.md`, `examples/ver.yaml` with a live `publish: {verify: false}`, and the `AGENTS.md` rule on typed options → verify: `tests/examples.rs` passes in CI.
  Evidence: `tests/examples.rs` passed with `--no-verify` in the expected argv.
- [x] 5.3 This repository's `.ctl/ver.yaml` sets `publish: {verify: false}` on `verctl`, with the reason in a comment → verify: `verctl check` passes in CI.
  Evidence: `.ctl/ver.yaml` sets it, with the reason in a comment.
- [x] 5.4 A patch fragment → verify: `verctl check` passes on the branch.
  Evidence: `.changeset/publish-options.md`.

## 6. Proof

- [ ] 6.1 Push, and read CI on the pushed commit → verify: `verify` passes; record the run here.
- [ ] 6.2 The next verctl release runs no verify build in its publish job and still reads the crates.io index.
  Verify: the publish job log of that run, recorded here and on [VER-118](../../../tasks.yaml).
