# Proposal

## Why

The operator decided on 2026-11-02 that no repository releases on its own.
A release starts only when a person merges the Version PR.
verctl can still arm GitHub auto-merge on the Version PR from CI.
Ten caller workflows pass `auto-merge: true` to the reusable release workflow, so each of them publishes as soon as its Version PR's checks pass.

Queue row: VER-079.

## What Changes

1. `verctl prepare` loses `--auto-merge` and the `VERCTL_AUTO_MERGE` variable that binds to it.
2. `prepare --pr` no longer reads the base branch's rules or `VERCTL_JOB_TOKEN`, and never calls `enablePullRequestAutoMerge`.
3. The reusable [`release.yml`][release-yml] and [`actions/version-pr`][version-pr-action] lose their `auto-merge` input.
   A caller that still passes it fails when it moves to this release, because GitHub refuses an undeclared input.
4. verctl's own [`self-release.yml`][self-release] drops the `hold` job and the `VERCTL_RELEASE_HOLD` variable, which existed only to turn auto-merge off.
5. The instructions, the operator skill, the README, and `AGENTS.md` stop describing auto-merge and say that a person merges the Version PR.
## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `version-pr-auto-merge`: its three requirements are removed, and one requirement states that verctl never arms a Version PR's merge.

The open change `reusable-release-workflow` drops `auto-merge` from its declared inputs, so archiving it later does not bring the input back.

## Impact

1. Code: [`src/cli.rs`][cli], [`src/main.rs`][main], [`src/release.rs`][release], [`src/github.rs`][github], [`tests/status.rs`][status-tests].
2. Workflows and actions: [`release.yml`][release-yml], [`self-release.yml`][self-release], [`actions/version-pr/action.yml`][version-pr-action].
3. Docs: [`src/instructions.md`][instructions], [`skills/verctl/SKILL.md`][skill], their templates under `.ctl/`, [`README.md`][readme], [`AGENTS.md`][agents].
4. Callers: each repository that passes `auto-merge: true` drops it when it moves its workflow ref to this release.

[release-yml]: ../../../.github/workflows/release.yml
[self-release]: ../../../.github/workflows/self-release.yml
[version-pr-action]: ../../../actions/version-pr/action.yml
[cli]: ../../../src/cli.rs
[main]: ../../../src/main.rs
[release]: ../../../src/release.rs
[github]: ../../../src/github.rs
[status-tests]: ../../../tests/status.rs
[instructions]: ../../../src/instructions.md
[skill]: ../../../skills/verctl/SKILL.md
[readme]: ../../../README.md
[agents]: ../../../AGENTS.md
