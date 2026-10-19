# Tag at merge for registry-checked packages

## Why

A release whose first publish run does not finish is retried at its version
commit. By then a later merge may have changed `.github/workflows`, and
GitHub then refuses to create the tag and Release with 403 "Resource not
accessible by integration": the Releases API needs `workflows: write` for a
commit whose workflows differ from the default branch, and `GITHUB_TOKEN`
cannot hold it.

A consumer hit it on its first release. The crates.io rate limit for new
crates stopped two runs, two workflow pull requests merged before the
retry, and the third run uploaded every crate and then failed to create the
Release (run 39198937088). The operator finished it by hand with a token
that holds the permission.

The operator decided on 2026-10-19 against granting `workflows: write` to
any App, and for tagging at merge instead.

Queue row: VER-073.
A patch: it changes when `publish` creates a tag, and adds no configuration
key.

## What changes

1. `verctl publish` creates the tag and Release of a tag whose packages all
   have a registry check, or no upload, before any upload. In the run that
   merged the Version PR, HEAD is the default branch tip, so GitHub needs
   no workflows permission. A retry finds the tag and Release and creates
   neither.
2. A tag that names a package with an unchecked upload keeps "publish, then
   tag", because its tag is the only record that the upload ran.
3. `verctl docs` waits only for crates the crates.io index lists, since a
   tag no longer says that a crates.io upload happened.
4. The operator instructions and README describe the order.

This reverses "Publish precedes the tag" of release-state-machine (VER-011)
for registry-checked packages. That change's proposal, design, and spec are
updated to say so.

## Limit

A package with an unchecked upload that does real work still meets the 403
when a workflow change merges while its release is pending. No org package
has one today: every package is checked against crates.io, uses
`publisher: none`, or runs a no-op upload that moves to `publisher: none`.
When such a package appears, it needs a guard: a pull request check that
fails a change to `.github/workflows` while its release is pending.

## Impact

1. `src/publish.rs`, `src/docsrs.rs`, `src/pending.rs` (comment), and
   `src/github.rs` (the 403 remedy).
2. `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, and
   `README.md`.
3. `.github/workflows/release.yml` is unchanged: the publish job already runs
   in the merge run at the version commit.
