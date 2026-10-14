## ADDED Requirements

### Requirement: Release state is derived from facts on every run

verctl SHALL store no release state. On every run it SHALL derive each
package's stages from the fragments, the manifests, the default branch's
first-parent history, the lockfiles, the registry, the remote tags, the GitHub
Release, and its assets. It SHALL NOT read a label, a branch name, or an event
payload to decide a stage.

#### Scenario: A rerun on a later commit

- **WHEN** `demo@0.4.0` was published by an earlier run whose tag push failed, and two more commits have landed on `main` since
- **THEN** `verctl status --format json` reports `demo` at stage `published` with release commit equal to the commit where `Cargo.toml` became `0.4.0`
- **AND** `verctl release` run on that release commit creates tag `v0.4.0` there and reports the publish as `already`

#### Scenario: No label is read

- **WHEN** `GITHUB_EVENT_PATH` names a payload whose pull request carries no labels
- **THEN** `verctl status` and `verctl check` report the same result as with the label present

### Requirement: The release commit comes from first-parent history

verctl SHALL take the release commit of `p@v` to be the newest commit on the
first-parent history of the branch named by `branch` where package `p`'s
resolved version became `v`, resolving it at each commit from `p`'s manifest
and, when the version is inherited, the workspace root manifest. It SHALL NOT
take HEAD as the release commit.

#### Scenario: HEAD has moved past the release

- **WHEN** commit `A` on `main` set `demo` to `0.4.0` and commit `B` followed it without touching `Cargo.toml`
- **THEN** `verctl status --format json` reports `release_commit` as `A` for `demo@0.4.0`

#### Scenario: Publishing from the wrong commit

- **WHEN** `verctl release` runs with HEAD at `B` and `demo@0.4.0` is unpublished
- **THEN** it exits non-zero before any side effect
- **AND** its error names commit `A` as the one to check out

### Requirement: Each transition is guarded and repeatable

verctl SHALL run a transition only when its guard holds, SHALL report `already`
and do nothing when its postcondition already holds, and SHALL stop the run at
the first failed transition without attempting anything that depends on it.

#### Scenario: The registry already lists the version

- **WHEN** crates.io's sparse index lists `demo` `0.4.0` and the tag is missing
- **THEN** `verctl release` does not run `cargo publish`
- **AND** it creates the tag and reports `publish: already`

#### Scenario: A dependency failed to publish

- **WHEN** `unixd` depends on `unixd-cache`, both move in one release, and `cargo publish -p unixd-cache` fails
- **THEN** `verctl release` exits non-zero and never runs `cargo publish -p unixd`

### Requirement: Publish precedes tag, and tags land on the release commit

verctl SHALL create a tag only after every publishing package the tag names
is published,
SHALL create it at the release commit, SHALL push all ready tags of a run in one
atomic push, and SHALL treat an existing tag that peels to another commit as a
conflict that stops every transition for that package.

#### Scenario: A tag on another commit

- **WHEN** remote tag `v0.4.0` peels to a commit other than the release commit of `demo@0.4.0`
- **THEN** `verctl release` exits non-zero before any side effect
- **AND** `verctl status --format json` reports `demo` at stage `conflict` naming both commits

#### Scenario: Two crates, two tags

- **WHEN** `tags` is `{name}-v{version}` and `unixd-cache@0.2.0` and `unixd@0.3.0` are published in one run
- **THEN** `verctl release` pushes `unixd-cache-v0.2.0` and `unixd-v0.3.0` with one `git push --atomic`

### Requirement: A release with assets is a draft until every asset is uploaded

verctl SHALL create the GitHub Release as a draft when the package declares
`assets`, SHALL upload only assets that are not `uploaded`, and SHALL publish
the draft only when every declared asset is `uploaded`. An existing Release for
the tag SHALL be updated, never duplicated.

#### Scenario: One asset job failed

- **WHEN** `verctl` declares assets `darwin-arm64` and `linux-x64` and only `linux-x64` was uploaded
- **THEN** `verctl release` leaves the Release a draft
- **AND** `verctl release asset darwin-arm64` uploads the missing asset, after which `verctl release` publishes the Release

#### Scenario: The Release exists

- **WHEN** a GitHub Release for `v0.4.0` exists and its body differs from the `## verctl 0.4.0` CHANGELOG section
- **THEN** `verctl release` updates that Release's body and creates no second Release

### Requirement: A first publication is a stop, not a guess

When a package has never been published and its registry's `auth` is `oidc`,
verctl SHALL report the package at stage `bootstrap`, name the registry setup
that is missing, and run no transition for it.

#### Scenario: A new crate under OIDC

- **WHEN** `demo` is not on crates.io and `registries.crates-io.auth` is `oidc`
- **THEN** `verctl release` exits non-zero before any side effect
- **AND** its error names `demo`, `crates-io`, and the first publish that must happen with a token

### Requirement: A release that does not publish completes without a registry

A package does not publish when it declares `publish = false` or
`"private": true`, or uses a declared ecosystem with no publish block. A
release that does not publish SHALL have no publish transition. Without
`assets`, verctl SHALL treat it as `released` once it is merged and SHALL
create no tag and no Release for it. With `assets`, it SHALL be tagged and
released like any other package.

#### Scenario: A bumped tool crate

- **WHEN** crate `xtask` declares `publish = false`, has no `assets`, and the Version PR that bumped it to `0.1.1` is merged
- **THEN** `verctl status` reports `xtask@0.1.1` as `released`
- **AND** `verctl release` creates no tag and no Release for `xtask`
- **AND** a later `verctl propose` is not blocked by `xtask`

### Requirement: Whether a release publishes is decided at its release commit

verctl SHALL decide whether a release publishes from the package's manifest
and `.ctl/ver.yaml` at its release commit only. A later change of that setting
SHALL apply from the next version: a version merged while its package
published SHALL be published even if `publish = false` or `"private": true`
lands before the release runs, and verctl SHALL offer no way to cancel it.

#### Scenario: A tool crate starts to publish without a bump

- **WHEN** `xtask@0.1.1` was merged with `publish = false`, and a later commit removes `publish = false` without changing the version
- **THEN** `verctl status` still reports `xtask@0.1.1` as `released`, and `verctl propose` is not blocked by it
- **AND** the next version of `xtask` goes through `publish`, or `bootstrap` when the registry needs a first publish

#### Scenario: A crate stops publishing before its release ran

- **WHEN** `demo@0.4.0` was merged while `demo` published, is not yet on crates.io, and a later commit adds `publish = false` without changing the version
- **THEN** `verctl release` still publishes `demo@0.4.0`, because its release commit publishes
- **AND** the next version of `demo` does not publish

#### Scenario: A dependency whose release will never publish

- **WHEN** `app@0.2.0` publishes at its release commit and keeps a dependency on `core`, and `core@0.3.0` was merged while `core` declared `publish = false`
- **THEN** `verctl status` reports `app` at stage `conflict`, naming `app` and `core`, and `verctl release` does not wait on `core`
