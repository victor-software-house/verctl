# Tasks

The pull request's CI verifies every compile, format, lint, and test task.
This change is one part of [release-state-machine](../release-state-machine/tasks.md), which records the operator's decisions in its section 0 and releases 0.8.0 in its section 5.
Tasks keep the numbers they had there.
State notes are from the code on `main` at [cb1be28][main], read on 2026-11-09 (UTC).

## 2. The release state machine

- [ ] 2.1 The release commit is derived from first-parent history, and `status` reports both axes and the versioned plan; proof: the release-state scenarios for the release commit and for no labels pass
  - 2026-11-09, partly shipped: `publish` derives each version commit from first-parent history (3452801, 2026-08-17), and no label is read ([#188][pr188]). Remaining: `status` reporting both axes.
- [ ] 2.2 `release` and `release asset` run verify, publish, tag, release, asset, and finalize with their guards, and `released` steps after finalize; proof: the release-state transition scenarios and the release-completion draft scenarios pass against a local bare remote and mocked registry and GitHub APIs
  - 2026-11-09, partly shipped under `publish` and `assets`: registry, tag, Release, and asset steps that report `already` on a rerun, and a registry-checked package tagged before its uploads (`src/publish.rs:350`). Remaining: the `release` verb, one atomic tag push, a draft Release until every asset is uploaded (PR 228, closed, is the reusable start), finalize, and `released` steps.

[main]: https://github.com/victor-software-house/verctl/commit/cb1be28
[pr188]: https://github.com/victor-software-house/verctl/pull/188
