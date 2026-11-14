# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

## 1. `release bootstrap`

**Decision.**
`verctl release bootstrap [PACKAGE]...` runs locally with the operator's token from `VERCTL_REGISTRY_TOKEN`.
It publishes the real code of each named package the registry lacks, or of every such declared package when none is named, at the version the default branch holds.

It tags each standalone package, then runs `release trust`.
It runs at the default branch's tip, fetched first, and refuses any other HEAD, so the version it publishes is the one the default branch holds.
Run on its own, it also needs `VERCTL_OWNER_TOKEN` for its `release trust` step and checks both tokens before any upload, so no crate is created without its entry.
A member of a shared-version group gets no tag, because its tag belongs to the group's release.

It is a step under `release`, as `release asset <target>` is.
[`derived-release-state`][derived] already plans `release bootstrap <package>` for this, and [`release-completion`][completion] names the matching state `bootstrap`, so the three agree.

**Alternative.**
A flag, `release --new`, after the crates.io token scope `publish-new`.
It lost because it would add a second name for the step the state model already calls `bootstrap`, and a flag on `release` mixes a one-time act into every release.
`release --first`, close to Nx `--first-release`, and `release --unpublished`, cargo-release's flag, lost too: every verctl release publishes unpublished versions.

**Override.**
None needed: a repository that never adds a crate never runs it.

## 2. `release trust` adds only a missing entry

**Decision.**
`verctl release trust [PACKAGE]...` lists each crate's entries, then creates this repository's entry only when none matches, and reports `already set` otherwise.
crates.io accepts duplicate entries, up to 5 per crate, so a create without the list would not be idempotent.
It never edits or deletes another entry.
`release trust --dry-run` (`-n`) lists what it would add, as `release --dry-run` and `render --dry-run` preview.

It takes no provider argument: the repository comes from the `origin` remote and the workflow file from `release_workflow`, the key `check trust` reads ([`release-checks`][checks] section 3.2).
The entry names no environment, so it matches a run in any environment, as `check trust` compares it.
It reads the owner's token from `VERCTL_OWNER_TOKEN`: crates.io's create endpoint requires the `trusted-publishing` token scope, the scope `check trust` already uses, and a `publish-new` token cannot add an entry.
crates.io refuses a sixth entry with 409, so a crate at that limit fails naming it.

The step acts and the `trust` check of [`release-checks`][checks] proves, so both share the noun and differ in the verb: `check trust`, `release trust`.

**Alternative.**
A top-level `verctl trust`, or a provider subcommand, as `npm trust github`, plus a `--force` that registers again.

1. A top-level verb lost because the setup is a release step like `asset` and `bootstrap`, and would be a verb outside the command set `derived-release-state` plans.
2. A provider subcommand lost because npm takes the repository and workflow as typed arguments, which verctl reads from the repository.
3. `--force` lost because a forced create could only add a duplicate.

A GitLab remote can come later, since crates.io supports GitLab entries too.

**Override.**
None: the entry the step adds is the one the release workflow needs.

## 3. The first version is the one the default branch holds

**Decision.**
A standalone package first publishes at `0.0.0`, where verctl already starts it, and `check` fails a never-published standalone package at any other version.
A member of a shared-version group, such as a crate whose manifest sets `version.workspace = true`, first publishes at the group's current version.

**Alternative.**
Publish a placeholder to claim the name.
It lost because crates.io and npm forbid content that only reserves a name.

**Override.**
None: a package that should start elsewhere sets `first_minor: true` for its first Version PR.

## 4. Rate limits

Only a first publish meets the crates.io new-crate limit of 5, then 1 every 10 minutes.
`release bootstrap` and `release trust` wait on a 429 until the time crates.io gives, then try again, so one run creates any number of crates.
cargo prints that time after "try again after" as an HTTP date, which verctl already parses for a release's stop record; the trust endpoint sends it in `Retry-After`.
verctl parses the fixed HTTP date form itself, with no new dependency.

## 6. The `0.0.0` rule lives in `check registry`

Whether a package was ever published is registry state, so the rule that a never-published standalone package sits at `0.0.0` runs in `check registry`, the one check that reads crates.io, and bare `check` stays offline.

## 5. A token in CI is opt-in

**Decision.**
The reusable `release.yml` takes a boolean input `bootstrap`, defaulting to `false`, beside `publish` and `assets`.
With it and the `registry-token` secret, the release job runs `release --bootstrap`, which creates each package the registry lacks.
Trusted publishing stays the default.

**Alternative.**
`publish-new`, after the crates.io token scope.
It lost because each boolean input beside it names the job or step it runs, and the step is `bootstrap`.
Always publishing a missing crate with the token when the secret is set lost too, because a stored token then creates crates without anyone asking for it.

**Disable.**
Leave `bootstrap` out; it defaults to `false`.

[checks]: ../release-checks/design.md
[completion]: ../release-completion/proposal.md
[derived]: ../derived-release-state/proposal.md
