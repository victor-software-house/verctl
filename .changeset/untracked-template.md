---
verctl: patch
---

A template in the source tree that git does not track now fails the run,
naming the file, instead of rendering nothing and letting the release serve
the stale hand-authored version of that file. Adding a served file and
getting silence was the one place this lane failed open, and it did it once
per repo, at the moment somebody was learning the feature.

Only tracked templates still render, for the reason they always did:
`prepare` stages what it writes, so a template git does not carry would put
a served file on the tag with no source beside it, and `check` would have
nothing to compare against on a fresh clone. What changed is the answer to
one that is present anyway — commit it or delete it.

Two things stay silent, both deliberately. A template *outside* the source
tree is not verctl's to render, which is what keeps this crate's own
changelog templates out of the released set; it was never a claim to serve
anything. And a tree with no repository has no index to not carry a file,
and cannot serve anything by tag, so it renders nothing without a word.
