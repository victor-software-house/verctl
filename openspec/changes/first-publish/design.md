# Design

The research note `docs/research/2026-11-13-atomic-releases.md` holds the evidence for each decision.
The operator made each decision on 2026-11-13.

## 1. `release --new`

**Decision.**
`verctl release --new` runs locally with the operator's token from `VERCTL_REGISTRY_TOKEN`.
It publishes the real code of each package the registry lacks, at the version the default branch holds, tags each standalone package, and runs `verctl trust`.
A member of a shared-version group gets no tag, because its tag belongs to the group's release.
The name follows the crates.io token scope `publish-new`, which a first publish needs.

**Alternative.**
`release --first`, close to Nx `--first-release`, or `release --unpublished`, cargo-release's flag.
They lost because every verctl release publishes unpublished versions, and the case this flag handles is a package the registry does not know.

**Override.**
None needed: a repository that never adds a crate never runs it.

## 2. `trust` adds the entry only when it is missing

**Decision.**
`verctl trust` lists each crate's entries, then creates this repository's entry only when none matches, and reports `already set` otherwise.
crates.io accepts duplicate entries, up to 5 per crate, so a create without the list would not be idempotent.
It never edits or deletes another entry.
`trust --dry-run` (`-n`) lists what it would add, as `release --dry-run` and `render --dry-run` preview.

It takes no provider argument: the repository comes from the `origin` remote, the workflow file and environment from the one workflow that calls the reusable `release.yml`.

**Alternative.**
A provider subcommand, as `npm trust github`, plus a `--force` that registers again.
It lost because npm asks the user to type the repository and workflow, which verctl reads from the repository, and because a forced create could only add a duplicate.
A GitLab remote can come later, since crates.io supports GitLab entries too.

**Override.**
None: the entry the command adds is the one the release workflow needs.

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
`release --new` and `trust` wait until `Retry-After` on a 429, then try again, so one run creates any number of crates.

## 5. A token in CI is opt-in

**Decision.**
The reusable `release.yml` takes a boolean input `publish-new`, defaulting to `false`, beside `publish` and `crates-io-trusted-publishing`.
With it and the `registry-token` secret, the workflow publishes a crate the registry lacks.
Trusted publishing stays the default.

**Alternative.**
Always publish a missing crate with the token when the secret is set.
It lost because a stored token then creates crates without anyone asking for it.

**Disable.**
Leave `publish-new` out; it defaults to `false`.
