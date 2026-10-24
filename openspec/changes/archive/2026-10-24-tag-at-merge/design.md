# Design

## Context

`verctl publish` at a version commit uploads each package, then creates one
GitHub Release per tag through the Releases API with `target_commitish` set
to HEAD. GitHub checks `workflows: write` when that commit's
`.github/workflows` differs from the default branch at the time of the call.
When the tag already exists, `target_commitish` is unused.

`pending` decides completeness from facts: the registry lists each version,
and the tag, the Release, and every declared asset exist. For an upload with
no registry check, the tag stands in for the registry.

## 1. Which tags come first

A tag is created before the uploads when no package under it has an
unchecked upload: every upload has a crates.io index check or a `query`, or
the package has no upload (`publisher: none`). `tags_before_upload` decides
it from the same `publisher::uploads` the run executes, so the order and the
uploads cannot disagree.

For those tags the registry decides completeness, so a tag created first
claims nothing `pending` reads: a retry still finds the package incomplete
and uploads it. The existing tag is checked to name HEAD, which `pending`
already guarantees for the commit it hands to the run.

When GitHub refuses an early Release with 403 for a missing permission, the
uploads still run, and the run fails at the end naming the refusal, beside
any later failure. A rerun with a token that holds the permission then has
only the Release left to do. Any other early failure, such as an existing
tag that names another commit, stops the run before any upload.

A shared `v{version}` tag over a checked and an unchecked package follows
the unchecked one, because creating it early would mark that package
complete before it ran.

Alternative considered: a separate App with `contents` and `workflows`,
whose key only the publish job reads. It lost on the operator's decision:
it gives a release job write access to workflows in every installed
repository. A draft Release created early lost too: a draft has no tag
until it is published, so publishing it meets the same check.

## 2. docs.rs

`verctl docs` used the tag naming HEAD as proof that each crate was
uploaded. A tag created before a failed upload would start a wait of up to
an hour for crates docs.rs can never build. It now also requires the
crates.io index to list the version, which is the fact the tag stood for.

## 3. Override and disable path

There is no key. A package that wants "publish, then tag" declares an upload
without a check: `publish` or `publishers.<name>.argv` with an empty
`query`. That is also the disable path.

## 4. Residual race

The publish job of the merge run can start after another merge. If that
merge changed a workflow, the early Release still meets the 403. The window
is the plan job and the publish job's setup, against the hours between a
failed upload and its retry before.
