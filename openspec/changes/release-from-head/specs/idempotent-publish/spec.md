## REMOVED Requirements

### Requirement: verctl publishes only at a version commit

**Reason**: HEAD is the release reference. A run at a later commit releases the versions HEAD still holds, with HEAD's code, so it no longer sends the retry to the version commit.

**Migration**: none. A release that failed at its version commit resumes at whatever commit the default branch holds.

### Requirement: A publish acts only on the packages released at HEAD

**Reason**: HEAD is the release reference. A run releases every version HEAD's manifests hold whose release is incomplete, from HEAD, whichever commit set it.

**Migration**: none. A version an older commit set and HEAD still holds is released from HEAD. A version HEAD no longer holds stays skipped.

## ADDED Requirements

### Requirement: A release acts on every incomplete version HEAD holds

`verctl release` SHALL publish, tag, and release, from HEAD, every package version HEAD's manifests hold whose release is incomplete.
It SHALL leave out a version that HEAD's manifests no longer hold, and name no older commit in its plan.

#### Scenario: Two packages bumped at different commits

- **WHEN** commit `A` set `core` to `0.3.1`, later commit `C` set `app` to `0.2.1`, neither is published, and `verctl release` runs at `C`
- **THEN** it publishes, tags, and releases `core@0.3.1` and `app@0.2.1` from `C`

#### Scenario: The plan names HEAD only

- **WHEN** the same history, and `verctl status --github-output "$GITHUB_OUTPUT"` runs at `C`
- **THEN** it writes `commits=["<C>"]` and `pending=true` to `$GITHUB_OUTPUT`

#### Scenario: A version HEAD no longer holds

- **WHEN** commit `A` set `core` to `0.3.1`, later commit `C` set it to `0.4.0`, neither is published, and `verctl release` runs at `C`
- **THEN** it publishes `core@0.4.0` only, and `core@0.3.1` stays unpublished

#### Scenario: HEAD moved past the version commit

- **WHEN** commit `A` set `demo` to `0.4.6`, commit `B` followed without touching `Cargo.toml`, crates.io lacks `0.4.6`, and `verctl release` runs at `B`
- **THEN** it publishes `demo@0.4.6` from `B`

### Requirement: A run releases HEAD's versions only, from HEAD

A release run SHALL release the versions HEAD's manifests hold, from HEAD.
`verctl status` SHALL list HEAD's incomplete releases only.
The reusable `release.yml` SHALL check out no commit other than the one that triggered it.

#### Scenario: Status lists HEAD's versions

- **WHEN** commit `A` set `core` to `0.3.1` and was never released, a later commit `C` set `core` to `0.4.0`, HEAD is `C`, and `verctl status` runs
- **THEN** it lists `core@0.4.0` and does not list `core@0.3.1`

#### Scenario: The workflow checks out one commit

- **WHEN** `release.yml` runs on a push to `main`
- **THEN** every `actions/checkout` step in it checks out `github.sha` and no job checks out another ref
