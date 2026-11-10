# Design

## Context

verctl keeps no model of a release. `publish` decides "this commit is a
release" from a CHANGELOG heading and default-branch ancestry. Both stay true
for every later commit, so a rerun can tag a commit that was never published.
This change derives each release's state from facts on every run and makes
each transition guarded and repeatable.

This change is one part of [release-state-machine][rsm], which holds the
goals, the decisions, and the migration every part shares. How a release
completes is [release-completion][rc].

## 1. Facts

verctl stores no release state. It observes these facts on every run and
derives everything else from them.

| Fact | Observed from |
|:--|:--|
| Intent | fragments under `.changeset/` on the commit being read |
| Declared version | each package's manifest at a commit |
| Release commit `R(p, v)` | the first-parent history of the release branch: the newest commit where package `p`'s resolved version became `v`, resolved at each commit from its manifest and, when inherited, the workspace root manifest |
| Lockfile agreement | the lockfile's entry for each workspace package equals its manifest version, read by verctl's parser |
| Published | the registry: the crates.io sparse index, the npm packument, or an extension's `published` check |
| Tagged | the remote tag for `p@v`, peeled to a commit |
| Released | the GitHub Release for that tag: draft or published |
| Assets | each declared asset on that Release with `state: uploaded` |

## 2. Stages

Each package has two independent axes.

The current release, for the version declared on the release branch:

```text
merged ──publish──▶ published ──tag──▶ tagged ──release──▶ drafted ──asset──▶ … ──finalize──▶ released
   │                                                          (no assets: tagged ──release──▶ released)
   └── never published and the registry needs a first publish ──▶ bootstrap (stop, operator acts)
```

The next release, for fragments not yet applied:

```text
none ──fragment lands──▶ pending ──propose──▶ proposed ──merge──▶ (current axis: merged)
                                    proposed ──main moves──▶ stale ──propose──▶ proposed
                                    proposed ──no fragment left──▶ stale ──propose──▶ none (PR closed)
```

The `conflict` stage stops every transition for a package until an operator
resolves it. It is entered when a tag peels to a commit other than `R(p, v)`,
or when a dependency that `p`'s published manifest keeps has a current release
that does not publish, so `publish` could never proceed. verctl never waits on
a fact that cannot arrive. The fragment rule in [version-pr-content][vpc] keeps the second case from
arising through pull requests that pass `check`. A package in `conflict` does
not block `propose`, so a Version PR that resolves it can still be opened. One
option is a new version of the dependency, whose release commit publishes
because the dependency publishes on the release branch (the load refuses the
state otherwise). The other is a new version of `p` that drops the dependency
or stops publishing.

## 3. Transitions

| Transition | Guard | Effect | Postcondition, checked after |
|:--|:--|:--|:--|
| `propose` | No open pull request on the proposal branch has a base other than `<base>`, checked before the version step runs. No package's current release is short of `released`, except one in `conflict` (a current release that does not publish and declares no assets is `released` at `merged`). A fragment is pending | run the version step on a tree at the release branch tip, commit it on the proposal branch, open or update the Version PR | the proposal branch passes `check` |
| `propose` (close) | no fragment is pending and a Version PR is open; a retargeted pull request is reported, never closed | close it with a comment naming why | no Version PR is open |
| `publish` | HEAD is `R(p, v)`. Every workspace dependency that `p`'s published manifest keeps and that the publish order puts before `p` is published (the load refuses a publishing package that depends that way on a package that does not publish). The `verify` steps pass. The registry does not list `p@v` | the package's publish block runs | the registry lists `p@v` |
| `tag` | every publishing package the tag names is published | create the tags at `R(p, v)` and push all ready tags in one atomic push | each remote tag peels to `R(p, v)` |
| `release` | the tag exists | create the GitHub Release as a draft when assets are declared, otherwise published; if one exists, update its body | the Release exists with the CHANGELOG section as its body |
| `asset` | the Release is a draft; the asset is not uploaded | build at `R(p, v)` and upload | the asset is `uploaded` |
| `finalize` | every declared asset is uploaded | publish the draft, then run the `released` steps | the Release is published and each `released` step's check passes |

Rules that hold across all transitions:

1. A transition whose postcondition already holds does nothing and reports
   `already`. A rerun on any later commit is therefore safe.
2. A failed transition stops the run. Nothing that depends on it is attempted.
   The next run starts from the facts.
3. `release` handles the oldest unreleased release commit first, then the
   next. It never reads "the release" from HEAD.
4. Publish happens before tag for a package with an unchecked upload, whose
   tag is the record that the upload ran. A package whose uploads all have a
   registry check is tagged first, in the run that merged the Version PR,
   because the registry, not the tag, records the upload. Also, a rerun at an
   older commit could not create the tag without `workflows: write`
   ([tag-at-merge](../archive/2026-10-24-tag-at-merge/design.md)).
5. verctl never decides one fact from another. A published version does not
   imply a tag, and a tag does not imply a Release.

## 4. Commands

| Command | Does | Replaces |
|:--|:--|:--|
| `status` | both axes per package, the next transition, and the versioned plan (`--format json`, `--github-output`) | `status`, `assets --github-output` |
| `check` | identity and content ([version-pr-content][vpc]), lockfile agreement, served files | `check`, `check --versions` |
| `version` | the version step on the working tree; reports what ran under `after` | `prepare` |
| `propose` | the version step on the release branch tip, then open, update, or close the Version PR | `prepare --pr` |
| `release` | reconcile the oldest unreleased release commit: verify, publish, tag, release, finalize | `publish` |
| `release asset <target>` | build and upload one asset | `assets --build --upload` |
| `release bootstrap <package>` | the operator-run first publication and trust setup, from bun-release | bun-release's bootstrap |
| `render` | re-render every served file at the versions on HEAD | `pin` |
| `ci` | plan the validation jobs | unchanged |
| `instructions` | print the installed contract | unchanged |

`--dry-run` on `version`, `propose`, and `release` reports the transitions and
steps it would run and changes nothing.

## 5. Workflows

Two workflows replace three.

1. `ci.yml` on `pull_request`: `verify` runs `verctl check`. No `labeled` or
   `unlabeled` types.
2. `release.yml` on push to the release branch and `workflow_dispatch`:
   1. `plan`: `verctl status --github-output` emits the release commit, the
      tags, and the asset matrix.
   2. `publish`: when a release is pending, check out the release commit and
      run `verctl release`.
   3. `assets`: one job per target; each checks out the release commit, runs
      `verctl release asset <target>`, and attests the archive.
   4. `finalize`: `verctl release` again, which publishes the draft and runs
      the `released` steps.
   5. `propose`: `verctl propose`, with a token whose pull request events start
      CI without approval.

A pull request opened with `GITHUB_TOKEN` has its CI runs held for approval,
so `propose` needs a GitHub App token. VER-044 decides whether verctl gets its
own App.

## 6. Risks

1. [The release commit walk reads history] → Workflows already fetch full
   history. The walk reads only the first-parent commits that touch the files
   that can hold the package's version (its manifest and, when the version is
   inherited at any point, the workspace root manifest), newest first,
   following renames. It resolves the version at each commit and stops at
   `R(p, v)` itself: the first commit whose first parent resolves a version
   other than `v`, or has no such file. Its length is therefore the life of the
   current version, for every package, tagged or not.

[rsm]: ../release-state-machine/design.md
[rc]: ../release-completion/design.md
[vpc]: ../version-pr-content/design.md
