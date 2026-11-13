## ADDED Requirements

### Requirement: A partial release is its own state

`verctl status` SHALL report a release with some but not all registry uploads as a partial release, with its count and its cause.
A rerun, by dispatch or by "Re-run failed jobs", SHALL skip every crate the registry index lists, and SHALL resume with the rest.

#### Scenario: One of nine crates is live

- **WHEN** `shrub-parser` `0.7.0` is on crates.io, the other 8 crates are not, and crates.io refused `shrub-process` with status 429 and a `Retry-After` header
- **THEN** `verctl status` reports `partially published: 1 of 9 crates live`, names the 429 as the cause, and names the resume time from the header

#### Scenario: A rerun resumes

- **WHEN** the cause is gone and `verctl release` reruns
- **THEN** it runs no `cargo publish` for `shrub-parser`, publishes the other 8 crates in dependency order, and then publishes the draft Release
