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
with a named step.
The package SHALL keep every other block of its ecosystem.

#### Scenario: A crate that publishes through a mise task

- **WHEN** crate `demo` declares `publish: demo-publish` and `steps.demo-publish` is `{task: "release:demo"}`
- **THEN** `verctl version` still bumps `Cargo.toml` and runs `cargo update --workspace`
- **AND** `verctl release` runs `mise run --raw release:demo` instead of `cargo publish`, then queries crates.io for `demo` at the new version

### Requirement: Steps run as mise tasks or argv with declared effects

A step SHALL declare exactly one of `task` or `run`.
verctl SHALL run a `task` step as `mise run --raw <task>` and a `run` step as
its argv.

verctl SHALL pass `VERCTL_PLAN` in the environment of every step.
For a step that runs for a package, verctl SHALL also pass `VERCTL_PACKAGE`,
`VERCTL_VERSION`, `VERCTL_PREVIOUS_VERSION`, `VERCTL_TAG`, and
`VERCTL_RELEASE_COMMIT`.

verctl SHALL treat a non-zero exit as failure.
A step SHALL change only the paths in its `writes`.

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
lockfiles only to check that they match the manifests, and SHALL compare
a lockfile's parsed entries, never its bytes.

#### Scenario: A lock written by another mise version

- **WHEN** `mise.dev.lock` lists the same two `rust` entries as the lock `check` expects, in the opposite order
- **THEN** `verctl check` reports no lockfile problem

#### Scenario: A stale lockfile after a declared step

- **WHEN** the Lockfile step of a declared ecosystem exits zero but `uv.lock` still names the old version
- **THEN** `verctl version` exits non-zero and names `uv.lock`

### Requirement: A package that publishes depends only on packages that publish

verctl SHALL refuse to load a configuration in which a package that publishes
depends on a workspace package that does not publish.
The refusal SHALL name both packages.

#### Scenario: A published crate depends on a tool crate

- **WHEN** crate `demo` publishes and has a normal dependency on workspace crate `xtask`, which declares `publish = false`
- **THEN** `verctl status` exits non-zero and names `demo` and `xtask`

### Requirement: Only some dependencies count toward publishing

Dependencies match by package name, after renames, aliases, and workspace
inheritance are resolved. For cargo, normal and build dependencies count,
target-specific ones included, and so does a development dependency that
declares a `version`, which `cargo publish` keeps. For bun, `dependencies`,
`optionalDependencies`, and `peerDependencies` count. A path-only cargo
development dependency and a bun `devDependencies` entry SHALL NOT count.

#### Scenario: A versioned development dependency on a tool crate

- **WHEN** crate `demo` publishes and has a development dependency on `xtask` that declares `version = "0.1"`, and `xtask` declares `publish = false`
- **THEN** `verctl status` exits non-zero and names `demo` and `xtask`

#### Scenario: A published crate tests with a tool crate

- **WHEN** crate `demo` publishes and has a path-only development dependency on `xtask`, which declares `publish = false`
- **THEN** `verctl status` loads the configuration, and `demo`'s publish does not wait on `xtask`

### Requirement: A cargo dependency cycle is refused

Every dependency that counts SHALL also order publishes.
A package SHALL wait only on the dependencies that order puts before it.

For cargo, verctl SHALL refuse to load a configuration in which such
dependencies form a cycle.
Cargo cannot publish crates with cyclic versioned development dependencies
([rust-lang/cargo#4242][cargo-4242]).

#### Scenario: Two crates test with each other

- **WHEN** publishing crates `a` and `b` each have a versioned development dependency on the other
- **THEN** `verctl status` exits non-zero, names the cycle `a -> b -> a`, and says that a path-only development dependency breaks it

### Requirement: A bun dependency cycle is published in the listed order

For bun, a cycle SHALL NOT be refused, because the npm registry does not
require a dependency to exist first. Outside a cycle, publishes follow
dependency order whatever order `packages` lists; among the members of a bun
cycle, they follow the order `packages` lists them in `.ctl/ver.yaml`.

#### Scenario: Two bun packages that peer on each other

- **WHEN** publishing bun packages `@demo/host` and `@demo/plugin`, declared in that order, each list the other in `peerDependencies`
- **THEN** `verctl status` loads the configuration, and `verctl release` publishes `@demo/host` before `@demo/plugin`

[cargo-4242]: https://github.com/rust-lang/cargo/issues/4242
