# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made the decisions on 2026-11-13 and 2026-11-14.

## 1. `release --new`

**Decision.**
`verctl release --new` (`VERCTL_NEW`) lets a release create each crate crates.io lacks, with the registry token from `VERCTL_REGISTRY_TOKEN`.
Every other upload uses the trusted-publishing token when it is set, and the registry token otherwise.
The release publishes the real code of a crate at the version the default branch holds.
It runs locally and in CI alike, and everything else is the release: its proofs, its draft Release, and the tag that the draft's publish creates.

The operator chose on 2026-11-13 that a first publish and `trust` are two atomic operations.
The first was `release --first`, renamed `release --new` with the workflow input `publish-new` the same day (operator, 2026-11-13).

**Alternative.**
A separate step with its own upload, tag and wait code, which `release bootstrap` was for a day.
It lost because it did again what `release` does, under a second name (operator, 2026-11-14).
`release --first`, close to Nx `--first-release`, and `release --unpublished`, cargo-release's flag, lost on 2026-11-13: every verctl release publishes unpublished versions.

**Override.**
None needed: a repository that never adds a crate never passes the flag.

## 2. `release trust` adds only a missing entry

**Decision.**
`verctl release trust [CRATE]...` lists each crate's entries, then creates this repository's entry only when none matches, and reports `already set` otherwise.
crates.io accepts duplicate entries, up to 5 per crate, so a create without the list would not be idempotent.
It never edits or deletes another entry.
`release trust --dry-run` (`-n`) lists what it would add, as `release --dry-run` and `render --dry-run` preview.

It takes no provider argument: the repository comes from the `origin` remote and the workflow file from `release_workflow`, the key `check trust` reads ([`release-checks`][checks] section 3.2).
The entry names no environment, so it matches a run in any environment, as `check trust` compares it.
It reads the owner's token from `VERCTL_OWNER_TOKEN`: crates.io's create endpoint requires the `trusted-publishing` token scope, the scope `check trust` already uses, and the registry token cannot add an entry.
crates.io refuses a sixth entry with 409, so a crate at that limit fails naming it.

The step acts and the `trust` check of [`release-checks`][checks] proves, so both share the noun and differ in the verb: `check trust`, `release trust`.

**Alternative.**
A top-level `verctl trust`, or a provider subcommand, as `npm trust github`, plus a `--force` that registers again.

1. A top-level verb lost because the setup is a release step like `asset`, and would be a verb outside the command set `derived-release-state` plans.
2. A provider subcommand lost because npm takes the repository and workflow as typed arguments, which verctl reads from the repository.
3. `--force` lost because a forced create could only add a duplicate.

A GitLab remote can come later, since crates.io supports GitLab entries too.

**Override.**
None: the entry the step adds is the one the release workflow needs.

## 3. `release bootstrap` is the two operations in a row

**Decision.**
`verctl release bootstrap [CRATE]...` runs the `release --new` code path, then the `release trust` code path for each crate that run created.
It takes every option `release` takes, since it is a release.
With crate names, only those crates may be created, and another crate crates.io lacks fails the proof as it does without `--new`.
Several crates may be named, and an unknown name fails before any request, listing the known ones.

It needs `VERCTL_REGISTRY_TOKEN` and `VERCTL_OWNER_TOKEN`, both checked before any write, so no crate is created without its entry.
`--dry-run` needs neither and prints what both steps would do.
It has no upload, tag or tip check of its own: the release's proofs apply, and the tag comes from the draft's publish.

[`derived-release-state`][derived] plans `release bootstrap` as the operator-run first publication and trust setup, and [`release-completion`][completion] names the matching state `bootstrap`, so the three agree.

**Alternative.**
No shorthand: the operator runs the two commands.
It lost because they are one act, and a crate created without its entry stays untrusted until someone remembers the second command (operator, 2026-11-14).

**Override.**
None needed: `release --new` and `release trust` run alone.

## 4. The first version is the one the default branch holds

**Decision.**
A standalone package first publishes at `0.0.0`, where verctl already starts it, and `check registry` fails a never-published standalone package at any other version.
A member of a shared-version group, such as a crate whose manifest sets `version.workspace = true`, first publishes at the group's current version.
Whether a package was ever published is registry state, so the rule runs in `check registry`, the one check that reads crates.io, and bare `check` stays offline.

**Alternative.**
Publish a placeholder to claim the name.
It lost because crates.io and npm forbid content that only reserves a name.

**Override.**
None: a package that should start elsewhere sets `first_minor: true` for its first Version PR.

## 5. Rate limits

Only a first publish meets the crates.io new-crate limit of 5, then 1 every 10 minutes.
A release that the limit stops fails with the time crates.io gives and records it in its draft, and a rerun resumes it, as it does after any stop.
A crate the stopped run created needs `release trust`, which a rerun of `release bootstrap` does not repeat.
`release trust` waits on a 429 until the `Retry-After` the endpoint sends, then tries again.
verctl parses the fixed HTTP date form itself, with no new dependency.

## 6. A token in CI is opt-in

**Decision.**
The reusable `release.yml` takes a boolean input `publish-new`, defaulting to `false`, beside `publish` and `assets`.
With it and the `registry-token` secret, the release job runs `release --new`, which creates each package the registry lacks.
Trusted publishing stays the default.

**Alternative.**
Always creating a missing crate with the token when the secret is set lost, because a stored token then creates crates without anyone asking for it.

**Disable.**
Leave `publish-new` out; it defaults to `false`.

[checks]: ../release-checks/design.md
[completion]: ../release-completion/proposal.md
[derived]: ../derived-release-state/proposal.md
