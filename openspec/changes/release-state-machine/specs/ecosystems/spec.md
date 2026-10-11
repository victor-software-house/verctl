## ADDED Requirements

### Requirement: The manifest selects the ecosystem

verctl SHALL treat a package whose `path` names `Cargo.toml` as `cargo` and one
whose `path` names `package.json` as `bun`. A `package.json` with no tracked
`bun.lock` between its directory and the repository root SHALL fail the load
and name `ecosystem.command`. Any other manifest SHALL declare
`ecosystem.command`.

#### Scenario: A package.json without bun.lock

- **WHEN** package `web` at `package.json` has a tracked `pnpm-lock.yaml` and no `bun.lock`
- **THEN** `verctl status` exits non-zero and its error names `web` and `ecosystem.command`

### Requirement: Cargo is native

For a `cargo` package, verctl SHALL write `package.version` or
`workspace.package.version`, find dependents through `cargo metadata --no-deps`,
refresh a tracked `Cargo.lock` with `cargo update --workspace`, order publishes
over normal and build dependencies, decide "published" from the registry's
sparse index, and publish with `cargo publish --locked -p <name>` one package at
a time. A package with `publish = false` SHALL never be published.

#### Scenario: Published order in a workspace

- **WHEN** `unixd` has a normal dependency on `unixd-cache` and both move
- **THEN** `verctl release --dry-run` lists `unixd-cache` before `unixd`, whatever order `packages` declares

#### Scenario: A package that does not publish

- **WHEN** crate `xtask` declares `publish = false` and a fragment bumps it
- **THEN** `verctl release` does not publish `xtask` and reports it as `not published`

### Requirement: Bun is native

For a `bun` package, verctl SHALL write `version`, find dependents through
`workspace:` dependencies, refresh a tracked `bun.lock` with
`bun install --lockfile-only`, refuse a Bun older than 1.4.1, decide
"published" from the registry's packument, and publish by packing with
`bun pm pack`, inspecting the packed `package.json`, and publishing that
tarball. A package with `"private": true` SHALL never be published.

#### Scenario: A packed package still names a protocol

- **WHEN** the packed `package.json` of `@demo/b` holds `"@demo/a": "workspace:*"`
- **THEN** `verctl release` exits non-zero before `bun publish` and names `@demo/b` and `@demo/a`

#### Scenario: A packed package names a stale version

- **WHEN** `@demo/a` is `1.1.0` in its manifest and the packed `package.json` of `@demo/b` holds `"@demo/a": "1.0.0"`
- **THEN** `verctl release` exits non-zero before `bun publish` and names `bun.lock`

#### Scenario: An old Bun

- **WHEN** `bun --version` prints `1.4.0`
- **THEN** `verctl version` exits non-zero before writing any file and names `1.4.1`

### Requirement: Other stacks declare every step

A package with `ecosystem.command` SHALL declare `version.read` and
`version.write`, and MAY declare `lockfile.run` with `lockfile.writes`,
`published`, and `publish`. verctl SHALL run each declared argv for its step and
SHALL allow only the paths in `lockfile.writes` to change during the lockfile
step.

#### Scenario: A lockfile step writes an undeclared file

- **WHEN** `lockfile.run` is `[uv, lock]`, `lockfile.writes` is `[uv.lock]`, and the step also changes `pyproject.toml`
- **THEN** `verctl version` exits non-zero and names `pyproject.toml`

### Requirement: verctl performs the OIDC exchange

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
