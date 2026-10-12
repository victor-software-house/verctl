## ADDED Requirements

### Requirement: The manifest selects a built-in ecosystem unless one is named

verctl SHALL treat a package whose `path` names `Cargo.toml` as `cargo` and one
whose `path` names `package.json` as `bun`, unless the package names an entry
of `ecosystems`. A `package.json` with no tracked `bun.lock` between its
directory and the repository root, and no named ecosystem, SHALL fail the load
and name `ecosystems`. Any other manifest SHALL name an ecosystem.

#### Scenario: A package.json without bun.lock

- **WHEN** package `web` at `package.json` has a tracked `pnpm-lock.yaml`, no `bun.lock`, and no `ecosystem`
- **THEN** `verctl status` exits non-zero and its error names `web` and `ecosystems`

### Requirement: A built-in block can be replaced per package

A package SHALL be able to replace its ecosystem's Lockfile or publish block
with a named step and keep every other block of its ecosystem.

#### Scenario: A crate that publishes through a mise task

- **WHEN** crate `demo` declares `publish: demo-publish` and `steps.demo-publish` is `{task: "release:demo"}`
- **THEN** `verctl version` still bumps `Cargo.toml` and runs `cargo update --workspace`
- **AND** `verctl release` runs `mise run --raw release:demo` instead of `cargo publish`, then queries crates.io for `demo` at the new version

### Requirement: Steps run as mise tasks or argv with declared effects

A step SHALL declare exactly one of `task` or `run`. verctl SHALL run a `task`
step as `mise run --raw <task>` and a `run` step as its argv, pass
`VERCTL_PLAN` in its environment, and, for a step that runs for a package,
also `VERCTL_PACKAGE`, `VERCTL_VERSION`, `VERCTL_PREVIOUS_VERSION`,
`VERCTL_TAG`, and `VERCTL_RELEASE_COMMIT`. It SHALL treat a non-zero exit as
failure. A step SHALL change only the paths in its `writes`.

#### Scenario: A version step writes an undeclared file

- **WHEN** step `version-file` declares `writes: [src/version.rs]` and also changes `README.md`
- **THEN** `verctl version` exits non-zero and names `README.md` and `version-file`

#### Scenario: A verify step fails

- **WHEN** `at.verify` lists step `smoke` and `smoke` exits 1
- **THEN** `verctl release` exits non-zero before any publish, tag, or Release

### Requirement: Steps after a release declare when they are done

A step listed under `released` SHALL declare `check`. verctl SHALL run the
check first and skip the step when the check exits zero.

#### Scenario: A rerun after an announcement

- **WHEN** `at.released` lists `announce`, whose `check` exits zero
- **THEN** `verctl release` reports `announce: already` and does not run it

### Requirement: Lockfiles are written only by their owners

verctl SHALL NOT write, patch, or rewrite any lockfile. `Cargo.lock` SHALL be
written by `cargo update --workspace`, `bun.lock` by
`bun install --lockfile-only`, `mise.lock` by `mise lock`, and any other
lockfile by a declared step that lists it in `writes`. verctl SHALL read
lockfiles only to check that they agree with the manifests, and SHALL compare
a lockfile's parsed entries, never its bytes.

#### Scenario: A lock written by another mise version

- **WHEN** `mise.dev.lock` lists the same two `rust` entries as the lock `check` expects, in the opposite order
- **THEN** `verctl check` reports no lockfile problem

#### Scenario: A stale lockfile after a declared step

- **WHEN** the Lockfile step of a declared ecosystem exits zero but `uv.lock` still names the old version
- **THEN** `verctl version` exits non-zero and names `uv.lock`

### Requirement: Cargo is a built-in

For a `cargo` package, verctl SHALL write `package.version` or
`workspace.package.version`, find dependents through `cargo metadata --no-deps`,
refresh a tracked `Cargo.lock` with `cargo update --workspace`, order publishes
over normal and build dependencies, decide "published" from the registry's
sparse index, and publish with `cargo publish --locked -p <name>` one package at
a time. A package with `publish = false` SHALL never be published.

#### Scenario: Publish order in a workspace

- **WHEN** `unixd` has a normal dependency on `unixd-cache` and both move
- **THEN** `verctl release --dry-run` lists `unixd-cache` before `unixd`, whatever order `packages` declares

#### Scenario: A package that does not publish

- **WHEN** crate `xtask` declares `publish = false` and a fragment bumps it
- **THEN** `verctl release` does not publish `xtask` and reports it as `not published`

### Requirement: Bun is a built-in, and verctl owns the registry protocol

For a `bun` package, verctl SHALL write `version`, find dependents through
`workspace:` dependencies, refresh a tracked `bun.lock` with
`bun install --lockfile-only`, refuse a Bun older than 1.4.1, pack with
`bun pm pack`, inspect the packed `package.json`, and publish that tarball
through its own npm registry client after querying the packument. A package
with `"private": true` SHALL never be published.

#### Scenario: A packed package still names a protocol

- **WHEN** the packed `package.json` of `@demo/b` holds `"@demo/a": "workspace:*"`
- **THEN** `verctl release` exits non-zero before any registry request and names `@demo/b` and `@demo/a`

#### Scenario: A packed package names a stale version

- **WHEN** `@demo/a` is `1.1.0` in its manifest and the packed `package.json` of `@demo/b` holds `"@demo/a": "1.0.0"`
- **THEN** `verctl release` exits non-zero before any registry request and names `bun.lock`

#### Scenario: An old Bun

- **WHEN** `bun --version` prints `1.4.0`
- **THEN** `verctl version` exits non-zero before writing any file and names `1.4.1`

### Requirement: A repository's own tool pin moves after its release exists

The built-in `mise-pin` step SHALL run only after `finalize`, SHALL move the pin
with `mise lock <tool>@<version>` under `MISE_LOCKFILE_MODE=generate`, and
SHALL then read `mise.lock` and fail unless every platform the lock lists has a
URL and a checksum for the new version.

#### Scenario: The pin moves after finalize

- **WHEN** `at.released` lists `mise-pin` for tool `github:victor-software-house/verctl` and `v0.4.1` has just been finalized
- **THEN** `verctl release` runs `mise lock github:victor-software-house/verctl@0.4.1` with `MISE_LOCKFILE_MODE=generate` after the Release is published

#### Scenario: A platform has no asset

- **WHEN** `mise.lock` lists `linux-x64` and `macos-arm64`, and the `v0.4.1` Release has no `macos-arm64` asset
- **THEN** `mise-pin` exits non-zero and names `macos-arm64`, though `mise lock` exited 0

### Requirement: verctl performs the OIDC exchange when asked

When a registry's `auth` is `oidc`, verctl SHALL exchange the GitHub Actions
OIDC token for a registry token itself: once per run for crates.io, and once
per package for npm. When `auth` is `token`, it SHALL read the token from the
registry's documented environment variable.

#### Scenario: Two npm packages

- **WHEN** `@demo/a` and `@demo/b` publish to `npm` with `auth: oidc`
- **THEN** `verctl release` performs two exchanges, each naming its own package

#### Scenario: No OIDC available

- **WHEN** `auth` is `oidc` and `ACTIONS_ID_TOKEN_REQUEST_URL` is unset
- **THEN** `verctl release` exits non-zero before any side effect and names `id-token: write`
