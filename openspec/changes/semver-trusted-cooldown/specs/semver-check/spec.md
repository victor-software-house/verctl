## ADDED Requirements

### Requirement: A check blocked only by trusted crates runs a second time

When `cargo semver-checks` fails and cargo names one or more requirements as too new for `min-publish-age`, `verctl semver` SHALL read the crates.io owners of each crate so named.
When each has a user owner whose login `CARGO_TRUSTED_OWNERS` lists, `verctl semver` SHALL run the same command once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`.
The second run SHALL decide the crate's row.

#### Scenario: The baseline is a fresh release with a trusted owner

- **WHEN** `CARGO_TRUSTED_OWNERS=any-victor`, `qctl` 0.7.0 was published 2 days ago with owner `any-victor`, and `cargo semver-checks --package qctl --baseline-version 0.7.0` fails with "failed to select a version for the requirement `qctl = "=0.7.0"`" and "version 0.7.0 is too new"
- **THEN** `verctl semver` runs the command again with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`
- **AND** reports `qctl` as `passed` when that run passes

### Requirement: A check blocked by an untrusted crate fails as before

When any crate the cooldown rejected has no user owner that `CARGO_TRUSTED_OWNERS` lists, or the variable is unset or empty, `verctl semver` SHALL run no second check.
Its error SHALL name each rejected crate and the reason no retry ran.

#### Scenario: A third-party crate is too new

- **WHEN** `CARGO_TRUSTED_OWNERS=any-victor`, the failure names `serde` as too new, and crates.io lists no owner `any-victor` for `serde`
- **THEN** `verctl semver` runs no second check
- **AND** exits non-zero, naming `serde` as rejected by the cooldown and not owned by a trusted login

#### Scenario: No trusted owners are set

- **WHEN** `CARGO_TRUSTED_OWNERS` is unset and the failure names `qctl` as too new
- **THEN** `verctl semver` runs no second check, and its error names `CARGO_TRUSTED_OWNERS` as unset
