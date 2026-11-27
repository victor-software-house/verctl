## ADDED Requirements

### Requirement: A member upload passes the cooldown for its own siblings

When a workspace member's `cargo publish` fails and cargo names as too new only crates that are members of the same workspace, verctl SHALL run that upload once more with `CARGO_RESOLVER_INCOMPATIBLE_PUBLISH_AGE=allow`, and that run SHALL decide.
When cargo names any other crate, the upload SHALL fail with an error that names each refused crate that is not a member.
The second run SHALL happen only when the upload's command passes `--locked` or `--frozen`.

#### Scenario: A sibling published seconds earlier

- **WHEN** `release` publishes `ctl-text` and then `ctl-layout`, which requires `ctl-text = "=0.8.0-rc.2"`, under a three-day cooldown
- **THEN** the first `ctl-layout` upload fails on the cooldown, the second runs with the cooldown off, and `ctl-layout` is published

#### Scenario: A publisher without --locked

- **WHEN** a `publish` list replaces the command with one that passes neither `--locked` nor `--frozen`, and the cooldown refuses a sibling
- **THEN** the upload fails without a second run, and the error names the missing `--locked`

#### Scenario: A third-party crate too new

- **WHEN** the cooldown refuses `serde` for a member's upload
- **THEN** the upload fails without a second run, and the error names `serde` as not a member of the workspace
