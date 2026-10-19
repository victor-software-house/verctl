## ADDED Requirements

### Requirement: The version step writes every file a release changes

`verctl version` SHALL apply the plan of the pending fragments to the working
tree: each manifest, each dependent whose requirement no longer admits the new
version, each lockfile through its owner, each `version` step, each package
CHANGELOG, each served file, and the removal of every consumed fragment. It
SHALL report the commands and steps it ran under `after`.

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

### Requirement: A Version PR is identified by facts that cannot change

verctl SHALL identify a Version PR as a pull request whose head repository is
the repository itself, whose head branch is `verctl/release/<base>`, and whose
current base, read from the pull request when verctl runs, is the branch named
by `branch` and equals `<base>`. It SHALL NOT read a pull request's title,
body, or labels to identify it, and SHALL NOT take the base from an event
payload.

#### Scenario: A fork uses the same branch name

- **WHEN** a pull request from a fork has head branch `verctl/release/main` and changes `Cargo.toml` from `0.3.0` to `0.3.1` with a consumed fragment
- **THEN** `verctl check` exits non-zero and names the head repository

#### Scenario: The base was changed

- **WHEN** the open pull request on `verctl/release/main` was retargeted to `next`
- **THEN** `verctl propose` exits non-zero before pushing to `verctl/release/main`, names that pull request to close, and opens no second one
- **AND** `verctl check` on it exits non-zero and names the base

#### Scenario: The title was edited

- **WHEN** the Version PR's title was changed to `wip`
- **THEN** `verctl propose` still finds and updates it

### Requirement: The Version PR follows the plan

`verctl propose` SHALL run the version step on a tree at the tip of `branch`,
commit it on `verctl/release/<branch>`, and open or update the Version PR
titled `proposal.title`. It SHALL close an open Version PR when no fragment is
pending, and SHALL refuse to open or update one while any package's current
release is short of `released`, except a package in `conflict`, which needs a
new version to resolve.

#### Scenario: The owner command re-resolves more than the bump

- **WHEN** `bun install --lockfile-only` on the release branch tip also moves an unrelated entry in `bun.lock`
- **THEN** `verctl propose` exits non-zero without committing, names the moved entry, and says the release branch needs a lockfile refresh

#### Scenario: A package in conflict

- **WHEN** `core` publishes on `main`, `core@0.3.0` was merged while it did not, `app@0.2.0` is therefore at stage `conflict`, and a fragment for `core` is on `main`
- **THEN** `verctl propose` opens the Version PR that moves `core` to a version whose release commit publishes

#### Scenario: No fragment is pending

- **WHEN** a Version PR is open and every fragment it consumed has been removed from `main` by another pull request
- **THEN** `verctl propose` closes the Version PR with a comment that says no fragment is pending

#### Scenario: A merged release is incomplete

- **WHEN** `demo@0.4.0` is at stage `published` and a new fragment for `demo` is on `main`
- **THEN** `verctl propose` exits zero without opening a Version PR
- **AND** its report names `demo@0.4.0` and the transition it waits on

### Requirement: check requires identity and content

`verctl check` SHALL compare HEAD with its merge-base on `branch`. A pull
request that is not a Version PR SHALL fail when it changes a manifest version
or adds a CHANGELOG section; it may add or remove fragments. Any pull request
that changes whether a package publishes SHALL fail unless it adds a fragment
for that package.
A manifest version equal to that package's version at the merge-base of HEAD
and the release branch SHALL NOT count as changed, whatever the pull request's
base.

#### Scenario: A hand-edited version on a feature branch

- **WHEN** a pull request from branch `fix/typo` changes `Cargo.toml` from `0.3.0` to `0.3.1`
- **THEN** `verctl check` exits non-zero and its error names `demo 0.3.0 -> 0.3.1` and the branch

#### Scenario: The release branch merged into next

- **WHEN** a pull request into `next` merges `main`, which released `0.3.1` after `next` branched at `0.3.0`
- **THEN** `verctl check` exits zero

#### Scenario: The released version typed on next

- **WHEN** a pull request into `next` changes `Cargo.toml` from `0.3.0` to `0.3.1` without merging `main`
- **THEN** `verctl check` exits non-zero and its error names `demo 0.3.0 -> 0.3.1`

#### Scenario: Publishing turned on without a fragment

- **WHEN** a pull request removes `publish = false` from crate `xtask` and adds no fragment for `xtask`
- **THEN** `verctl check` exits non-zero and names `xtask` and the missing fragment

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
of the fragments present at the merge-base and absent at HEAD, every planned
bump is applied, and each changed package's CHANGELOG carries the new
version's section. A manifest SHALL differ only in version fields and in the
dependency requirements the plan rewrites.

#### Scenario: The Version PR on its first CI run

- **WHEN** the pull request was opened by `verctl propose` and carries no label
- **THEN** `verctl check` exits zero

#### Scenario: A Version PR that adds a dependency

- **WHEN** the Version PR bumps `demo` in `Cargo.toml` and also adds a dependency there
- **THEN** `verctl check` exits non-zero and names `Cargo.toml` and the added dependency

### Requirement: A Version PR leaves lockfiles and served files as the plan implies

A Version PR SHALL fail unless every lockfile agrees with its manifests, a
lockfile's parsed entries and header differ from the merge-base only in the
versions of the workspace packages the plan bumps and in references that
resolve to them, with no entry added or removed, and every served file equals
its template's render at the versions on HEAD.

#### Scenario: A stale lockfile

- **WHEN** the Version PR bumps `package.json` for workspace package `a` to `1.1.0`, and `bun.lock` still records `"version": "1.0.0"` for `packages/a`
- **THEN** `verctl check` exits non-zero and names `bun.lock` and `packages/a`

#### Scenario: A Version PR that moves a third-party dependency

- **WHEN** the Version PR bumps `demo` and also changes the resolved version of `serde` in `Cargo.lock`
- **THEN** `verctl check` exits non-zero and names `Cargo.lock` and `serde`

#### Scenario: A served file edited by hand

- **WHEN** `examples/mise.toml` is rendered from `.ctl/templates/mise.toml.jinja` and a pull request edits it by hand
- **THEN** `verctl check` exits non-zero and names the file and its template
