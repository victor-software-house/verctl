## ADDED Requirements

### Requirement: The tag comes with the published Release, after every upload

`verctl publish` SHALL create a tag only by publishing the draft Release of the run, after every upload of the run has succeeded.
The run SHALL upload every declared asset to a draft Release first, then publish to the registries in dependency order, then publish the draft, which creates the tag at HEAD.
A run that finds the tag and a published Release SHALL create neither, and SHALL fail before any upload when the tag names a commit other than HEAD.

#### Scenario: Two crates and one asset target, in order

- **WHEN** a workspace holds crates `core` and `app` at `0.4.0`, `app` depends on `core`, `.ctl/ver.yaml` declares one asset target `linux-x86_64`, and `verctl publish` runs at HEAD
- **THEN** the run uploads the `linux-x86_64` asset to a draft Release for `v0.4.0`
- **AND** it then publishes `core` and `app`, in that order
- **AND** it then publishes the draft, which creates tag `v0.4.0` at HEAD

#### Scenario: The crates.io rate limit on the second crate

- **WHEN** the workspace above runs `verctl publish`, `core` publishes, and `cargo publish` for `app` fails with status 429
- **THEN** no tag `v0.4.0` exists and no published Release exists
- **AND** a draft Release for `v0.4.0` holds the `linux-x86_64` asset
- **AND** `verctl status` lists the release as incomplete

#### Scenario: A rerun finds the finished release

- **WHEN** tag `v0.4.0` names HEAD, its Release is published, the crates.io index lists `core` and `app` at `0.4.0`, and `verctl publish` runs
- **THEN** it creates no tag and no Release, and reports `nothing to release`

## REMOVED Requirements

### Requirement: A registry-checked package is tagged before the uploads

**Reason**: The tag comes last, with the published Release, so that a tag never exists for a release that did not complete.
**Migration**: A rerun after a failed upload resumes at HEAD and creates the tag only after the last upload succeeds. Nothing is needed in a repository's workflow.
