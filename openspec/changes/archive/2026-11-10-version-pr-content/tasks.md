# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../../release-state-machine/tasks.md).
That change records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 1. Blocks and built-ins

- [x] 1.4 `version` replaces `prepare`, and the report field `next` becomes `after`; proof: the version-content version-step scenarios pass
  - 2026-11-09, not started: the verb is `prepare` (`src/cli.rs:40`).
  - 2026-11-09, in 0.8.0 through [command-renames](../../command-renames/proposal.md) (queue row [VER-091][ver-091]), with `pin` renamed `render` and no change to what the version step does.
  - 2026-11-10, the verb and the field ship on `main` at [98efe62][head]. Open: the version-step scenarios, which 2.1 adds.
  Evidence, 2026-11-10: [#275][verctl#275] renamed the verb, and the report field is `after`.

## 2. The version step and the content check

- [x] 2.1 `version` runs each bumped manifest's lockfile owner and `prepare.after`, each command once, and commits the owner's lockfile. It reports the commands under `after` and consumes a fragment that bumps nothing. Proof: the version-step scenarios pass in `tests/prepare_stage.rs`
  Evidence, 2026-11-10: shipped in [#282][verctl#282]: `the_version_step_refreshes_a_tracked_cargo_lock`, `prepare_after_that_repeats_the_lockfile_owner_runs_once` and `a_fragment_that_bumps_nothing_is_consumed` pass.
- [x] 2.2 Parsers for `Cargo.lock`, `bun.lock`, `package-lock.json`, and `uv.lock` read each entry's name, version, and whether it is a workspace member. Proof: unit tests on real lock excerpts in `tests/testdata/` pass
  Evidence, 2026-11-10: shipped in [#283][verctl#283]: `tests/lockfile.rs` reads each lockfile excerpt in `tests/testdata/lockfiles/`.
- [x] 2.3 `check --versions` on a Version PR compares HEAD with the merge-base. It compares the write set, the plan, each manifest by replay, each lockfile, each CHANGELOG, and each pinned and served file. Proof: the version-content Version PR scenarios pass. Their fixtures are shaped like the `ctl-core` four-member, `shrub` ten-entry, and `qctl` pinned Version PRs.
  - 2026-11-10 (UTC), shipped in [#286][pr286] at [3802041][c3802041]: the 22 tests in `tests/content.rs` pass in its [CI verify run][run286]. A review there found that a changed symlink or submodule escaped the check. The fix and its two tests are in the pull request that carries 2.4.
- [x] 2.4 `check` fails a pull request that is not a Version PR when it adds a CHANGELOG section. It fails such a pull request when it changes whether a package publishes without a fragment that bumps it above `none`. Proof: the version-proposal `check` scenarios for both pass
  - 2026-11-10 (UTC), written: `src/pull_request.rs` and nine tests in `tests/versions.rs`, one per scenario plus the release-branch merge, the merge queue, and a workspace member. A version heading counts by its version, so fixing a name in one passes. Not yet verified in CI.
  - 2026-11-10 (UTC), shipped in [#288][pr288] at [940a21f][c940a21f]: the nine tests pass in its [CI verify run][run288]. Review on it found three gaps. The pull request above it fixes each with a test:
    - no merge-base to compare with;
    - a publish change that a merge of the release branch brings;
    - `publish.workspace` inherited from the workspace root.
- [x] 2.5 The instructions, the operator skill, `README.md`, and the rustdoc describe the version step and the content check; proof: `cargo test operator_docs` passes
  Evidence, 2026-11-10: the docs and rustdoc changed in [#282][verctl#282], [#286][verctl#286] and [#289][verctl#289]; `operator_docs` passed.

[c3802041]: https://github.com/victor-software-house/verctl/commit/3802041
[c940a21f]: https://github.com/victor-software-house/verctl/commit/940a21f
[head]: https://github.com/victor-software-house/verctl/commit/98efe62
[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[pr286]: https://github.com/victor-software-house/verctl/pull/286
[pr288]: https://github.com/victor-software-house/verctl/pull/288
[run286]: https://github.com/victor-software-house/verctl/actions/runs/42816191124/job/129830412432
[run288]: https://github.com/victor-software-house/verctl/actions/runs/42818410391/job/129837995816
[ver-091]: ../../../../tasks.yaml

[verctl#275]: https://github.com/victor-software-house/verctl/pull/275
[verctl#282]: https://github.com/victor-software-house/verctl/pull/282
[verctl#283]: https://github.com/victor-software-house/verctl/pull/283
[verctl#286]: https://github.com/victor-software-house/verctl/pull/286
[verctl#289]: https://github.com/victor-software-house/verctl/pull/289
