## REMOVED Requirements

### Requirement: A publish acts only on the packages released at HEAD

**Reason**: HEAD is the release reference. A run releases every version HEAD's manifests hold whose release is incomplete, from HEAD, whichever commit set it.

**Migration**: none. A version an older commit set and HEAD still holds is released from HEAD. A version HEAD no longer holds stays skipped.

## ADDED Requirements

### Requirement: A publish acts on every incomplete version HEAD holds

`verctl publish` SHALL publish, tag, and release, from HEAD, every package version HEAD's manifests hold whose release is incomplete.
It SHALL leave out a version that HEAD's manifests no longer hold, and name no older commit in its plan.

#### Scenario: Two packages bumped at different commits

- **WHEN** commit `A` set `core` to `0.3.1`, later commit `C` set `app` to `0.2.1`, neither is published, and `verctl publish` runs at `C`
- **THEN** it publishes, tags, and releases `core@0.3.1` and `app@0.2.1` from `C`

#### Scenario: The plan names HEAD only

- **WHEN** the same history, and `verctl publish --plan` runs at `C`
- **THEN** it writes `commits=["<C>"]` and `pending=true` to `$GITHUB_OUTPUT`

#### Scenario: A version HEAD no longer holds

- **WHEN** commit `A` set `core` to `0.3.1`, later commit `C` set it to `0.4.0`, neither is published, and `verctl publish` runs at `C`
- **THEN** it publishes `core@0.4.0` only, and `core@0.3.1` stays unpublished
