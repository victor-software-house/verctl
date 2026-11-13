---
verctl: patch!
verctl:group: changed
---

A Release with a declared asset target stays a draft until every asset is uploaded, so a repository can turn on GitHub immutable Releases.  
`verctl release` creates the tag, then the draft, and a later `verctl release` at that commit publishes it once every asset is uploaded, and otherwise reports `draft <url> lacks <names>`.  
A draft is incomplete.  
A published Release that lacks a declared asset stays pending for that target only, unless GitHub holds it immutable: it then counts complete, and `status` and `release` warn that a later version must ship the asset.  
The reusable `release.yml` gains a `finalize` job, and its `plan` job needs `contents: write` to list drafts, so a caller grants `contents: write` to the whole called workflow.
