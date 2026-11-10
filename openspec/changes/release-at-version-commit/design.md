# Design

## Context

`verctl publish` decides "this commit is a release" from a CHANGELOG heading
and default-branch ancestry, and creates the tag and the Release at HEAD. Both
stay true for every later commit, so a rerun can tag a commit that was never
published. This change publishes at the commit that set each version.

It is one of the parts [reusable-release-workflow][rrw] was split into. The
registry check it relies on is [registry-publish-check][rpc].

## 1. The version commit

A package's version commit is the newest commit where its manifest version
became the version it has at HEAD.
The search follows the first-parent history of the default branch.
`verctl publish --plan` finds, for each package, its version commit and
whether its release is complete: registry, tag, and Release, plus every
declared asset unless assets are off.

The workflow passes `assets: false` as `VERCTL_SKIP_ASSETS=true`
(`--skip-assets`), so a repository that turns assets off still reaches
`nothing to release`.
It reports every incomplete version commit, oldest first, or nothing.

A run that released only the oldest commit would leave a newer one waiting.
`propose` pushes `verctl/release/<base>` and does not push the default branch,
so no new run would start until an unrelated push.
The `publish` matrix therefore covers every pending commit in one run.

`verctl publish` refuses to run unless HEAD is a version commit of at least
one package whose release is incomplete.
It publishes, tags, and releases only the packages whose version commit is
HEAD.

Every other incomplete package is left for the run at its own version commit,
and the report names it.
The tag it creates is at HEAD, which is then that package's version commit.
So a run on a later commit can no longer tag a commit that was never published.

An existing tag names the release commit when it sits on another commit that
HEAD contains.
The package has the same version at that commit.
Three cases fit that rule:

- A publish rerun after later commits tagged a newer HEAD that way.
- A version set again after a revert keeps the tag from its first release.
- A branch rewritten and merged back with its old tip as the second parent
  keeps the tagged commit only on that parent.
  A forkctl append publish does this.

In each case the registry already has the version, so that commit is the
release and its assets build there.
A tag on a commit with another version, or on a commit HEAD does not contain,
fails the plan.
That release was tagged in the wrong place, and counting it complete would hide
it.

Alternative: dispatch with a `commit` input to resume. Lost: a missed run would then
need a person to find the commit.
An example is a pending run replaced in its concurrency group.
The plan job finds it on the next push.

## 2. Nothing to release

When every package's release at HEAD's versions is complete, `verctl publish`
and `verctl publish --plan` report `nothing to release` and exit zero. The
JSON report carries `"outcome": "nothing-to-release"`, or `"released"` when
the run changed anything.

[rrw]: ../reusable-release-workflow/design.md
[rpc]: ../registry-publish-check/design.md
