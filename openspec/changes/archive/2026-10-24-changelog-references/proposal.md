# Link each changelog entry to its pull request

## Why

Every changelog entry verctl writes is a bare summary.
`release::changelog_sections` passes `pull_request: None` and `commit: None`
to the template, so no entry in CHANGELOG.md or a Release body names the
pull request that shipped it.
`ChangelogInput::with_author_filter` is defined and never called, and no
configuration key reads `internalAuthors`, so the author policy the operator
instructions document does nothing (verctl 0.3.14).

A consumer adopting verctl asked for a short `(#N)` on each entry and no
author credit. The house rule for release notes is the same: credit no
maintainer, and credit outside contributors only.

Queue row: VER-051.

## What changes

1. Each entry links the pull request that merged its fragment, written
   `([#N](url))`, or the commit that added it when no pull request is named.
2. No entry carries an author byline, so no maintainer is credited.
3. No configuration key yet. `Config` has only public fields, so a new
   field is a breaking library change for cargo-semver-checks, and this
   ships as a patch. `changelog.internal_authors`, which names the
   maintainers, waits for 0.4.0, when `Config` becomes `#[non_exhaustive]`
   (VER-054).

## Impact

1. `src/release.rs`, `src/git.rs`, `src/github.rs`, `src/main.rs`, and
   their tests. Every library change is additive.
2. The operator instructions describe the rule in place of the
   `internalAuthors` key, which never existed.
3. CHANGELOG.md and Release bodies of every consumer gain references on
   their next release.
