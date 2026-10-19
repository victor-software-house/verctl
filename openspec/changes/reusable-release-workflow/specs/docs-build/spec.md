## ADDED Requirements

### Requirement: verctl checks the docs.rs build of a released crate

`verctl docs` SHALL poll docs.rs, with a bounded backoff, for each package
released at HEAD whose publish target is crates.io, that has a library target
as Cargo resolves it, and whose own release tag for that version exists on
`origin` and names HEAD, until docs.rs reports `doc_status` true. It SHALL exit non-zero, naming the crate, when the
timeout runs out while docs.rs still reports `doc_status` false or answers 404.

#### Scenario: A build that failed

- **WHEN** `demo@0.4.6` is released at HEAD with tag `v0.4.6` and docs.rs still answers `{"doc_status": false, "version": "0.4.6"}` when the timeout runs out
- **THEN** `verctl docs` exits non-zero and reports `demo@0.4.6` as `failed`

#### Scenario: A running build

- **WHEN** docs.rs answers `{"doc_status": false, "version": "0.4.6"}` for `demo@0.4.6`, then `{"doc_status": true, "version": "0.4.6"}` within the timeout
- **THEN** `verctl docs` reports `demo@0.4.6` as `built` and exits zero

#### Scenario: A queued build

- **WHEN** docs.rs answers 404 for `demo@0.4.6`, then `{"doc_status": true, "version": "0.4.6"}` within the timeout
- **THEN** `verctl docs` reports `demo@0.4.6` as `built` and exits zero

#### Scenario: A build that never arrives

- **WHEN** docs.rs still answers 404 for `demo@0.4.6` when `VERCTL_TIMEOUT=1` runs out
- **THEN** `verctl docs` exits non-zero and names `demo@0.4.6` as unbuilt

#### Scenario: A binary-only crate

- **WHEN** the only package released at HEAD is a crates.io crate whose `cargo metadata` lists no library target
- **THEN** `verctl docs` asks docs.rs nothing and exits zero

#### Scenario: A library found by auto-discovery

- **WHEN** `demo` has `src/lib.rs` and no `[lib]` section, and `demo@0.4.6` is released at HEAD with tag `v0.4.6`
- **THEN** `verctl docs` polls docs.rs for `demo@0.4.6`

#### Scenario: Per-package tags with one publish failed

- **WHEN** `tags.template` is `{name}@{version}`, `core@0.3.1` reached crates.io, the publish of `app@0.2.1` failed, and neither tag exists
- **THEN** `verctl docs` asks docs.rs nothing, and after a rerun creates both tags it polls docs.rs for `core@0.3.1` and `app@0.2.1`

#### Scenario: A tag only the remote has

- **WHEN** the checkout holds no tags and `origin` has `v0.4.6` naming HEAD, where `demo@0.4.6` is released
- **THEN** `verctl docs` polls docs.rs for `demo@0.4.6`

#### Scenario: An annotated tag

- **WHEN** `origin` has an annotated tag `v0.4.6` whose tag object peels to HEAD, where `demo@0.4.6` is released
- **THEN** `verctl docs` polls docs.rs for `demo@0.4.6`

#### Scenario: A release whose publish failed

- **WHEN** `demo@0.4.6` is the version at HEAD and tag `v0.4.6` does not exist
- **THEN** `verctl docs` asks docs.rs nothing and exits zero

#### Scenario: A crate on another registry

- **WHEN** the only package released at HEAD publishes to a registry other than crates.io
- **THEN** `verctl docs` asks docs.rs nothing and exits zero

### Requirement: The docs.rs build is not part of a complete release

The `docs` job SHALL run after `publish` finishes, whether a leg failed or
not, and SHALL NOT delay `propose` or the next version commit's publish. A failed docs.rs build SHALL NOT make a
release incomplete.

#### Scenario: A later version commit failed to publish

- **WHEN** one run publishes version commit `A`, then the `publish` leg for later commit `C` fails
- **THEN** the `docs` leg at `A` checks the docs.rs build of `A`'s crates

#### Scenario: A rerun after a failed docs build

- **WHEN** the `docs` job failed for `demo@0.4.6` and the workflow runs again with everything else in place
- **THEN** `verctl publish --plan` reports `nothing to release`
