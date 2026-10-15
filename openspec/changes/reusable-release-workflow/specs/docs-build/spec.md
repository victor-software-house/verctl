## ADDED Requirements

### Requirement: verctl checks the docs.rs build of a released crate

`verctl docs` SHALL poll docs.rs for each package released at HEAD whose
publish target is crates.io, until docs.rs reports the version built, with a
bounded backoff. It SHALL exit non-zero, naming the crate, when docs.rs
reports `doc_status` false, or when the crate is still unbuilt after the
timeout.

#### Scenario: A build that failed

- **WHEN** `demo@0.4.6` is released at HEAD and docs.rs answers `{"doc_status": false, "version": "0.4.6"}`
- **THEN** `verctl docs` exits non-zero and reports `demo@0.4.6` as `failed`

#### Scenario: A queued build

- **WHEN** docs.rs answers 404 for `demo@0.4.6`, then `{"doc_status": true, "version": "0.4.6"}` within the timeout
- **THEN** `verctl docs` reports `demo@0.4.6` as `built` and exits zero

#### Scenario: A build that never arrives

- **WHEN** docs.rs still answers 404 for `demo@0.4.6` when `VERCTL_TIMEOUT=1` runs out
- **THEN** `verctl docs` exits non-zero and names `demo@0.4.6` as unbuilt

#### Scenario: A crate on another registry

- **WHEN** the only package released at HEAD publishes to a registry other than crates.io
- **THEN** `verctl docs` asks docs.rs nothing and exits zero

### Requirement: The docs.rs build is not part of a complete release

The `docs` job SHALL run after `publish`, and SHALL NOT delay `propose` or
the next version commit's publish. A failed docs.rs build SHALL NOT make a
release incomplete.

#### Scenario: A rerun after a failed docs build

- **WHEN** the `docs` job failed for `demo@0.4.6` and the workflow runs again with everything else in place
- **THEN** `verctl publish --plan` reports `nothing to release`
