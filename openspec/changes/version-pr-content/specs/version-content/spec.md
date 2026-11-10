## ADDED Requirements

### Requirement: The version step writes every file a release changes

`verctl version` SHALL apply the plan of the pending fragments to the working
tree: each manifest, each dependent whose requirement no longer admits the new
version, each lockfile through its owner, and each `version` step. It SHALL
apply each package CHANGELOG, each served file, and the removal of every
consumed fragment. It SHALL report the commands and steps it ran under `after`.

#### Scenario: A cargo package with a tracked Cargo.lock

- **WHEN** `demo` at `Cargo.toml` is `0.3.0`, `Cargo.lock` is tracked, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl version --format json` exits zero
- **AND** `Cargo.toml` and `Cargo.lock` both record `demo` at `0.3.1`
- **AND** the report's `after` lists `cargo update --workspace`

#### Scenario: A fragment that bumps nothing is consumed

- **WHEN** `.changeset/docs.md` holds only `demo: none` and no other fragment is pending
- **THEN** `verctl version` removes `.changeset/docs.md` and changes no manifest

#### Scenario: A dependent requirement no longer admits the new version

- **WHEN** `app` depends on workspace crate `core` with `version = "0.3"`, and `core` gets a `minor` fragment while at `0.3.2`
- **THEN** `verctl version` sets `core` to `0.4.0`, rewrites `app`'s requirement to `0.4`, and bumps `app` by `patch`
- **AND** `app`'s CHANGELOG section names the dependency update

### Requirement: A Version PR changes only what the version step writes

A Version PR SHALL fail when it changes any path outside what the version step
writes: manifests, lockfiles, CHANGELOGs, served files, consumed fragments, and
the declared `writes` of `version` steps, with the write set read from the
configuration at the merge-base. `check` SHALL constrain only which paths a
`version` step's declared `writes` change, not their content, which the Version
PR's review covers.

#### Scenario: A Version PR that carries a source change

- **WHEN** the Version PR also changes `src/main.rs`
- **THEN** `verctl check` exits non-zero and names `src/main.rs`

#### Scenario: A Version PR that widens its own write set

- **WHEN** the Version PR adds `src/lib.rs` to a `version` step's `writes` in `.ctl/ver.yaml` and changes `src/lib.rs`
- **THEN** `verctl check` exits non-zero and names `src/lib.rs`, because the write set comes from the merge-base

### Requirement: A Version PR applies the plan

A Version PR SHALL fail unless every changed manifest version equals the plan
of the fragments present at the merge-base and absent at HEAD, and every planned
bump is applied. It SHALL also fail unless each changed package's CHANGELOG
carries the new version's section. A manifest SHALL differ only in version fields and in the
dependency requirements the plan rewrites.

#### Scenario: The Version PR on its first CI run

- **WHEN** the pull request was opened by `verctl propose` and carries no label
- **THEN** `verctl check` exits zero

#### Scenario: A Version PR that adds a dependency

- **WHEN** the Version PR bumps `demo` in `Cargo.toml` and also adds a dependency there
- **THEN** `verctl check` exits non-zero and names `Cargo.toml` and the added dependency

### Requirement: A Version PR leaves lockfiles and served files as the plan implies

A Version PR SHALL fail unless every lockfile agrees with its manifests. It
SHALL also fail unless a lockfile's parsed entries and header differ from the
merge-base only in the versions of the workspace packages the plan bumps and in
references that resolve to them. It SHALL also fail if a lockfile has an entry
added or removed. It SHALL also fail unless every served file equals its template's render at the versions on
HEAD.

#### Scenario: A stale lockfile

- **WHEN** the Version PR bumps `package.json` for workspace package `a` to `1.1.0`, and `bun.lock` still records `"version": "1.0.0"` for `packages/a`
- **THEN** `verctl check` exits non-zero and names `bun.lock` and `packages/a`

#### Scenario: A Version PR that moves a third-party dependency

- **WHEN** the Version PR bumps `demo` and also changes the resolved version of `serde` in `Cargo.lock`
- **THEN** `verctl check` exits non-zero and names `Cargo.lock` and `serde`

#### Scenario: A served file edited by hand

- **WHEN** `examples/mise.toml` is rendered from `.ctl/templates/mise.toml.jinja` and a pull request edits it by hand
- **THEN** `verctl check` exits non-zero and names the file and its template
