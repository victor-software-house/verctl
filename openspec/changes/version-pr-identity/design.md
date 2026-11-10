# Design

## Context

`check --versions` decides "this is the Version PR" from a branch name or a
label read from an event snapshot, which races its own label (VER-029). This
change identifies the Version PR by facts that cannot change. It is one part of
[release-state-machine][rsm]; what a Version PR may contain is
[version-pr-content][vpc].

## 1. What can identify a pull request

GitHub lets a pull request's title, body, base, and state change after it is
opened, but not its head branch or head repository: the update endpoint takes
no head ([GitHub REST][gh-pr-update]). Labels change too, and a label applied
after creation is missing from the `opened` event, which is the VER-029 race.

So the identity of a Version PR rests on the two facts that cannot drift, and
on the base read live on every run:

1. its head repository is the repository itself, not a fork;
2. its head branch is `verctl/release/<base>`;
3. its current base, read from the pull request at run time, is the release
   branch, and is the `<base>` its head branch names.

`propose` finds its open pull requests by head through the pull request list
endpoint, filtered as `<owner>:verctl/release/<base>`, then confirms each
match's head repository is this repository and reads its base. A pull request
on the proposal branch whose base was changed is not a Version PR: `propose`
exits non-zero before pushing to the proposal branch, names every such match
to close, and opens no second one. `check` reads all three from the API when it runs, never from the event
payload. Title, body, and labels are never read.

A version the release branch already carries is not a hand edit on another
base. `check` also reads each manifest at the merge-base of HEAD and the
release branch (`origin/<default>`), and accepts a version equal to that one.
Merging the release branch into `next` brings its released versions along,
and passes without any identity. Typing the same version without that history
still fails, because the merge-base with the release branch then predates the
release. A pull request into the release branch has one merge-base, so its
check is unchanged.

## 2. Identity and content are both required

The two rules `check` applies, identity and content, are stated in
[version-pr-content][vpc]. These alternatives to identity were considered:

1. **Label** (today). Lost: mutable by anyone with write access, and absent
   from the event that opens the PR.
2. **Title or body marker.** Lost: both are editable.
3. **Content only** (the first draft of this design). Lost to identity plus
   content: the operator asked for deterministic identification, and content
   alone would let a feature branch carry a release.
4. **Head branch alone** (`GITHUB_HEAD_REF`). Lost: a fork can use the same
   branch name, which is why VER-012 refused it. The head repository check
   closes that.

## 3. How many release branches, and their names

The proposal branch is `verctl/release/<base>`, for example
`verctl/release/main`. It is derived, not configured.

| Option | Gain | Cost | Verdict |
|:--|:--|:--|:--|
| One Version PR per base branch | one place to review a release; dependents and the shared lockfile stay in one diff | packages release together | **chosen** |
| One Version PR per package (release-please `separate-pull-requests`) | packages release on their own schedule | dependents span PRs that must merge in order; every PR rewrites the shared `Cargo.lock` or `bun.lock`, so each merge conflicts the others | rejected |
| Branch named after the version (`release/v0.4.0`) | the name says what ships | the version changes while the PR is open (a `minor` fragment turns `0.3.13` into `0.4.0`). A PR's head branch cannot be renamed, so each change would close the PR and open another, losing its review. With several packages there is no single version to name | rejected |
| Branch named after the base | deterministic; a maintenance base such as `release/0.3` gets its own PR without a rename | none found | **chosen** |

Changesets uses the same rule (`changeset-release/<base>`); release-please
names a branch per base and per component.

[rsm]: ../release-state-machine/design.md
[vpc]: ../version-pr-content/design.md
[gh-pr-update]: https://docs.github.com/en/rest/pulls/pulls#update-a-pull-request
