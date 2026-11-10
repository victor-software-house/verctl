# Compare versions from a shallow pull request checkout

## Motivation

`verctl check --versions` compares each manifest to the merge-base of HEAD and the default branch, but it never fetches.
The pull request checkout of `actions/checkout` is a depth-1 fetch of `refs/pull/<n>/merge`.
It holds no `origin/<base>`, and HEAD has no parents.

The check fails there on every pull request with this error:

> cannot resolve the default branch (origin exists but origin/HEAD, origin/main, and origin/master are missing; fetch the default branch)

A repository whose shared `verify` adds the check, as ctl-core's `tasks/rust.toml` is about to, fails its Verify job on every pull request.
verctl's own CI passed only because its verify job checks out with `fetch-depth: 0`.

`verctl publish` already fetches the default branch history it needs.
The check does not.

Queue row: [VER-076][ver-076].
A patch: it changes behaviour only for runs that fail today, and adds no configuration key.

## Changes

1. Before comparing, `check --versions` fetches each of `VERCTL_DEFAULT_BRANCH`, the event payload's `repository.default_branch`, and `GITHUB_BASE_REF` that is set, when `origin/<branch>` is missing or the clone is shallow.
2. A shallow clone is fetched with its boundary undone, so the server also sends the parents of HEAD and the merge-base can be computed.
3. The default branch is fetched even when a pull request targets another branch, so the released baseline reads the branch a full clone would.
4. A full clone that has the ref fetches nothing, and a run outside Actions names no branch, so it fetches nothing.
5. The fetch uses `GITHUB_TOKEN` and `GITHUB_REPOSITORY` when both are set, as `publish` does, and `origin` otherwise.

## Impact

1. `src/git.rs`, `src/github.rs`, and `src/versions.rs`.
2. `src/instructions.md`, `.ctl/operator/instructions.md.jinja`, and `AGENTS.md`.

[ver-076]: ../../../../tasks.yaml
