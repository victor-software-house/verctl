# Design

## Context

A release that publishes nothing, a release with assets, and a package's first
publication each end differently. verctl must not wait on a fact that
cannot arrive. This change states how each completes. It is one part of
[release-state-machine][rsm]; the stages and transitions it refers to are in
[derived-release-state][drs].

## 1. A release that does not publish

A package does not publish when it declares `publish = false` or
`"private": true`, or uses a declared ecosystem with no publish block. Whether
a release publishes is a fact of its release commit: it is read from the
manifest and `.ctl/ver.yaml` at `R(p, v)`, and nowhere else. A release that
does not publish has no `publish` transition. Without `assets`, it is complete
at `merged`: it gets no tag and no Release, and it counts as `released`; a tag
whose packages are all of this kind is never created. With `assets`, it goes
`merged ──tag──▶ tagged` and continues as above. Changesets likewise does not
tag private packages by default ([config][cs-private]); verctl still versions
them, because a fragment that names one is intent.

A change of that setting therefore applies from the next version. A version
merged while its package did not publish is never published, and a version
merged while it published is published even if the setting changes before the
release runs. The next version after a package starts to publish enters
`bootstrap` if the registry needs a first publish and `merged ──publish──▶`
otherwise.

[rsm]: ../release-state-machine/design.md
[drs]: ../derived-release-state/design.md
[cs-private]: https://github.com/changesets/changesets/blob/c73949ba7b3160a4aa5729223335c190de1528f8/docs/config-file-options.md#privatepackages-object-or-boolean
