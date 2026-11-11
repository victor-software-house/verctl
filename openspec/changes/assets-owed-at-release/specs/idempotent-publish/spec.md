## ADDED Requirements

### Requirement: A release owes the assets its version commit declared

A release SHALL owe an asset target only when `.ctl/ver.yaml` declares it both at HEAD and at the release's version commit.
A version commit without `.ctl/ver.yaml` SHALL owe no asset.
`verctl publish --plan` and `verctl status` SHALL read the config at a version commit only when the release lacks a target that HEAD declares.

#### Scenario: A target added after a release

- **WHEN** commit `A` set `demo` to `0.0.1` with `assets.targets` holding `linux-x64`, the Release `v0.0.1` holds the `linux-x64` tarball, and a later commit adds `darwin-arm64`
- **THEN** `verctl publish --plan` reports `nothing to release` for `demo@0.0.1`

#### Scenario: A target the release commit declared

- **WHEN** commit `A` set `demo` to `0.0.1` with `linux-x64` and `darwin-arm64`, and the Release `v0.0.1` holds only the `linux-x64` tarball
- **THEN** `verctl publish --plan` reports commit `A` as pending, and `verctl assets` plans only `darwin-arm64`
