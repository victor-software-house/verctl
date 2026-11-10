# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md).
That change records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 1. Blocks and built-ins

- [ ] 1.4 `version` replaces `prepare`, and the report field `next` becomes `after`; proof: the version-content version-step scenarios pass
  - 2026-11-09, not started: the verb is `prepare` (`src/cli.rs:40`).
  - 2026-11-09, in 0.8.0 through [command-renames](../command-renames/proposal.md) (VER-091), with `pin` renamed `render` and no change to what the version step does.
  - 2026-11-10, the verb and the field ship on `main` at [98efe62][head]. Open: the version-step scenarios, which 2.1 adds.

## 2. The version step and the content check

- [ ] 2.1 `version` runs each bumped manifest's lockfile owner and `prepare.after`, each command once, and commits the owner's lockfile. It reports the commands under `after` and consumes a fragment that bumps nothing. Proof: the version-step scenarios pass in `tests/prepare_stage.rs`
- [ ] 2.2 Parsers for `Cargo.lock`, `bun.lock`, `package-lock.json`, and `uv.lock` read each entry's name, version, and whether it is a workspace member. Proof: unit tests on real lock excerpts in `tests/testdata/` pass
- [ ] 2.3 `check --versions` on a Version PR compares HEAD with the merge-base: the write set, the plan, each manifest by replay, each lockfile, each CHANGELOG, and each pinned and served file. Proof: the version-content Version PR scenarios pass. Their fixtures are shaped like the `ctl-core` four-member, `shrub` ten-entry, and `qctl` pinned Version PRs.
- [ ] 2.4 `check` fails a pull request that is not a Version PR when it adds a CHANGELOG section. It fails any pull request that changes whether a package publishes without a fragment for it. Proof: the version-proposal `check` scenarios for both pass
- [ ] 2.5 The instructions, the operator skill, `README.md`, and the rustdoc describe the version step and the content check; proof: `cargo test operator_docs` passes

[head]: https://github.com/victor-software-house/verctl/commit/98efe62
[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
