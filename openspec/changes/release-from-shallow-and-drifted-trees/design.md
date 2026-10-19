# Design

## Context

`verctl publish` walks history three times before it releases anything: the
ancestry proof against the default branch, the walk to the commit that set
each version, and the walk to a tag's commit. A shallow clone ends each walk
at its boundary. Since `ensure_origin_head`, verctl fetches the default
branch itself when `origin/<default>` is missing, with history, through a
remote that carries one refspec.

## 1. Unshallow at the entry points

`git::ensure_full_history(root, branch, fetch)` fetches
`+refs/heads/<branch>:refs/remotes/origin/<branch>` with
`gix::remote::fetch::Shallow::undo()` when the repository is shallow. It
reuses `fetch_one_branch`, so it sends the same credentials and the same
single refspec as the ancestry fetch; `fetch_one_branch` takes the shallow
mode as a parameter, and the ancestry fetch passes `Shallow::NoChange`.

It runs at the start of `publish` (after `--dry-run` returns) and of
`publish --plan`, which both walk history. It does not run inside `pending`,
because a read function that fetches hides a side effect, and the ancestry
proof runs before `pending`.

The branch comes from `VERCTL_DEFAULT_BRANCH`, the variable that already
turns on the ancestry fetch. Without it verctl has no branch it trusts, so it
does not fetch, and the shallow-clone errors name the variable.

A clone that is not shallow, or has no `origin`, is a no-op.

## 2. The 403 on Release creation

Creating a Release answers 403 "Resource not accessible by integration" when
the token lacks `workflows: write` and the tag's commit has a
`.github/workflows` tree that differs from the default branch; the response's
`X-Accepted-GitHub-Permissions` header reads
`contents=write; contents=write,workflows=write`. An Actions `GITHUB_TOKEN`
can never hold `workflows`.

`ensure_release_with` matches the typed `octocrab::Error::GitHub` status, not
the message text, and adds context that names the permission and the remedy.
Other failures keep their current message.

## 3. No workflow change

The fix in the workflow would mint a GitHub App token with
`workflows: write` for the publish job. The App the Version PR already uses
holds `contents: write`, `metadata: read`, and `pull_requests: write`, and no
`workflows`. Granting it widens what that App can change in every installed
repository, so it was an operator decision. The operator decided on
2026-10-19 that no App gets `workflows: write`: a registry-checked package is
tagged before its uploads instead, in the run that merged the Version PR
([tag-at-merge](../tag-at-merge/design.md), VER-073). The error and the docs
still carry the remedy for a package with an unchecked upload.
