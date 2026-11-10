## ADDED Requirements

### Requirement: A release with assets is a draft until every asset is uploaded

verctl SHALL create the GitHub Release as a draft when the package declares
`assets`.
It SHALL upload only assets that are not `uploaded`.
It SHALL publish the draft only when every declared asset is `uploaded`.
An existing Release for the tag SHALL be updated and SHALL NOT be duplicated.

#### Scenario: One asset job failed

- **WHEN** `verctl` declares assets `darwin-arm64` and `linux-x64` and only `linux-x64` was uploaded
- **THEN** `verctl release` leaves the Release a draft
- **AND** `verctl release asset darwin-arm64` uploads the missing asset, after which `verctl release` publishes the Release

#### Scenario: The Release exists

- **WHEN** a GitHub Release for `v0.4.0` exists and its body differs from the `## verctl 0.4.0` CHANGELOG section
- **THEN** `verctl release` updates that Release's body and creates no second Release

### Requirement: A first publication stops for registry setup

When a package has never been published and its registry's `auth` is `oidc`,
verctl SHALL report the package at stage `bootstrap`.
It SHALL name the registry setup that is missing and run no transition for the
package.

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

### Requirement: Whether a release publishes is decided at the release commit

verctl SHALL decide whether a release publishes from the package's manifest
and `.ctl/ver.yaml` at its release commit only.
A later change of that setting SHALL apply from the next version.
A version merged while its package published SHALL be published even if
`publish = false` or `"private": true` lands before the release runs.
verctl SHALL offer no way to cancel that publish.

#### Scenario: A tool crate starts to publish without a bump

- **WHEN** `xtask@0.1.1` was merged with `publish = false`, and a later commit removes `publish = false` without changing the version
- **THEN** `verctl status` still reports `xtask@0.1.1` as `released`, and `verctl propose` is not blocked by it
- **AND** the next version of `xtask` goes through `publish`, or `bootstrap` when the registry needs a first publish

#### Scenario: A crate stops publishing before the release ran

- **WHEN** `demo@0.4.0` was merged while `demo` published, is not yet on crates.io, and a later commit adds `publish = false` without changing the version
- **THEN** `verctl release` still publishes `demo@0.4.0`, because its release commit publishes
- **AND** the next version of `demo` does not publish

#### Scenario: A dependency whose release will never publish

- **WHEN** `app@0.2.0` publishes at its release commit and keeps a dependency on `core`, and `core@0.3.0` was merged while `core` declared `publish = false`
- **THEN** `verctl status` reports `app` at stage `conflict`, naming `app` and `core`, and `verctl release` does not wait on `core`
