# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md).
That change records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.

State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 1. Blocks and built-ins

- [ ] 1.2 Steps run as `mise run --raw` or argv with the `VERCTL_*` environment, `writes` enforced and `check` honoured.
  Declared ecosystems and per-package block overrides work.
  Proof: the building-blocks step scenarios pass.
  - 2026-11-09, not started. `prepare.after` with `stage` globs is the precursor (`src/config.rs:1038`).
- [ ] 1.3 verctl writes no lockfile; proof: `git grep` finds no write to a path ending in `.lock` in `src/`, and the lockfile ownership scenario passes
  - 2026-11-09, partly shipped: `src/` writes no lockfile outside tests. The lockfile ownership scenario needs steps (1.2).

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
