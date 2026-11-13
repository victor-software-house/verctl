---
verctl: patch!
verctl:group: changed
---

A Release with a declared asset target stays a draft until every asset is uploaded, so a repository can turn on GitHub immutable Releases.  
`verctl release` creates the tag, then the draft, and a later `verctl release` at that commit publishes it once every asset is uploaded.  
It reports a draft that still lacks assets as a warning, `draft <url> lacks <names>`, and exits zero.  
A draft is incomplete.  
A published Release that lacks a declared asset stays pending for that target only, unless GitHub holds it immutable: it then counts complete, and `status` and `release` warn that a later version must ship the asset.  
`--skip-assets` also publishes an existing draft without its assets.

The reusable `release.yml` gains a `finalize` job, which runs `release --releases-only` after the assets.  
Its `plan` job needs `contents: write` to list drafts, so a caller grants `contents: write` to the whole called workflow.  
The plan hands each leg the assets it expects at that commit as `VERCTL_DECLARED_ASSETS`, which `release` reads as `--declared-assets`.

A hand-written workflow that runs `actions/publish` and then `actions/asset` must run `verctl release` again at the version commit after its uploads, which publishes the draft.  
Without that run the Release stays a draft.  
Draft publishing covers asset targets.  
A vendored set's Release is still created published, so immutable Releases are not yet safe for a repository with vendored sets.
