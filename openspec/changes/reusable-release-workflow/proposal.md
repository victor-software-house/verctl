# Ship one reusable release workflow, with an idempotent publish

## Why

Every repository that releases with verctl carries two workflows of its own,
`version-pr.yml` and `publish.yml`, copied from [`examples/workflows/`][examples]
and then edited. The copies drift, and each fix lands once per repository:

1. [VER-062][ver-062] and [VER-063][ver-063] changed the same concurrency group
   and the same zizmor findings in this repository and in the examples, and the
   same fix then had to reach every consumer by hand.
2. The publish workflow decides a release from the event: a merged pull request
   whose head is `version-packages`. A merge that GitHub reports late, a
   rerun, or a manual dispatch on the wrong branch each needed its own clause
   ([#153][pr-153]).
3. `verctl publish` runs the publisher every time and decides "already
   published" from six substrings of its stderr. A rerun after a partial
   failure works only when the registry's wording matches.
4. The Version PR branch is the constant `version-packages`, the name
   Changesets uses, though verctl serves one Version PR per base branch.

The operator ruled on 2026-10-15: verctl owns the release workflows, as one
reusable workflow called from a thin per-repository caller, with every piece
optional and overridable, and its defaults owned by verctl. The minimal slice
B of [release-state-machine][rsm] is unpaused for it: an idempotent publish,
one flow on push to the default branch plus dispatch, and the derived branch.
The rest of that change stays paused.

Queue row: VER-066. This change references [release-state-machine][rsm] and
takes three of its decisions as a patch-sized slice on today's commands: the
registry decides "published" (its decision 9), a push to the release branch
triggers release, then propose (its decision 5), and the proposal branch is
`verctl/release/<base>` (its section 2.3). It does not rename commands, change
`.ctl/ver.yaml`, or derive release state beyond what the slice needs.

## What Changes

1. verctl ships `.github/workflows/release.yml` with `on: workflow_call`. Its
   inputs turn each part off or redirect it, and every default comes from
   verctl. A caller passes only what differs.
2. One flow: the caller triggers on push to the default branch and on
   `workflow_dispatch`. The workflow plans, publishes, builds missing assets,
   then proposes the next Version PR, in that order, and each job is safe to
   rerun.
3. `verctl publish` asks the registry before it runs a publisher, and checks
   the tag and the Release before it creates them. When everything already
   holds it reports `nothing to release` and exits zero.
4. `verctl publish` acts on the commit that set each version, found on the
   default branch's first-parent history, never on a later HEAD.
5. The Version PR branch becomes `verctl/release/<base>`.
6. `prepare --pr --auto-merge` enables auto-merge from Rust, so the workflow
   and the action share one implementation, and `auto-merge.sh` goes.
7. Each repository replaces `version-pr.yml` and `publish.yml` with one caller,
   rendered by the shared template.
8. A new `verctl docs` waits for docs.rs to build each crates.io crate
   released at a version commit, and fails when the build failed. verctl
   0.4.5 and qctl 0.5.4 both shipped with no docs.rs build, and nothing
   noticed.

## Capabilities

### New Capabilities

- `release-workflow`: the reusable workflow, its inputs and defaults, the one
  flow, and caller migration.
- `idempotent-publish`: registry query, tag and Release checks, the version
  commit, and `nothing to release`.
- `version-pr-branch`: the derived branch and what reads it.
- `docs-build`: the docs.rs build check after a crates.io release.

### Modified Capabilities

None. The `version-pr-auto-merge` change keeps its requirements; its script
moves into `prepare --pr`.

## Impact

1. Code: `src/publish.rs`, `src/publisher.rs`, `src/release.rs`,
   `src/versions.rs`, `src/git.rs`, `src/github.rs`, `src/cli.rs`, and a
   new `src/docs.rs`.
2. Workflows: `.github/workflows/release.yml` (new, reusable), this
   repository's own caller, `examples/workflows/`, and the removal of
   `version-pr.yml` and `publish.yml` here.
3. Actions: `actions/version-pr` calls `prepare --pr --auto-merge`;
   `auto-merge.sh` is deleted. The actions stay for hand-written workflows
   until [VER-043][ver-043].
4. Consumers: each replaces two workflows with one caller in one pull request,
   closes the open `version-packages` pull request, and deletes that branch.
5. Every step runs `mise run ver <verb>` with settings in `env:` only, which needs
   ctl-core's environment-bound flags (CTC-022) in the verctl release that
   ships this.

[examples]: ../../../examples/workflows/
[rsm]: ../release-state-machine/proposal.md
[ver-062]: https://github.com/victor-software-house/verctl/pull/153
[ver-063]: https://github.com/victor-software-house/verctl/pull/154
[pr-153]: https://github.com/victor-software-house/verctl/pull/153
[ver-043]: ../../../tasks.yaml
