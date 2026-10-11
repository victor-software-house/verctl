## ADDED Requirements

### Requirement: The version step writes every file a release changes

`verctl version` SHALL apply the plan of the pending fragments to the working
tree: each manifest, each dependent whose requirement no longer admits the new
version, each lockfile, each package CHANGELOG, each served file, and the
removal of every consumed fragment. It SHALL report the commands it ran under
`after`.

#### Scenario: A cargo package with a tracked Cargo.lock

- **WHEN** `demo` at `Cargo.toml` is `0.3.0`, `Cargo.lock` is tracked, and `.changeset/fix.md` holds `demo: patch`
- **THEN** `verctl version --format json` exits zero
- **AND** `Cargo.toml` and `Cargo.lock` both record `demo` at `0.3.1`
- **AND** the report's `after` lists `cargo update --workspace`

#### Scenario: A none-only fragment is consumed

- **WHEN** `.changeset/docs.md` holds only `demo: none` and another fragment holds `demo: patch`
- **THEN** `verctl version` removes both fragments

#### Scenario: A dependent requirement no longer admits the new version

- **WHEN** `app` depends on workspace crate `core` with `version = "0.3"`, and `core` gets a `minor` fragment while at `0.3.2`
- **THEN** `verctl version` sets `core` to `0.4.0`, rewrites `app`'s requirement to `0.4`, and bumps `app` by `patch`
- **AND** `app`'s CHANGELOG section names the dependency update

### Requirement: The Version PR follows the plan

`verctl propose` SHALL apply the version step to a tree at the tip of `branch`,
commit it on `proposal.branch`, and open or update the Version PR titled
`proposal.title`. It SHALL close an open Version PR when the plan is empty, and
SHALL refuse to open or update one while any package's current release is short
of `released`.

#### Scenario: The plan empties

- **WHEN** a Version PR is open and every fragment it consumed has been removed from `main` by another pull request
- **THEN** `verctl propose` closes the Version PR with a comment that says the plan is empty

#### Scenario: A merged release is incomplete

- **WHEN** `demo@0.4.0` is at stage `published` and a new fragment for `demo` is on `main`
- **THEN** `verctl propose` exits zero without opening a Version PR
- **AND** its report names `demo@0.4.0` and the transition it waits on

### Requirement: check verifies a Version PR by its content

`verctl check` SHALL compare HEAD with its merge-base on `branch` and SHALL
fail unless every changed manifest version equals the plan of the fragments
present at the merge-base and absent at HEAD, every planned bump is applied,
each changed package's CHANGELOG carries the new version's section, every
lockfile agrees with its manifests, and every served file equals its template's
render at the versions on HEAD.

#### Scenario: A hand-edited version

- **WHEN** a pull request changes `Cargo.toml` from `0.3.0` to `0.3.1` and removes no fragment
- **THEN** `verctl check` exits non-zero and its error names `demo 0.3.0 -> 0.3.1` and the missing fragment

#### Scenario: The Version PR on its first CI run

- **WHEN** the pull request was opened by `verctl propose` and carries no label
- **THEN** `verctl check` exits zero

#### Scenario: A stale lockfile

- **WHEN** a pull request bumps `package.json` for workspace package `a` to `1.1.0` with its fragment, and `bun.lock` still records `"version": "1.0.0"` for `packages/a`
- **THEN** `verctl check` exits non-zero and names `bun.lock` and `packages/a`

#### Scenario: A served file edited by hand

- **WHEN** `examples/mise.toml` is rendered from `.ctl/templates/mise.toml.jinja` and a pull request edits it by hand
- **THEN** `verctl check` exits non-zero and names the file and its template
