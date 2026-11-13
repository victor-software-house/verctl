## REMOVED Requirements

### Requirement: A package has no skipped version since the last release

**Reason**: HEAD is the release reference, and a run never releases an older version, so a skipped version is no longer a refusal.
**Migration**: A skipped version stays unreleased. Releasing it later is a separate opt-in that no change adds yet.

### Requirement: A skipped version has one definition

**Reason**: HEAD is the release reference, and a run never releases an older version, so `verctl publish --plan` has no skipped version to list.
**Migration**: A skipped version stays unreleased. Releasing it later is a separate opt-in that no change adds yet.

### Requirement: A skipped version is listed at the oldest commit

**Reason**: HEAD is the release reference, and a run never releases an older version, so no listing at an older commit remains.
**Migration**: A skipped version stays unreleased. Releasing it later is a separate opt-in that no change adds yet.
