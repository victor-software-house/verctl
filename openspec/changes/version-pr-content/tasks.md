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

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
