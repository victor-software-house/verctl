## ADDED Requirements

### Requirement: The version step writes every file a release changes

`verctl version` SHALL apply the plan of the pending fragments to the working
tree: each manifest, each lockfile through its owner, and each `prepare.after`
command, each command once. It SHALL apply each package CHANGELOG, each pinned and
served file, and the removal of every consumed fragment, one that bumps
nothing included. It SHALL report the commands and steps it ran under
`after`.

#### Scenario: A cargo package with a tracked Cargo.lock

- **WHEN** `demo` at `Cargo.toml` is `0.3.0`, `Cargo.lock` is tracked, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl version --format json` exits zero
- **AND** `Cargo.toml` and `Cargo.lock` both record `demo` at `0.3.1`
- **AND** the report's `after` lists `cargo update --workspace`

#### Scenario: A fragment that bumps nothing is consumed

- **WHEN** `.changeset/docs.md` holds only `demo: none` and no other fragment is pending
- **THEN** `verctl version` removes `.changeset/docs.md` and changes no manifest

### Requirement: A Version PR changes only what the version step writes

A Version PR SHALL fail when it changes any path outside what the version step
writes. That write set holds manifests, lockfiles, CHANGELOGs, pinned and served
files, consumed fragments, and the paths `prepare.stage` names, read from the
configuration at the merge-base. `check` SHALL constrain only which paths that
`prepare.stage` names change, not their content, which the Version PR's review
covers.

#### Scenario: A Version PR that carries a source change

- **WHEN** the Version PR also changes `src/main.rs`
- **THEN** `verctl check` exits non-zero and names `src/main.rs`

#### Scenario: A Version PR that adds a path to the write set

- **WHEN** the Version PR adds `src/lib.rs` to `prepare.stage` in `.ctl/ver.yaml` and changes `src/lib.rs`
- **THEN** `verctl check` exits non-zero and names `src/lib.rs`, because the write set comes from the merge-base

#### Scenario: A path prepare.stage names at the merge-base

- **WHEN** `prepare.stage` at the merge-base names `docs/version.txt`, and the Version PR changes it
- **THEN** `verctl check` exits zero

### Requirement: The content check runs no command

`check` SHALL replay the version step without running a lockfile owner or a
`prepare.after` command, and without a network call. It SHALL give the same
answer with or without a registry. It MAY read the members of a Cargo
workspace with `cargo metadata --no-deps --offline`, and only for a planned
`Cargo.toml` whose lockfile is a `Cargo.lock`.

#### Scenario: Members read offline

- **WHEN** the Version PR moves `ctl-core`'s four `Cargo.lock` members and no registry is reachable
- **THEN** `verctl check` exits zero

### Requirement: A Version PR applies the plan

A Version PR SHALL fail unless every changed manifest version equals the plan
of the fragments present at the merge-base and absent at HEAD.
It SHALL fail unless every planned bump is applied.
It SHALL also fail unless each changed package's CHANGELOG
carries the new version's section above the merge-base text.

#### Scenario: The Version PR at the first CI run

- **WHEN** the pull request was opened by `verctl version --pr` and carries no label
- **THEN** `verctl check` exits zero

### Requirement: A Version PR changes a manifest only where the version step does

A manifest on a Version PR SHALL equal its merge-base text with the planned
version written by its driver.
Its pins SHALL be applied at the planned versions in that text.
A difference SHALL fail and name the manifest and the key that differs.

#### Scenario: A Version PR that adds a dependency

- **WHEN** the Version PR bumps `demo` in `Cargo.toml` and also adds a dependency there
- **THEN** `verctl check` exits non-zero and names `Cargo.toml` and the added dependency

#### Scenario: Requirements a pin rewrites

- **WHEN** `shrub`'s `Cargo.toml` pins eight `version = "={version}", path = "shrub-` requirements and the Version PR moves `shrub` and those eight requirements from `0.6.2` to `0.7.0`
- **THEN** `verctl check` exits zero

#### Scenario: A requirement no pin explains

- **WHEN** the Version PR bumps `demo` and also changes a requirement in `Cargo.toml` that no pin names
- **THEN** `verctl check` exits non-zero and names `Cargo.toml` and that dependency

### Requirement: A Version PR moves only workspace versions in a lockfile

A Version PR SHALL fail unless every lockfile matches its manifests.

It SHALL also fail unless a lockfile's parsed entries and header differ from the
merge-base only in two places.
One is the version of a workspace member whose version the plan moves, directly or through workspace inheritance.
The other is a reference that resolves to such a member.

It SHALL also fail if a lockfile has an entry added or removed.

#### Scenario: A stale lockfile

- **WHEN** the Version PR bumps `package.json` for workspace package `a` to `1.1.0`, and `bun.lock` still records `"version": "1.0.0"` for `packages/a`
- **THEN** `verctl check` exits non-zero and names `bun.lock` and `packages/a`

#### Scenario: Members that inherit the workspace version

- **WHEN** `ctl-core` at `Cargo.toml` sets `workspace.package.version`, members `ctl-layout`, `ctl-term`, and `ctl-text` inherit it, and the Version PR moves all four `Cargo.lock` entries from `0.6.28` to `0.7.0`
- **THEN** `verctl check` exits zero

#### Scenario: A member that moves without a planned bump

- **WHEN** the Version PR bumps `unixd` and `Cargo.lock` also moves `unixd-cache`, which no fragment bumps and which inherits no version from `unixd`
- **THEN** `verctl check` exits non-zero and names `Cargo.lock` and `unixd-cache`

#### Scenario: A Version PR that moves a third-party dependency

- **WHEN** the Version PR bumps `ctl-core` and also changes the resolved version of `anstyle` in `Cargo.lock`
- **THEN** `verctl check` exits non-zero and names `Cargo.lock` and `anstyle`

### Requirement: A lockfile that records no workspace version stays unchanged

A Version PR SHALL fail when it changes `pnpm-lock.yaml`, `poetry.lock`, or
`pdm.lock`, which record no workspace version. It SHALL also fail when it
changes `yarn.lock` or `bun.lockb`, which verctl does not read, and name the
file.

#### Scenario: A changed yarn.lock

- **WHEN** the Version PR bumps workspace package `a` and changes `yarn.lock`
- **THEN** `verctl check` exits non-zero and names `yarn.lock`

### Requirement: A Version PR leaves served and pinned files as rendered

A Version PR SHALL fail unless every served file equals its template's render
at the versions on HEAD.
It SHALL also fail unless every pinned file equals its merge-base text with
its pins applied at the planned versions.

#### Scenario: A served file edited by hand

- **WHEN** `examples/mise.toml` is rendered from `.ctl/templates/mise.toml.jinja` and a pull request edits it by hand
- **THEN** `verctl check` exits non-zero and names the file and its template

#### Scenario: Pinned files on the Version PR

- **WHEN** `README.md` and `skills/qctl/SKILL.md` pin `qctl`, and the Version PR moves `qctl` from `0.6.0` to `0.7.0` in each pinned line and nowhere else
- **THEN** `verctl check` exits zero
