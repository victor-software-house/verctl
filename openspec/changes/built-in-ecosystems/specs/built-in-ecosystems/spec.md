## ADDED Requirements

### Requirement: Cargo is a built-in

For a `cargo` package, verctl SHALL do all of these:

- write `package.version` or `workspace.package.version`;
- find dependents through `cargo metadata --no-deps`;
- refresh a tracked `Cargo.lock` with `cargo update --workspace`;
- order publishes over the dependencies that count above;
- decide "published" from the registry's sparse index;
- publish with `cargo publish --locked -p <name>` one package at a time.

A package with `publish = false` SHALL never be published.

#### Scenario: Publish order in a workspace

- **WHEN** `unixd` has a normal dependency on `unixd-cache` and both move
- **THEN** `verctl release --dry-run` lists `unixd-cache` before `unixd`, whatever order `packages` declares

#### Scenario: A package that does not publish

- **WHEN** crate `xtask` declares `publish = false` and a fragment bumps it
- **THEN** `verctl release` does not publish `xtask` and reports it as `not published`

### Requirement: Bun is a built-in, and verctl owns the registry protocol

For a `bun` package, verctl SHALL write `version`, find dependents through
`workspace:` dependencies, refresh a tracked `bun.lock` with
`bun install --lockfile-only`, refuse a Bun older than 1.4.1, and pack with
`bun pm pack`. It SHALL inspect the packed `package.json` and publish that
tarball through its own npm registry client after querying the packument. A package
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

### Requirement: A repository's own locked tool version moves after its release exists

The built-in `mise-pin` step SHALL run only after `finalize`.

It SHALL move the locked version with `mise lock <tool>@<version>` under
`MISE_LOCKFILE_MODE=generate`.

It SHALL then read `mise.lock` and fail unless every platform the lock lists
has a URL and a checksum for the new version.

#### Scenario: The locked version moves after finalize

- **WHEN** `at.released` lists `mise-pin` for tool `github:victor-software-house/verctl` and `v0.4.1` has been finalized
- **THEN** `verctl release` runs `mise lock github:victor-software-house/verctl@0.4.1` with `MISE_LOCKFILE_MODE=generate` after the Release is published

#### Scenario: A platform has no asset

- **WHEN** `mise.lock` lists `linux-x64` and `macos-arm64`, and the `v0.4.1` Release has no `macos-arm64` asset
- **THEN** `mise-pin` exits non-zero and names `macos-arm64`, though `mise lock` exited 0

### Requirement: verctl performs the OIDC exchange when asked

When a registry's `auth` is `oidc`, verctl SHALL exchange the GitHub Actions
OIDC token for a registry token itself. The exchange happens once per run
for crates.io, and once per package for npm. When `auth` is `token`, it SHALL read the token from the
registry's documented environment variable.

#### Scenario: Two npm packages

- **WHEN** `@demo/a` and `@demo/b` publish to `npm` with `auth: oidc`
- **THEN** `verctl release` performs two exchanges, each naming its own package

#### Scenario: No OIDC available

- **WHEN** `auth` is `oidc` and `ACTIONS_ID_TOKEN_REQUEST_URL` is unset
- **THEN** `verctl release` exits non-zero before any side effect and names `id-token: write`
