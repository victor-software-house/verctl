# Compare versions from a shallow pull request checkout

## Why

`verctl check --versions` compares each manifest to the merge-base of HEAD and the default branch, but it never fetches.
The pull request checkout of `actions/checkout` is a depth-1 fetch of `refs/pull/<n>/merge`: there is no `origin/<base>`, and HEAD has no parents.
The check fails there on every pull request with "cannot resolve the default branch (origin exists but origin/HEAD, origin/main, and origin/master are missing; fetch the default branch)".
A repository whose shared `verify` adds the check, as ctl-core's `tasks/rust.toml` is about to, fails its Verify job on every pull request.
verctl's own CI passed only because its verify job checks out with `fetch-depth: 0`.
`verctl publish` already fetches the default branch history it needs; the check does not.

Queue row: VER-076.
A patch: it changes behaviour only for runs that fail today, and adds no configuration key.

## What changes

1. Before comparing, `check --versions` fetches each of `VERCTL_DEFAULT_BRANCH` and `GITHUB_BASE_REF` that is set, when `origin/<branch>` is missing or the clone is shallow.
2. A shallow clone is fetched with its boundary undone, so the server also sends the parents of HEAD and the merge-base can be computed.
3. A full clone that has the ref fetches nothing, and a run outside Actions sets neither variable, so it fetches nothing.
4. The fetch uses `GITHUB_TOKEN` and `GITHUB_REPOSITORY` when both are set, as `publish` does, and `origin` otherwise.

## Impact

1. `src/git.rs` and `src/versions.rs`.
2. `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, and `AGENTS.md`.
