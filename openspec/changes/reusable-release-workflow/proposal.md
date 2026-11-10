# Ship one reusable release workflow

## Motivation

Every repository that releases with verctl carries two workflows of its own,
`version-pr.yml` and `publish.yml`, copied from [`examples/workflows/`][examples]
and then edited. The copies drift, and each fix lands once per repository:

1. [VER-062][ver-062] and [VER-063][ver-063] changed the same concurrency group
   and the same zizmor findings in this repository and in the examples.
   The same fix then had to reach every consumer by hand.
2. The publish workflow decides a release from the event: a merged pull request
   whose head is `version-packages`. A merge that GitHub reports late, a
   rerun, or a manual dispatch on the wrong branch each needed its own clause
   ([#153][pr-153]).

The operator ruled on 2026-10-15 that verctl owns the release workflows.
They are one reusable workflow called from a thin per-repository caller, with
every piece optional and overridable.
verctl owns the defaults.

The minimal slice B of [release-state-machine][rsm] is unpaused for it.
That slice is an idempotent publish, one flow on push to the default branch
plus dispatch, and the derived branch.
The rest of that change stays paused.

Queue rows: [VER-066][queue] and [VER-074][queue]. This change holds the workflow and its
callers. The rest of slice B is split into four changes, each with its own
proposal. They are [registry-publish-check][rpc],
[release-at-version-commit][ravc], [docs-build-check][docs], and
[derived-version-pr-branch][dvpb]. None renames a command or changes
`.ctl/ver.yaml`.

## Changes

1. verctl ships `.github/workflows/release.yml` with `on: workflow_call`. Its
   inputs turn each part off or redirect it, and every default comes from
   verctl. A caller passes only what differs.
2. One flow: the caller triggers on push to the default branch and on
   `workflow_dispatch`. The workflow plans, publishes, builds missing assets,
   then proposes the next Version PR, in that order, and each job is safe to
   rerun.
3. Each repository replaces `version-pr.yml` and `publish.yml` with one caller,
   rendered by the shared template.

## Capabilities

### New Capabilities

- `release-workflow`: the reusable workflow, its inputs and defaults, the one
  flow, and caller migration.

### Modified Capabilities

None.

## Impact

1. Workflows: `.github/workflows/release.yml` (new, reusable), this
   repository's own caller, `examples/workflows/`, and the removal of
   `version-pr.yml` and `publish.yml` here.
2. Consumers: each replaces two workflows with one caller in one pull request,
   closes the open `version-packages` pull request, and deletes that branch.
3. Every step runs `mise run ver <verb>`, on the verctl pinned in the task,
   with settings in `env:` only.
   That needs ctl-core's environment-bound flags (queue row
   [CTC-022][ctc-022]) in the verctl release that ships this.

[examples]: ../../../examples/workflows/
[rsm]: ../release-state-machine/proposal.md
[rpc]: ../registry-publish-check/proposal.md
[ravc]: ../release-at-version-commit/proposal.md
[docs]: ../docs-build-check/proposal.md
[dvpb]: ../derived-version-pr-branch/proposal.md
[queue]: ../../../tasks.yaml
[ctc-022]: https://github.com/victor-software-house/ctl-core/blob/main/tasks.yaml
[ver-062]: https://github.com/victor-software-house/verctl/pull/153
[ver-063]: https://github.com/victor-software-house/verctl/pull/154
[pr-153]: https://github.com/victor-software-house/verctl/pull/153
