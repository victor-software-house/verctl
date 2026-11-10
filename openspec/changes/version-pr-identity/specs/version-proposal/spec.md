## ADDED Requirements

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
or adds a CHANGELOG heading for a version; it may add or remove fragments.
A manifest version or a CHANGELOG version present at the merge-base of HEAD
and the release branch SHALL NOT count as changed or added, whatever the pull
request's base.

#### Scenario: A hand-edited version on a feature branch

- **WHEN** a pull request from branch `fix/typo` changes `Cargo.toml` from `0.3.0` to `0.3.1`
- **THEN** `verctl check` exits non-zero and its error names `demo 0.3.0 -> 0.3.1` and the branch

#### Scenario: The release branch merged into next

- **WHEN** a pull request into `next` merges `main`, which released `0.3.1` after `next` branched at `0.3.0`
- **THEN** `verctl check` exits zero

#### Scenario: The released version typed on next

- **WHEN** a pull request into `next` changes `Cargo.toml` from `0.3.0` to `0.3.1` without merging `main`
- **THEN** `verctl check` exits non-zero and its error names `demo 0.3.0 -> 0.3.1`

#### Scenario: A CHANGELOG section added by hand

- **WHEN** a pull request from branch `fix/typo` adds `## 0.3.1` to `CHANGELOG.md`
- **THEN** `verctl check` exits non-zero and names `CHANGELOG.md` and `0.3.1`

#### Scenario: A typo fixed in a CHANGELOG heading

- **WHEN** a pull request changes the heading `## dmeo 0.3.0` to `## demo 0.3.0`
- **THEN** `verctl check` exits zero, because the pull request adds no version

### Requirement: check requires a release for a publish change

A pull request that is not a Version PR SHALL fail when it changes whether a
package publishes, unless it adds a fragment that bumps that package above
`none`. A publish setting equal to the package's setting at the merge-base of
HEAD and the release branch SHALL NOT count as changed.

#### Scenario: Publishing turned on without a fragment

- **WHEN** a pull request removes `publish = false` from crate `xtask` and adds no fragment for `xtask`
- **THEN** `verctl check` exits non-zero and names `xtask` and the missing fragment

#### Scenario: Publishing turned on with a none fragment

- **WHEN** a pull request removes `publish = false` from crate `xtask` and adds a fragment `xtask: none`
- **THEN** `verctl check` exits non-zero and names `xtask` and the missing fragment

#### Scenario: A released publish change merged into next

- **WHEN** a pull request into `next` merges `main`, whose Version PR released `xtask` after a pull request removed `publish = false`
- **THEN** `verctl check` exits zero
