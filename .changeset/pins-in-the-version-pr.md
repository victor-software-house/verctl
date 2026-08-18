---
verctl: patch
---

`prepare` rewrites `[[pins]]` and puts them on the Version PR commit. The
tag names that commit, so the released tree carries pins that name the
release; a pin rewritten after publish can never reach the tree a consumer
fetches by tag or `?ref=`. Publish pushes nothing but the tag.

A pin that has to name an already-published tarball — the one a repo's own
release lane installs — is not a `[[pins]]` entry, and this repo's
`mise.release.toml` says so.
