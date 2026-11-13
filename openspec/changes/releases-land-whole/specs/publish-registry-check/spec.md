## ADDED Requirements

### Requirement: verctl reads crate owners and trusted publishers

For each crate that publishes to crates.io, verctl SHALL read the crate's owners from the crates.io owners API, `GET /api/v1/crates/{name}/owners`, and the trusted publisher configs of the crate.
It SHALL report a crate by name when any of these holds:

1. the crate does not exist on crates.io
2. no trusted publisher config of the crate names the repository's release workflow
3. the owner list lacks the publishing identity

#### Scenario: A crate the registry lacks

- **WHEN** workspace member `shrub-process` is at `0.7.0` and `GET /api/v1/crates/shrub-process/owners` answers 404
- **THEN** verctl reports `shrub-process` as missing from crates.io

#### Scenario: A crate with no trusted publisher

- **WHEN** crate `demo` exists on crates.io and none of its trusted publisher configs names `release.yml` of the repository
- **THEN** verctl reports `demo` as lacking a trusted publisher for `release.yml`

#### Scenario: An owner list without the publishing identity

- **WHEN** `GET /api/v1/crates/demo/owners` lists `alice` and the publishing identity is the repository's GitHub Actions identity
- **THEN** verctl reports `demo` as not owned by the publishing identity, and names the owners it found
